// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from zickrian/isle @ a29a7c4 assistant (Copyright 2026 zickrian).
mod agent;
mod backends;
mod process;
mod protocol;

use isle_core::chat::{Approval, Message, ModelChoice};
use isle_core::state::AiProviderKind;
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::{mpsc, oneshot};

pub use backends::CLAUDE_MODELS;

pub enum Command {
    Ask {
        generation: u64,
        provider: AiProviderKind,
        /// Empty: the CLI's own default.
        model: String,
        items: Vec<Message>,
    },
    Cancel,
    /// A new OpenCode Go key, for the next request.
    Key(Option<String>),
    /// The user's answer to the command shown (`Update::approval`): Executar (true) or Negar.
    Approve { generation: u64, run: bool },
    /// The models a provider offers now, sent back as `Update::models`.
    Models(AiProviderKind),
}
#[derive(Debug, Default)]
pub struct Update {
    pub generation: u64,
    pub text: String,
    pub done: bool,
    pub error: Option<String>,
    /// A command waiting for Executar or Negar (None: none now).
    pub approval: Option<Approval>,
    /// A provider's models (or why they could not be listed); nothing else in this update.
    pub models: Option<(AiProviderKind, Result<Vec<ModelChoice>, String>)>,
}

/// An error as shown and logged: the OpenCode key never in it, at most 1200 characters.
fn redact(error: String, key: Option<&str>) -> String {
    let error = match key.filter(|k| !k.is_empty()) {
        Some(k) => error.replace(k, "[chave]"),
        None => error,
    };
    error.chars().take(1200).collect()
}

/// One answer at most this long; a command waiting for approval and running has its own limits.
const ASK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

type Pending = Arc<Mutex<Option<(u64, oneshot::Sender<bool>)>>>;

/// No CLI, timer, model query or polling until a request arrives. All I/O runs on rt's workers.
pub fn spawn(
    rt: &tokio::runtime::Runtime,
    mut key: Option<String>,
    emit: impl Fn(Update) + Send + Sync + 'static,
) -> mpsc::UnboundedSender<Command> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let emit = Arc::new(emit);
    let pending: Pending = Arc::default();
    let handle = rt.handle().clone();
    rt.spawn(async move {
        let mut active: Option<tokio::task::JoinHandle<()>> = None;
        while let Some(command) = rx.recv().await {
            match command {
                Command::Key(k) => key = k,
                Command::Approve { generation, run } => {
                    let waiting = pending.lock().unwrap_or_else(PoisonError::into_inner).take();
                    match waiting {
                        Some((g, answer)) if g == generation => { let _ = answer.send(run); }
                        other => *pending.lock().unwrap_or_else(PoisonError::into_inner) = other,
                    }
                }
                Command::Models(provider) => {
                    let (emit, key) = (emit.clone(), key.clone());
                    handle.spawn(async move {
                        let listed = tokio::time::timeout(std::time::Duration::from_secs(40), backends::models(provider, key.as_deref())).await;
                        let result = match listed {
                            Ok(Ok(models)) => Ok(models),
                            Ok(Err(err)) => Err(redact(format!("{err:#}"), key.as_deref())),
                            Err(_) => Err("A CLI demorou para listar os modelos.".into()),
                        };
                        emit(Update { models: Some((provider, result)), ..Default::default() });
                    });
                }
                Command::Cancel | Command::Ask { .. } => {
                    if let Some(task) = active.take() {
                        task.abort();
                        let _ = task.await;
                    }
                    pending.lock().unwrap_or_else(PoisonError::into_inner).take();
                    if let Command::Ask { generation, provider, model, items } = command {
                        let (emit, key, pending) = (emit.clone(), key.clone(), pending.clone());
                        active = Some(tokio::spawn(async move {
                            let (text, error) = converse(generation, provider, &model, items, key.as_deref(), &pending, &*emit).await;
                            let error = error.map(|s| redact(s, key.as_deref()));
                            emit(Update { generation, text, done: true, error, ..Default::default() });
                        }));
                    }
                }
            }
        }
        if let Some(task) = active {
            task.abort();
            let _ = task.await;
        }
    });
    tx
}

/// Port of isle's agent loop (agent.rs): the AI answers; a command it asks for waits for Executar, runs,
/// and its result goes back to it, until it answers without one. The shown text: a line per command
/// (✓ ran, ✗ failed or denied), then what it says.
async fn converse(
    generation: u64,
    provider: AiProviderKind,
    model: &str,
    items: Vec<Message>,
    key: Option<&str>,
    pending: &Pending,
    emit: &(dyn Fn(Update) + Send + Sync),
) -> (String, Option<String>) {
    let mut talk = items;
    let mut steps = String::new();
    let shown = |steps: &str, say: &str| if steps.is_empty() { say.to_owned() } else { format!("{steps}\n{say}").trim_end().to_owned() };
    for step in 0..=agent::MAX_STEPS {
        let mut latest = String::new();
        let answer = tokio::time::timeout(ASK_TIMEOUT, backends::ask(provider, model, &talk, key, |raw| {
            latest = shown(&steps, &agent::partial_say(raw));
            emit(Update { generation, text: latest.clone(), ..Default::default() });
        })).await;
        let raw = match answer {
            Ok(Ok(raw)) => raw,
            Ok(Err(err)) => return (latest, Some(format!("{err:#}"))),
            Err(_) => return (latest, Some("O assistente demorou mais de 3 minutos. Tente novamente.".into())),
        };
        let reply = agent::parse(&raw);
        talk.push(Message { user: false, text: raw });
        let Some(command) = reply.run.filter(|_| step < agent::MAX_STEPS) else {
            return (shown(&steps, &reply.say), None);
        };
        // Never offered when it cannot be shown whole and plainly (the card's and Enter's rule too).
        if !isle_core::chat::showable(&command.command) {
            steps.push_str(&format!("✗ {} (recusado: longo demais ou com caracteres escondidos)\n", command.title));
            talk.push(Message { user: true, text: agent::UNSHOWABLE.into() });
            continue;
        }
        // Executar or Negar: nothing runs without it; unanswered for long, it is denied.
        let (answer, wait) = oneshot::channel();
        *pending.lock().unwrap_or_else(PoisonError::into_inner) = Some((generation, answer));
        emit(Update { generation, text: shown(&steps, &reply.say), approval: Some(command.clone()), ..Default::default() });
        let approved = matches!(tokio::time::timeout(agent::APPROVAL, wait).await, Ok(Ok(true)));
        pending.lock().unwrap_or_else(PoisonError::into_inner).take();
        if !approved {
            steps.push_str(&format!("✗ {} (negado)\n", command.title));
            talk.push(Message { user: true, text: agent::DENIED.into() });
            continue;
        }
        emit(Update { generation, text: shown(&steps, &format!("⋯ {}", command.title)), ..Default::default() });
        let (code, output) = match agent::run(&command.command).await {
            Ok(done) => done,
            Err(err) => (None, format!("Não foi possível rodar: {err:#}")),
        };
        steps.push_str(&format!("{} {}\n", if code == Some(0) { "✓" } else { "✗" }, command.title));
        talk.push(Message { user: true, text: agent::result_message(&command, code, &output) });
    }
    (steps, None)
}

#[cfg(test)]
mod tests {
    #[test]
    fn idle_and_cancel_without_a_turn_emit_nothing() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = super::spawn(&rt, None, move |u| {
            tx.send(u).unwrap();
        });
        worker.send(super::Command::Cancel).unwrap();
        worker.send(super::Command::Approve { generation: 1, run: true }).unwrap();
        assert!(matches!(
            rx.recv_timeout(std::time::Duration::from_millis(40)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        drop(worker);
    }
    #[test]
    fn claude_lists_its_three_aliases_without_a_cli() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = super::spawn(&rt, None, move |u| { tx.send(u).unwrap(); });
        worker.send(super::Command::Models(isle_core::state::AiProviderKind::ClaudeCode)).unwrap();
        let update = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        let (_, models) = update.models.unwrap();
        assert_eq!(models.unwrap().iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["opus", "sonnet", "haiku"]);
    }
}
