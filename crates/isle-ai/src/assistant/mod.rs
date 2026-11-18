// SPDX-License-Identifier: GPL-3.0-or-later
// Ported from zickrian/isle @ a29a7c4 assistant (Copyright 2026 zickrian).
mod backends;
mod process;
mod protocol;

use isle_core::{chat::Message, state::AiProviderKind};
use tokio::sync::mpsc;

pub enum Command {
    Ask {
        generation: u64,
        provider: AiProviderKind,
        items: Vec<Message>,
    },
    Cancel,
    /// A new OpenCode Go key, for the next request.
    Key(Option<String>),
}
#[derive(Debug)]
pub struct Update {
    pub generation: u64,
    pub text: String,
    pub done: bool,
    pub error: Option<String>,
}

/// No CLI, timer, model query or polling until a request arrives. All I/O runs on rt's workers.
pub fn spawn(
    rt: &tokio::runtime::Runtime,
    mut key: Option<String>,
    emit: impl Fn(Update) + Send + Sync + 'static,
) -> mpsc::UnboundedSender<Command> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let emit = std::sync::Arc::new(emit);
    rt.spawn(async move {
        let mut active: Option<tokio::task::JoinHandle<()>> = None;
        while let Some(command) = rx.recv().await {
            if let Command::Key(k) = command {
                key = k;
                continue;
            }
            if let Some(task) = active.take() {
                task.abort();
                let _ = task.await;
            }
            if let Command::Ask {
                generation,
                provider,
                items,
            } = command
            {
                let (emit, key) = (emit.clone(), key.clone());
                active = Some(tokio::spawn(async move {
                    let mut latest = String::new();
                    let result = tokio::time::timeout(
                        std::time::Duration::from_secs(180),
                        backends::ask(provider, &items, key.as_deref(), |text| {
                            latest = text.into();
                            emit(Update {
                                generation,
                                text: text.into(),
                                done: false,
                                error: None,
                            });
                        }),
                    )
                    .await;
                    let (text, error) = match result {
                        Ok(Ok(text)) => (text, None),
                        Ok(Err(err)) => (latest, Some(format!("{err:#}"))),
                        Err(_) => (
                            latest,
                            Some("O assistente demorou mais de 3 minutos. Tente novamente.".into()),
                        ),
                    };
                    let error = error.map(|s| {
                        let redacted = match key.as_deref().filter(|k| !k.is_empty()) {
                            Some(k) => s.replace(k, "[chave]"),
                            None => s,
                        };
                        redacted.chars().take(1200).collect()
                    });
                    emit(Update {
                        generation,
                        text,
                        done: true,
                        error,
                    });
                }));
            }
        }
        if let Some(task) = active {
            task.abort();
            let _ = task.await;
        }
    });
    tx
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
        assert!(matches!(
            rx.recv_timeout(std::time::Duration::from_millis(40)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        drop(worker);
    }
}
