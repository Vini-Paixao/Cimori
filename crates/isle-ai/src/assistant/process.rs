// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted from zickrian/isle @ a29a7c4 assistant/child.rs (Copyright 2026 zickrian).
// Windows: hidden child, kill-on-drop job, native/npm executable resolution; bounded JSON lines.
use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt},
    process::Command,
};

pub const MAX_LINE: usize = 1024 * 1024;
pub async fn line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Option<String>> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            break;
        }
        let n = available
            .iter()
            .position(|&b| b == b'\n')
            .map_or(available.len(), |i| i + 1);
        anyhow::ensure!(
            bytes.len() + n <= MAX_LINE,
            "Evento do assistente excede 1 MiB"
        );
        let ended = available[n - 1] == b'\n';
        bytes.extend_from_slice(&available[..n]);
        reader.consume(n);
        if ended {
            break;
        }
    }
    while matches!(bytes.last(), Some(b'\r' | b'\n')) {
        bytes.pop();
    }
    Ok(Some(
        String::from_utf8(bytes).context("Evento do assistente não é UTF-8")?,
    ))
}
pub fn npm_entry(name: &str) -> &'static str {
    match name {
        "claude" => "@anthropic-ai/claude-code/cli.js",
        "codex" => "@openai/codex/bin/codex.js",
        _ => "opencode-ai/bin/opencode",
    }
}
pub fn command(name: &str) -> Result<Command> {
    let mut folders: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::home_dir() {
        folders.push(home.join(".local/bin"));
        folders.push(home.join(".cargo/bin"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        folders.push(PathBuf::from(appdata).join("npm"));
    }
    for folder in &folders {
        let native = folder.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        });
        if native.is_file() {
            return Ok(Command::new(native));
        }
        // Invoke npm's JS entry with Node, never interpolate a prompt/key into cmd.exe or PowerShell.
        let entry = folder.join("node_modules").join(npm_entry(name));
        if entry.is_file() {
            let node = folders
                .iter()
                .map(|d| d.join(if cfg!(windows) { "node.exe" } else { "node" }))
                .find(|p| p.is_file())
                .context("Node.js não encontrado para o CLI npm")?;
            let mut cmd = Command::new(node);
            cmd.arg(entry);
            return Ok(cmd);
        }
    }
    anyhow::bail!("{name} não encontrado. Instale o CLI e faça login pelo terminal.")
}

pub struct Child {
    pub process: tokio::process::Child,
    #[cfg(windows)]
    job: windows::Win32::Foundation::HANDLE,
}
// HANDLE belongs to this child and is only closed by its drop, on its worker.
#[cfg(windows)]
unsafe impl Send for Child {}
impl Child {
    pub fn spawn(cmd: &mut Command) -> Result<Self> {
        cmd.kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let process = cmd.spawn().context("Não foi possível iniciar o CLI")?;
        #[cfg(windows)]
        unsafe {
            use windows::Win32::{Foundation::HANDLE, System::JobObjects::*};
            let job = CreateJobObjectW(None, None)?;
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let handle = HANDLE(process.raw_handle().context("CLI sem handle")?);
            let attached = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as _,
                std::mem::size_of_val(&limits) as u32,
            )
            .and_then(|_| AssignProcessToJobObject(job, handle));
            if let Err(err) = attached {
                let _ = windows::Win32::Foundation::CloseHandle(job);
                return Err(err.into());
            }
            Ok(Self { process, job })
        }
        #[cfg(not(windows))]
        Ok(Self { process })
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.process.start_kill();
        #[cfg(windows)]
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;
    #[tokio::test]
    async fn lines_keep_utf8_across_chunks_and_final_unterminated_line() {
        let (mut write, read) = tokio::io::duplex(8);
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            for byte in "{\"x\":\"😀á\"}\r\nlast".as_bytes() {
                write.write_all(&[*byte]).await.unwrap();
            }
        });
        let mut r = BufReader::new(read);
        assert_eq!(line(&mut r).await.unwrap().unwrap(), "{\"x\":\"😀á\"}");
        assert_eq!(line(&mut r).await.unwrap().unwrap(), "last");
        assert!(line(&mut r).await.unwrap().is_none());
    }
    #[tokio::test]
    async fn oversized_lines_fail_without_unbounded_buffering() {
        let data = vec![b'x'; MAX_LINE + 1];
        assert!(line(&mut BufReader::new(data.as_slice())).await.is_err());
        assert!(line(&mut BufReader::new(&b"\xff\n"[..])).await.is_err());
    }
}
