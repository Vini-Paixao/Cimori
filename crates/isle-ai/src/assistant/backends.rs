// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted from zickrian/isle @ a29a7c4 assistant/{claude,codex,opencode,child}.rs.
// Copyright (C) 2026 zickrian. Windows text-only port: process per bounded transcript,
// no action schema, no voice, private application workspace and hidden child jobs.
use super::{
    process::{self, Child},
    protocol::{self, Stream},
};
use anyhow::{Context, Result, bail, ensure};
use isle_core::{chat::Message, state::AiProviderKind};
use serde_json::{Value, json};
use std::{path::PathBuf, process::Stdio};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout, Command},
};

use super::agent::INSTRUCTIONS;
use isle_core::chat::ModelChoice;

/// Claude Code's aliases offered, in order (isle's CLAUDE_ALIASES; never Fable).
pub const CLAUDE_MODELS: [(&str, &str); 3] = [("opus", "Opus"), ("sonnet", "Sonnet"), ("haiku", "Haiku")];

pub fn transcript(items: &[Message]) -> String {
    // JSON keeps message boundaries unambiguous, including quotes and role-like text in user input.
    serde_json::to_string(
        &items
            .iter()
            .map(|m| json!({"role":if m.user {"user"} else {"assistant"},"content":m.text}))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default()
}

fn data() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(isle_core::data_dir::NAME).join("assistant")
}
fn user_dir(env: &str, fallback: &str) -> PathBuf {
    std::env::var_os(env)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::home_dir().unwrap_or_default().join(fallback))
}

/// Isolated Codex settings, sharing only file credentials. A refreshed token is handed back only
/// if the user's sign-in has not changed meanwhile (same intent as upstream sign_in::share).
struct SignIn {
    user: PathBuf,
    local: PathBuf,
    before: Option<Vec<u8>>,
}
impl SignIn {
    fn codex(home: &std::path::Path) -> Result<Self> {
        let user = user_dir("CODEX_HOME", ".codex").join("auth.json");
        let local = home.join("auth.json");
        Self::from_paths(user, local)
    }
    fn from_paths(user: PathBuf, local: PathBuf) -> Result<Self> {
        let before = std::fs::read(&user).ok();
        if let Some(bytes) = &before {
            std::fs::write(&local, bytes)?;
        } else if local.exists() {
            std::fs::remove_file(&local)?;
        }
        Ok(Self {
            user,
            local,
            before,
        })
    }
}

/// Claude's login-helper settings may contain secrets; keep the temporary handoff only for the turn.
struct LoginFile(PathBuf);
impl Drop for LoginFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
impl Drop for SignIn {
    fn drop(&mut self) {
        if let (Some(before), Ok(renewed)) = (&self.before, std::fs::read(&self.local)) {
            if renewed != *before && std::fs::read(&self.user).ok().as_ref() == Some(before) {
                let result = std::str::from_utf8(&renewed)
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
                    .and_then(|text| crate::agents::file::write_atomic(&self.user, text));
                if let Err(err) = result {
                    tracing::warn!("Não foi possível devolver login renovado do Codex: {err}");
                }
            }
        }
        let _ = std::fs::remove_file(&self.local);
    }
}

struct Pipe {
    // Dropped before the login hand-back, even on cancellation.
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: tokio::task::JoinHandle<()>,
    tail: std::sync::Arc<std::sync::Mutex<String>>,
}
impl Pipe {
    fn start(mut cmd: Command) -> Result<Self> {
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = Child::spawn(&mut cmd)?;
        let stdin = child.process.stdin.take().context("CLI sem stdin")?;
        let stdout = BufReader::new(child.process.stdout.take().context("CLI sem stdout")?);
        let mut errors = BufReader::new(child.process.stderr.take().context("CLI sem stderr")?);
        let tail = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let kept = tail.clone();
        let stderr = tokio::spawn(async move {
            while let Ok(Some(line)) = process::line(&mut errors).await {
                let mut tail = kept
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *tail = line.chars().take(600).collect();
            }
        });
        Ok(Self {
            child,
            stdin,
            stdout,
            stderr,
            tail,
        })
    }
    async fn write(&mut self, value: Value) -> Result<()> {
        self.stdin
            .write_all(format!("{value}\n").as_bytes())
            .await
            .context("CLI encerrou a entrada")?;
        Ok(())
    }
    async fn next(&mut self) -> Result<Option<Value>> {
        while let Some(line) = process::line(&mut self.stdout).await? {
            if line.trim().is_empty() {
                continue;
            }
            return Ok(Some(
                serde_json::from_str(&line).context("Evento JSON inválido do CLI")?,
            ));
        }
        Ok(None)
    }
    async fn request(&mut self, id: u64, method: &str, params: Value) -> Result<Value> {
        self.write(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        while let Some(message) = self.next().await? {
            if message["id"] == id && message.get("method").is_none() {
                ensure!(
                    message.get("error").is_none(),
                    "{}",
                    protocol::reason(&message)
                );
                return Ok(message["result"].clone());
            }
            self.deny(&message).await?;
        }
        bail!("Codex encerrou antes de responder ({})", self.error())
    }
    async fn deny(&mut self, event: &Value) -> Result<()> {
        if event.get("id").is_some() && event.get("method").is_some() {
            self.write(json!({"jsonrpc":"2.0","id":event["id"],"error":{"code":-32601,"message":"Isle only answers"}})).await?;
        }
        Ok(())
    }
    fn error(&self) -> String {
        self.tail
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    async fn finish(&mut self) -> Result<()> {
        let status = self.child.process.wait().await?;
        let _ = (&mut self.stderr).await;
        ensure!(
            status.success(),
            "CLI encerrou com {status}: {}",
            self.error()
        );
        Ok(())
    }
}
impl Drop for Pipe {
    fn drop(&mut self) {
        self.stderr.abort();
    }
}

fn claude_settings(settings: &Value) -> Value {
    let mut kept = serde_json::Map::new();
    for key in [
        "env",
        "apiKeyHelper",
        "awsAuthRefresh",
        "awsCredentialExport",
        "forceLoginMethod",
    ] {
        if let Some(value) = settings.get(key) {
            kept.insert(key.into(), value.clone());
        }
    }
    Value::Object(kept)
}

/// One answer of the AI over `items`, with `model` (empty: the CLI's own default); `draft` gets the
/// raw text as it streams.
pub async fn ask(
    provider: AiProviderKind,
    model: &str,
    items: &[Message],
    key: Option<&str>,
    mut draft: impl FnMut(&str),
) -> Result<String> {
    let workspace = data().join("workspace");
    std::fs::create_dir_all(&workspace)?;
    let prompt = transcript(items);
    let mut stream = Stream::default();
    // This guard precedes Pipe, so the CLI/job dies before credentials are handed back on drop.
    let mut _sign_in = None;
    let mut _login_file = None;
    let mut cmd = match provider {
        AiProviderKind::ClaudeCode => {
            let mut cmd = process::command("claude")?;
            cmd.args([
                "-p",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--verbose",
                "--include-partial-messages",
                // Search only, as the Codex and OpenCode backends: WebFetch (upstream isle's too)
                // would let a fetched page steer the chat into sending the conversation to a URL.
                "--tools",
                "WebSearch",
                "--allowedTools",
                "WebSearch",
                "--setting-sources",
                "",
                "--strict-mcp-config",
                "--system-prompt",
                INSTRUCTIONS,
            ]);
            // Preserve login helpers/cloud env from settings, excluding hooks, projects and MCP.
            let file = data().join("claude-sign-in.json");
            let settings: Value =
                std::fs::read(user_dir("CLAUDE_CONFIG_DIR", ".claude").join("settings.json"))
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .unwrap_or_default();
            std::fs::write(&file, claude_settings(&settings).to_string())?;
            cmd.arg("--settings").arg(&file);
            if !model.is_empty() {
                cmd.arg("--model").arg(model);
            }
            _login_file = Some(LoginFile(file));
            cmd
        }
        AiProviderKind::Codex => {
            let (cmd, sign_in) = codex_command()?;
            _sign_in = Some(sign_in);
            cmd
        }
        AiProviderKind::OpenCodeGo => opencode(&workspace, key, model).await?,
    };
    cmd.current_dir(&workspace);
    let mut pipe = Pipe::start(cmd)?;
    match provider {
        AiProviderKind::ClaudeCode => {
            // Keep reading even after a broken pipe: the CLI may have left its login error on stdout.
            let _ = pipe.write(protocol::claude_request(&prompt)).await;
            let _ = pipe.stdin.shutdown().await;
        }
        AiProviderKind::Codex => {
            pipe.request(
                1,
                "initialize",
                json!({"clientInfo":{"name":"Cimori","version":env!("CARGO_PKG_VERSION")}}),
            )
            .await?;
            pipe.write(json!({"jsonrpc":"2.0","method":"initialized"}))
                .await?;
            let mut start = json!({"cwd":workspace,"approvalPolicy":"never","sandbox":"read-only","baseInstructions":INSTRUCTIONS});
            if !model.is_empty() {
                start["model"] = model.into();
            }
            let thread = pipe.request(2,"thread/start",start).await?;
            let id = thread["thread"]["id"]
                .as_str()
                .context("Codex não abriu conversa")?;
            // The turn/start response can be interleaved with deltas; read both below.
            pipe.write(json!({"jsonrpc":"2.0","id":3,"method":"turn/start","params":protocol::codex_turn(id,&prompt)})).await?;
        }
        AiProviderKind::OpenCodeGo => {
            pipe.stdin
                .write_all(format!("{INSTRUCTIONS}\n\nConversa (JSON):\n{prompt}").as_bytes())
                .await?;
            pipe.stdin.shutdown().await?;
        }
    }
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    while let Some(event) = pipe.next().await? {
        if provider == AiProviderKind::Codex {
            if event["id"] == 3 && event.get("error").is_some() {
                bail!("{}", protocol::reason(&event));
            }
            pipe.deny(&event).await?;
        }
        let changed = stream.event(provider, &event)?;
        if changed && last.elapsed().as_millis() >= 30 {
            draft(&stream.text);
            last = std::time::Instant::now();
        }
        if stream.done {
            break;
        }
    }
    if provider == AiProviderKind::OpenCodeGo {
        pipe.finish().await?;
    } else {
        ensure!(stream.done, "CLI parou antes de concluir: {}", pipe.error());
    }
    ensure!(
        !stream.text.trim().is_empty(),
        "O assistente não retornou texto: {}",
        pipe.error()
    );
    Ok(stream.text)
}

/// Codex's app-server, isolated (upstream's tool suppression, read-only), with the user's sign-in.
fn codex_command() -> Result<(Command, SignIn)> {
    let home = data().join("codex");
    std::fs::create_dir_all(&home)?;
    let sign_in = SignIn::codex(&home)?;
    // CONFIG is upstream's tool/feature suppression, kept verbatim in codex-config.toml.
    std::fs::write(home.join("config.toml"), include_str!("codex-config.toml"))?;
    let mut cmd = process::command("codex")?;
    cmd.args(["app-server", "-c", "sandbox_mode=\"read-only\""])
        .env("CODEX_HOME", home);
    Ok((cmd, sign_in))
}

/// The models a provider's CLI offers now: Claude Code's aliases, Codex's `model/list` (its pages; hidden
/// ones left out, as isle's codex_catalog), OpenCode Go's from `opencode models`.
pub async fn models(provider: AiProviderKind, key: Option<&str>) -> Result<Vec<ModelChoice>> {
    let choice = |id: &str, name: &str| ModelChoice { id: id.into(), name: name.into() };
    match provider {
        AiProviderKind::ClaudeCode => Ok(CLAUDE_MODELS.iter().map(|(id, name)| choice(id, name)).collect()),
        AiProviderKind::Codex => {
            let (mut cmd, _sign_in) = codex_command()?;
            cmd.current_dir(data());
            let mut pipe = Pipe::start(cmd)?;
            pipe.request(1, "initialize", json!({"clientInfo":{"name":"Cimori","version":env!("CARGO_PKG_VERSION")}})).await?;
            pipe.write(json!({"jsonrpc":"2.0","method":"initialized"})).await?;
            let (mut cursor, mut pages) = (Value::Null, Vec::new());
            for id in 2..12 {
                let page = pipe.request(id, "model/list", json!({"cursor": cursor, "includeHidden": false})).await?;
                cursor = page["nextCursor"].clone();
                pages.push(page);
                if cursor.is_null() {
                    break;
                }
            }
            Ok(codex_models(&pages))
        }
        AiProviderKind::OpenCodeGo => {
            let key = key.filter(|k| !k.trim().is_empty()).context("Coloque a chave do OpenCode Go em Configurações → IA")?;
            let mut list = go_command(key)?;
            list.arg("models");
            let listed = tokio::time::timeout(std::time::Duration::from_secs(30), listing(list))
                .await
                .context("OpenCode demorou para listar modelos")??;
            Ok(go_models(&listed).into_iter().map(|id| choice(id, id.trim_start_matches("opencode-go/"))).collect())
        }
    }
}

fn codex_models(pages: &[Value]) -> Vec<ModelChoice> {
    pages.iter().flat_map(|page| page["data"].as_array().into_iter().flatten())
        .filter(|m| m["hidden"] != true)
        .filter_map(|m| {
            let id = m["model"].as_str().or(m["id"].as_str())?;
            let name = m["displayName"].as_str().filter(|n| !n.is_empty()).unwrap_or(id);
            Some(ModelChoice { id: id.into(), name: name.into() })
        })
        .collect()
}

fn opencode_config() -> Value {
    // Upstream's deny list: config merging must never re-enable an earlier allow entry.
    let mut permission = json!({"*":"deny","websearch":"allow","read":{"*":"deny","/nonexistent/isle-reads-nothing":"allow"}});
    for name in [
        "bash",
        "edit",
        "write",
        "patch",
        "apply_patch",
        "multiedit",
        "glob",
        "grep",
        "list",
        "webfetch",
        "codesearch",
        "task",
        "todowrite",
        "todoread",
        "skill",
        "lsp",
        "question",
        "external_directory",
        "doom_loop",
        "plan_enter",
        "plan_exit",
        "workflow_tool_approval",
        "execute",
        "batch",
    ] {
        permission[name] = json!("deny");
    }
    json!({"autoupdate":false,"share":"disabled","snapshot":false,"mcp":{},"plugin":[],"instructions":[],"permission":permission,
        "agent":{"isle":{"mode":"primary","description":"Isle text assistant","prompt":INSTRUCTIONS,"steps":8,"permission":permission}}})
}
fn go_command(key: &str) -> Result<Command> {
    let mut cmd = process::command("opencode")?;
    // Preserve Windows/Node runtime and proxy variables; no inherited provider keys or configs.
    let keep: Vec<_> = std::env::vars_os()
        .filter(|(n, _)| {
            matches!(
                n.to_string_lossy().to_ascii_uppercase().as_str(),
                "PATH"
                    | "SYSTEMROOT"
                    | "WINDIR"
                    | "TEMP"
                    | "TMP"
                    | "COMSPEC"
                    | "PATHEXT"
                    | "APPDATA"
                    | "LOCALAPPDATA"
                    | "LANG"
                    | "TZ"
                    | "SSL_CERT_FILE"
                    | "SSL_CERT_DIR"
                    | "NODE_EXTRA_CA_CERTS"
                    | "HTTP_PROXY"
                    | "HTTPS_PROXY"
                    | "ALL_PROXY"
                    | "NO_PROXY"
            )
        })
        .collect();
    cmd.env_clear().envs(keep);
    let home = data().join("opencode");
    for (name, folder) in [
        ("HOME", "home"),
        ("USERPROFILE", "home"),
        ("APPDATA", "config"),
        ("LOCALAPPDATA", "data"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        let path = home.join(folder);
        std::fs::create_dir_all(&path)?;
        cmd.env(name, path);
    }
    for name in [
        "OPENCODE_DISABLE_PROJECT_CONFIG",
        "OPENCODE_PURE",
        "OPENCODE_DISABLE_CLAUDE_CODE",
        "OPENCODE_DISABLE_EXTERNAL_SKILLS",
        "OPENCODE_DISABLE_AUTOUPDATE",
        "OPENCODE_DISABLE_LSP_DOWNLOAD",
        "OPENCODE_DISABLE_SHARE",
        "OPENCODE_EXPERIMENTAL_DISABLE_FILEWATCHER",
        "OPENCODE_ENABLE_EXA",
    ] {
        cmd.env(name, "1");
    }
    cmd.env(
        "OPENCODE_AUTH_CONTENT",
        json!({"opencode-go":{"type":"api","key":key}}).to_string(),
    );
    let config = home.join("config/opencode/opencode.json");
    std::fs::create_dir_all(config.parent().unwrap())?;
    std::fs::write(&config, opencode_config().to_string())?;
    cmd.env("OPENCODE_CONFIG", config);
    Ok(cmd)
}
fn version(text: &str) -> Option<[u32; 3]> {
    let word = text.split_whitespace().find(|w| {
        w.trim_start_matches('v')
            .starts_with(|c: char| c.is_ascii_digit())
    })?;
    let mut parts = word.trim_start_matches('v').split('.');
    Some([
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts
            .next()
            .unwrap_or("0")
            .split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok()?,
    ])
}
fn go_models(list: &str) -> Vec<&str> {
    list.lines()
        .map(str::trim)
        .filter(|s| s.starts_with("opencode-go/") && !s.contains(char::is_whitespace))
        .collect()
}
fn go_model(list: &str) -> Option<&str> {
    go_models(list).into_iter().next()
}
async fn listing(mut cmd: Command) -> Result<String> {
    let mut pipe = Pipe::start({
        cmd.current_dir(data());
        cmd
    })?;
    pipe.stdin.shutdown().await?;
    let mut text = String::new();
    while let Some(line) = process::line(&mut pipe.stdout).await? {
        ensure!(
            text.len() + line.len() < process::MAX_LINE,
            "Lista de modelos muito grande"
        );
        text.push_str(&line);
        text.push('\n');
    }
    pipe.finish().await?;
    Ok(text)
}
async fn opencode(workspace: &std::path::Path, key: Option<&str>, chosen: &str) -> Result<Command> {
    let key = key
        .filter(|k| !k.trim().is_empty())
        .context("Coloque a chave do OpenCode Go em Configurações → IA")?;
    let mut check = go_command(key)?;
    check.arg("--version");
    let text = tokio::time::timeout(std::time::Duration::from_secs(10), listing(check))
        .await
        .context("OpenCode demorou para iniciar")??;
    let version = version(&text).context("Versão do OpenCode desconhecida")?;
    ensure!(
        version >= [1, 4, 8],
        "Atualize o OpenCode para 1.4.8 ou posterior"
    );
    let mut models = go_command(key)?;
    models.arg("models").arg(if version[0] >= 2 {
        "--standalone"
    } else {
        "--verbose"
    });
    let listed = tokio::time::timeout(std::time::Duration::from_secs(30), listing(models))
        .await
        .context("OpenCode demorou para listar modelos")??;
    // The chosen one while OpenCode still lists it, else the first.
    let model = go_models(&listed).into_iter().find(|m| *m == chosen).or_else(|| go_model(&listed))
        .context("O CLI não listou modelos opencode-go para este login")?;
    let mut cmd = go_command(key)?;
    cmd.arg("run");
    if version[0] >= 2 {
        cmd.arg("--standalone");
    }
    cmd.args([
        "--format",
        "json",
        "--agent",
        "isle",
        "--title",
        "Isle",
        "--print-logs",
        "--log-level",
        if version[0] >= 2 { "error" } else { "ERROR" },
    ]);
    cmd.arg(format!("--model={model}")).current_dir(workspace);
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renewed_credentials_return_only_to_the_unchanged_login() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ai-tests/sign-in");
        std::fs::create_dir_all(&dir).unwrap();
        let user = dir.join("user.json");
        let local = dir.join("local.json");
        std::fs::write(&user, "old login").unwrap();
        let login = SignIn::from_paths(user.clone(), local.clone()).unwrap();
        std::fs::write(&local, "renewed login").unwrap();
        drop(login);
        assert_eq!(std::fs::read_to_string(&user).unwrap(), "renewed login");
        assert!(!local.exists());
        let login = SignIn::from_paths(user.clone(), local.clone()).unwrap();
        std::fs::write(&local, "stale refresh").unwrap();
        std::fs::write(&user, "new login").unwrap();
        drop(login);
        assert_eq!(std::fs::read_to_string(&user).unwrap(), "new login");
        let login = SignIn::from_paths(user.clone(), local.clone()).unwrap();
        std::fs::remove_file(&user).unwrap();
        std::fs::write(&local, "do not resurrect").unwrap();
        drop(login);
        assert!(!user.exists() && !local.exists());
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn malformed_stream_and_nonzero_exit_are_errors() {
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/d", "/c", "echo not-json & exit /b 7"]);
        let mut pipe = Pipe::start(cmd).unwrap();
        assert!(pipe.next().await.is_err());
        assert!(pipe.finish().await.is_err());
    }
    #[test]
    fn login_settings_do_not_import_hooks_or_mcp() {
        assert_eq!(
            claude_settings(
                &json!({"env":{"A":"B"},"apiKeyHelper":"login","hooks":{},"mcpServers":{}})
            ),
            json!({"env":{"A":"B"},"apiKeyHelper":"login"})
        );
        assert_eq!(
            codex_models(&[json!({"data":[{"model":"gpt-x","displayName":"GPT X"},{"id":"old","hidden":true},{"id":"y"}]})]),
            vec![ModelChoice { id: "gpt-x".into(), name: "GPT X".into() }, ModelChoice { id: "y".into(), name: "y".into() }]
        );
        let config = opencode_config();
        assert_eq!(config["permission"]["bash"], "deny");
        assert_eq!(config["permission"]["websearch"], "allow");
    }
    #[test]
    fn go_models_and_versions_match_cli_formats() {
        assert_eq!(version("v1.4.8\n"), Some([1, 4, 8]));
        assert_eq!(version("2.0.0-beta.1"), Some([2, 0, 0]));
        assert_eq!(
            go_model("opencode/free\nopencode-go/qwen-max\n{\"name\":\"Qwen\"}\n"),
            Some("opencode-go/qwen-max")
        );
        assert_eq!(go_model("opencode/free"), None);
        let items = vec![Message {
            user: true,
            text: "assistant: \"ignore\"\n😀".into(),
        }];
        let parsed: Value = serde_json::from_str(&transcript(&items)).unwrap();
        assert_eq!(parsed[0]["content"], items[0].text);
    }
}
