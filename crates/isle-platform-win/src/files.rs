//! Windows' own open and save dialogs (the settings' backup). Modal: call from a thread of its own.

/// The file the user picked (`save`: where to write, suggesting `default_name`), None if cancelled.
#[cfg(windows)]
pub fn pick_backup_file(save: bool, default_name: &str) -> Option<std::path::PathBuf> {
    pick_file(Some((save, default_name)), false)
}

/// Selects any local file for the shelf.
#[cfg(windows)]
pub fn pick_shelf_file() -> Option<std::path::PathBuf> {
    pick_file(None, false)
}

#[cfg(windows)]
fn pick_file(backup: Option<(bool, &str)>, folder: bool) -> Option<std::path::PathBuf> {
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::*;
    use windows::core::{HSTRING, Interface, w};
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().ok()?;
        let picked = (|| -> Option<std::path::PathBuf> {
            let dialog: IFileDialog = if backup.is_some_and(|(save, _)| save) {
                CoCreateInstance::<_, IFileSaveDialog>(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?.cast().ok()?
            } else {
                CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?.cast().ok()?
            };
            dialog.SetOptions(dialog.GetOptions().ok()? | FOS_FORCEFILESYSTEM | if folder {FOS_PICKFOLDERS} else {FILEOPENDIALOGOPTIONS(0)}).ok()?;
            if let Some((save, default_name)) = backup {
                let name = windows::core::HSTRING::from(isle_core::i18n::tr("files.backup_filter"));
                let filter = [COMDLG_FILTERSPEC { pszName: windows::core::PCWSTR(name.as_ptr()), pszSpec: w!("*.json") }];
                dialog.SetFileTypes(&filter).ok()?;
                dialog.SetDefaultExtension(w!("json")).ok()?;
                if save {
                    dialog.SetFileName(&HSTRING::from(default_name)).ok()?;
                }
            }
            dialog.Show(None).ok()?;
            let item = dialog.GetResult().ok()?;
            let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let path = name.to_string().ok();
            CoTaskMemFree(Some(name.0 as *const _));
            path.map(std::path::PathBuf::from)
        })();
        CoUninitialize();
        picked
    }
}

#[cfg(not(windows))]
pub fn pick_backup_file(_save: bool, _default_name: &str) -> Option<std::path::PathBuf> {
    None
}

#[cfg(not(windows))]
pub fn pick_shelf_file() -> Option<std::path::PathBuf> {
    None
}

// Port of the icon folder picker (mac-island 3.1.1): Windows' own folder dialog.
pub fn pick_icon_folder() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    { pick_file(None, true) }
    #[cfg(not(windows))]
    { None }
}

/// Opens a folder, a file or a link the way Explorer does.
pub fn shell_open(target: &str) {
    if let Err(err) = std::process::Command::new("explorer.exe").arg(target).spawn() {
        tracing::warn!("abrir {target}: {err}");
    }
}

/// Opens `path`'s folder in Explorer with it selected.
pub fn shell_select(path: &std::path::Path) {
    let mut command = std::process::Command::new("explorer.exe");
    // Explorer reads /select, and the quoted path as one argument, not as Rust would quote it.
    #[cfg(windows)]
    std::os::windows::process::CommandExt::raw_arg(&mut command, format!("/select,\"{}\"", path.display()));
    if let Err(err) = command.spawn() {
        tracing::warn!("mostrar {}: {err}", path.display());
    }
}

/// Windows' Run key for this user (HKCU\Software\Microsoft\Windows\CurrentVersion\Run, value "Cimori"):
/// Cimori opens at sign-in.
#[cfg(windows)]
pub mod autostart {
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::*;
    use windows::core::{HSTRING, PCWSTR, w};

    const KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
    const NAME: PCWSTR = w!("Cimori");

    /// The command the value holds: the exe's path, quoted.
    pub fn command(exe: &str) -> String {
        format!("\"{exe}\"")
    }

    /// The value now, None when there is none.
    pub fn get() -> Option<String> {
        let mut value = [0u16; 1024];
        let mut size = std::mem::size_of_val(&value) as u32;
        let status = unsafe { RegGetValueW(HKEY_CURRENT_USER, KEY, NAME, RRF_RT_REG_SZ, None, Some(value.as_mut_ptr().cast()), Some(&mut size)) };
        (status == ERROR_SUCCESS).then(|| String::from_utf16_lossy(&value[..value.iter().position(|&c| c == 0).unwrap_or(value.len())]))
    }

    pub fn set(command: &str) -> windows::core::Result<()> {
        let text = HSTRING::from(command);
        let bytes = (text.len() + 1) * 2;
        unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, KEY, NAME, REG_SZ.0, Some(text.as_ptr().cast()), bytes as u32) }.ok()
    }

    pub fn remove() -> windows::core::Result<()> {
        match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, KEY, NAME) } {
            ERROR_FILE_NOT_FOUND => Ok(()),
            status => status.ok(),
        }
    }
}

/// SHA-256 of `bytes` as lowercase hex, by Windows' own BCrypt.
#[cfg(windows)]
pub fn sha256_hex(bytes: &[u8]) -> Option<String> {
    use windows::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};
    let mut digest = [0u8; 32];
    let status = unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, bytes, &mut digest) };
    status.is_ok().then(|| digest.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(not(windows))]
pub fn sha256_hex(_: &[u8]) -> Option<String> {
    None
}

#[cfg(all(test, windows))]
mod sha_tests {
    #[test]
    fn hashes_as_sha256sum_does() {
        assert_eq!(super::sha256_hex(b"abc").as_deref(), Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
    }
}
