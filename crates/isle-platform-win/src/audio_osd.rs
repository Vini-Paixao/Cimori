// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Adapted for isle-win from Mac Island & Dock by caliberda v3.1.0 (GetVolume,
// StatusEvents, ReadMixerApps, SetAppVolume): CoreAudio on a Rust MTA worker.

use isle_core::i18n::{trf};
use isle_core::state::{AppVolume, AudioOutput};
use std::sync::mpsc::{self, Sender};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioStatus {
    pub volume_pct: Option<u8>,
    pub muted: bool,
    pub mic_muted: Option<bool>,
    pub apps: Vec<AppVolume>,
    /// Filled only while a control center is open, like the mixer.
    pub outputs: Vec<AudioOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioCommand {
    Panel(bool),
    SetVolume(u8),
    Wheel(i32),
    ToggleMute,
    ToggleMic,
    SetAppVolume(String, u8),
    ToggleAppMute(String),
    /// Device id: becomes the default output for every role, like Windows' sound settings.
    SetOutput(String),
    #[doc(hidden)] Changed,
    #[doc(hidden)] DevicesChanged,
}

#[derive(Debug)]
pub enum AudioEvent { Status(AudioStatus), Error(String) }

/// Starts immediately; no COM work or startup wait on the caller's thread.
pub fn spawn(on_event: impl Fn(AudioEvent) + Send + 'static) -> Sender<AudioCommand> {
    let (tx, rx) = mpsc::channel();
    #[cfg(windows)] {
        let events = tx.clone();
        std::thread::spawn(move || {
            if let Err(err) = win::run(rx, events, &on_event) {
                on_event(AudioEvent::Error(trf("errors.audio", &[("err", &format!("{err:#}"))])));
            }
        });
    }
    #[cfg(not(windows))] std::thread::spawn(move || {
        on_event(AudioEvent::Status(AudioStatus::default()));
        for command in rx {
            if !matches!(command, AudioCommand::Panel(_) | AudioCommand::Changed | AudioCommand::DevicesChanged) {
                on_event(AudioEvent::Error("Controle de áudio disponível no Windows".into()));
            }
        }
    });
    tx
}

#[cfg(any(windows, test))]
fn volume_percent(scalar: f32) -> u8 { (scalar.clamp(0.0, 1.0) * 100.0).round() as u8 }
#[cfg(any(windows, test))]
pub fn wheel_volume(pct: u8, delta: i32) -> u8 {
    (pct as i64 + delta as i64 * 2 / 120).clamp(0, 100) as u8
}

#[cfg(any(windows, test))]
fn group_mixer_apps(mut readings: Vec<(AppVolume, bool)>) -> Vec<AppVolume> {
    readings.sort_by_key(|(_,active)| !active);
    let mut apps: Vec<AppVolume> = Vec::new();
    for (app,_) in readings {
        if !apps.iter().any(|a| a.id == app.id) { apps.push(app); }
    }
    apps.sort_by(|a,b| a.name.cmp(&b.name));
    apps
}

#[cfg(any(windows, test))]
fn mixer_session_index(active: &[bool]) -> Option<usize> {
    active.iter().position(|&playing| playing).or(if active.is_empty() { None } else { Some(0) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mute_uses_the_same_active_session_as_the_displayed_app() {
        let active = [false,true,false]; let muted = [false,true,false];
        assert!(muted[mixer_session_index(&active).unwrap()]);
        assert_eq!(mixer_session_index(&[true,true]),Some(0));
        assert_eq!(mixer_session_index(&[false,false]),Some(0));
        assert_eq!(mixer_session_index(&[]),None);
    }
    #[test]
    fn mixer_groups_program_sessions_preferring_active_then_keeps_row_order() {
        let app = |id: &str,name: &str,pct| AppVolume { id: id.into(), name: name.into(), volume_pct: pct, muted: false };
        let result = group_mixer_apps(vec![
            (app("spotify","Spotify",10),false), (app("spotify","Spotify",60),true),
            (app("firefox","Firefox",30),false), (app("spotify","Spotify",80),true),
        ]);
        assert_eq!(result, vec![app("firefox","Firefox",30),app("spotify","Spotify",60)]);
    }
    #[test]
    fn output_names_drop_the_driver_in_parentheses() {
        assert_eq!(short_output_name("Alto-falantes (Realtek(R) Audio)"), "Alto-falantes");
        assert_eq!(short_output_name("Fone"), "Fone");
        assert_eq!(short_output_name("(Sem nome)"), "(Sem nome)");
    }
    #[test]
    fn scalar_volume_is_rounded_and_bounded() {
        assert_eq!(volume_percent(0.655), 66);
        assert_eq!(volume_percent(1.2), 100);
        assert_eq!(volume_percent(-0.2), 0);
        assert_eq!(volume_percent(f32::NAN), 0);
    }
    #[test]
    fn wheel_uses_two_percent_per_notch_and_clamps() {
        assert_eq!(wheel_volume(65, 120), 67);
        assert_eq!(wheel_volume(65, -240), 61);
        assert_eq!(wheel_volume(99, 120), 100);
        assert_eq!(wheel_volume(1, -120), 0);
        assert_eq!(wheel_volume(65, 60), 66);
        assert_eq!(wheel_volume(65, i32::MAX), 100);
    }
}

/// "Alto-falantes (Realtek(R) Audio)" → "Alto-falantes": the driver part only crowds the chip.
#[cfg(any(windows, test))]
fn short_output_name(name: &str) -> &str {
    match name.split_once(" (") { Some((short, _)) if !short.trim().is_empty() => short.trim(), _ => name }
}

#[cfg(windows)]
mod win {
    use super::*;
    use anyhow::Result;
    use std::{ptr, sync::mpsc::{Receiver, RecvTimeoutError}, time::Duration};
    use windows::{core::{implement, Interface, PCWSTR, PWSTR}, Win32::{
        Foundation::CloseHandle,
        Media::Audio::{*, Endpoints::*},
        System::{Com::*, Threading::*},
        UI::Shell::PropertiesSystem::PROPERTYKEY,
    }};

    #[implement(IAudioEndpointVolumeCallback, IMMNotificationClient)]
    struct Events(Sender<AudioCommand>);
    impl IAudioEndpointVolumeCallback_Impl for Events_Impl {
        fn OnNotify(&self, _: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
            let _ = self.0.send(AudioCommand::Changed); Ok(())
        }
    }
    impl IMMNotificationClient_Impl for Events_Impl {
        fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> { self.changed() }
        fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> { self.changed() }
        fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> { self.changed() }
        fn OnDefaultDeviceChanged(&self, _: EDataFlow, _: ERole, _: &PCWSTR) -> windows::core::Result<()> { self.changed() }
        fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> { Ok(()) }
    }
    impl Events_Impl {
        fn changed(&self) -> windows::core::Result<()> {
            let _ = self.0.send(AudioCommand::DevicesChanged); Ok(())
        }
    }

    // Windows' own interface for the default device (undocumented, stable since Windows 7);
    // the mod and the volume tools use it. Only SetDefaultEndpoint is called.
    use policy::set_output;
    #[allow(non_snake_case)]
    mod policy {
    use windows::{core::PCWSTR, Win32::{Media::Audio::ERole, UI::Shell::PropertiesSystem::PROPERTYKEY}};
    #[windows::core::interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    unsafe trait IPolicyConfig: windows::core::IUnknown {
        fn GetMixFormat(&self, a0: PCWSTR, a1: *mut *mut std::ffi::c_void) -> windows::core::HRESULT;
        fn GetDeviceFormat(&self, a0: PCWSTR, a1: i32, a2: *mut *mut std::ffi::c_void) -> windows::core::HRESULT;
        fn ResetDeviceFormat(&self, a0: PCWSTR) -> windows::core::HRESULT;
        fn SetDeviceFormat(&self, a0: PCWSTR, a1: *mut std::ffi::c_void, a2: *mut std::ffi::c_void) -> windows::core::HRESULT;
        fn GetProcessingPeriod(&self, a0: PCWSTR, a1: i32, a2: *mut i64, a3: *mut i64) -> windows::core::HRESULT;
        fn SetProcessingPeriod(&self, a0: PCWSTR, a1: *mut i64) -> windows::core::HRESULT;
        fn GetShareMode(&self, a0: PCWSTR, a1: *mut std::ffi::c_void) -> windows::core::HRESULT;
        fn SetShareMode(&self, a0: PCWSTR, a1: *mut std::ffi::c_void) -> windows::core::HRESULT;
        fn GetPropertyValue(&self, a0: PCWSTR, a1: *const PROPERTYKEY, a2: *mut std::ffi::c_void) -> windows::core::HRESULT;
        fn SetPropertyValue(&self, a0: PCWSTR, a1: *const PROPERTYKEY, a2: *mut std::ffi::c_void) -> windows::core::HRESULT;
        fn SetDefaultEndpoint(&self, id: PCWSTR, role: ERole) -> windows::core::HRESULT;
        fn SetEndpointVisibility(&self, a0: PCWSTR, a1: i32) -> windows::core::HRESULT;
    }
    const CLSID_POLICY_CONFIG: windows::core::GUID = windows::core::GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
    /// Makes a device the default output for every role, like Windows' sound settings.
    pub fn set_output(id: &str) -> anyhow::Result<()> {
        use windows::Win32::{Media::Audio::{eCommunications, eConsole, eMultimedia}, System::Com::{CLSCTX_ALL, CoCreateInstance}};
        unsafe {
            let policy: IPolicyConfig = CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL)?;
            let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
            for role in [eConsole, eMultimedia, eCommunications] {
                policy.SetDefaultEndpoint(PCWSTR(wide.as_ptr()), role).ok()?;
            }
        }
        Ok(())
    }
    }
    const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY { fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0), pid: 14 };

    fn device_id(device: &IMMDevice) -> Result<String> {
        unsafe {
            let raw = device.GetId()?;
            let id = raw.to_string();
            CoTaskMemFree(Some(raw.0 as *const _));
            Ok(id?)
        }
    }
    fn outputs(devices: &IMMDeviceEnumerator) -> Result<Vec<AudioOutput>> {
        unsafe {
            let default = devices.GetDefaultAudioEndpoint(eRender, eMultimedia).ok().and_then(|d| device_id(&d).ok());
            let list = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            let mut found = Vec::new();
            for i in 0..list.GetCount()? {
                let Ok(device) = list.Item(i) else { continue };
                let Ok(id) = device_id(&device) else { continue };
                let name = device.OpenPropertyStore(STGM_READ)
                    .and_then(|store| store.GetValue(&PKEY_DEVICE_FRIENDLY_NAME))
                    .map(|value| value.to_string()).unwrap_or_default();
                let name = if name.is_empty() { format!("Saída {}", i + 1) } else { short_output_name(&name).to_string() };
                found.push(AudioOutput { is_default: default.as_deref() == Some(id.as_str()), id, name });
            }
            Ok(found)
        }
    }
    struct Endpoints {
        output: Option<IAudioEndpointVolume>,
        mic: Option<IAudioEndpointVolume>,
        callback: IAudioEndpointVolumeCallback,
    }
    impl Endpoints {
        fn new(devices: &IMMDeviceEnumerator, callback: IAudioEndpointVolumeCallback) -> Self {
            let activate = |flow, role| unsafe {
                devices.GetDefaultAudioEndpoint(flow, role).and_then(|d| d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)).ok()
            };
            let this = Self { output: activate(eRender, eMultimedia), mic: activate(eCapture, eConsole), callback };
            for endpoint in [&this.output, &this.mic].into_iter().flatten() {
                if let Err(err) = unsafe { endpoint.RegisterControlChangeNotify(&this.callback) } {
                    tracing::warn!("notificação de áudio: {err}");
                }
            }
            this
        }
        fn status(&self, devices: &IMMDeviceEnumerator, panel: bool) -> AudioStatus {
            unsafe {
                AudioStatus {
                    volume_pct: self.output.as_ref().and_then(|v| v.GetMasterVolumeLevelScalar().ok()).map(volume_percent),
                    muted: self.output.as_ref().and_then(|v| v.GetMute().ok()).is_some_and(|v| v.as_bool()),
                    mic_muted: self.mic.as_ref().and_then(|v| v.GetMute().ok()).map(|v| v.as_bool()),
                    apps: if panel { mixer(devices).unwrap_or_else(|e| { tracing::warn!("mixer: {e:#}"); Vec::new() }) } else { Vec::new() },
                    outputs: if panel { outputs(devices).unwrap_or_else(|e| { tracing::warn!("saídas: {e:#}"); Vec::new() }) } else { Vec::new() },
                }
            }
        }
        fn command(&self, devices: &IMMDeviceEnumerator, cmd: AudioCommand) -> Result<()> {
            unsafe {
                match cmd {
                    AudioCommand::SetVolume(pct) => self.output.as_ref().ok_or_else(|| anyhow::anyhow!("Nenhuma saída de áudio"))?
                        .SetMasterVolumeLevelScalar(pct.min(100) as f32 / 100.0, ptr::null())?,
                    AudioCommand::Wheel(delta) => {
                        let output = self.output.as_ref().ok_or_else(|| anyhow::anyhow!("Nenhuma saída de áudio"))?;
                        let pct = wheel_volume(volume_percent(output.GetMasterVolumeLevelScalar()?), delta);
                        output.SetMasterVolumeLevelScalar(pct as f32 / 100.0, ptr::null())?;
                    }
                    AudioCommand::ToggleMute | AudioCommand::ToggleMic => {
                        let endpoint = if cmd == AudioCommand::ToggleMic { &self.mic } else { &self.output };
                        let endpoint = endpoint.as_ref().ok_or_else(|| anyhow::anyhow!("Dispositivo de áudio indisponível"))?;
                        endpoint.SetMute(!endpoint.GetMute()?.as_bool(), ptr::null())?;
                    }
                    AudioCommand::SetAppVolume(ref id, pct) => {
                        let matching: Vec<_> = sessions(devices)?.into_iter().filter(|s| &s.id == id).collect();
                        anyhow::ensure!(!matching.is_empty(), "O aplicativo de áudio foi fechado");
                        for volume in matching.into_iter().map(|s| s.volume) {
                            volume.SetMasterVolume(pct.min(100) as f32 / 100.0, ptr::null())?;
                        }
                    }
                    AudioCommand::ToggleAppMute(ref id) => {
                        let matching: Vec<_> = sessions(devices)?.into_iter().filter(|s| &s.id == id).collect();
                        anyhow::ensure!(!matching.is_empty(), "O aplicativo de áudio foi fechado");
                        let active: Vec<_> = matching.iter().map(|s| s.active).collect();
                        if let Some(index) = mixer_session_index(&active) {
                            let mute = !matching[index].volume.GetMute()?.as_bool();
                            for session in matching { session.volume.SetMute(mute, ptr::null())?; }
                        }
                    }
                    AudioCommand::SetOutput(ref id) => set_output(id)?,
                    _ => {}
                }
            }
            Ok(())
        }
    }
    impl Drop for Endpoints {
        fn drop(&mut self) {
            for endpoint in [&self.output, &self.mic].into_iter().flatten() {
                let _ = unsafe { endpoint.UnregisterControlChangeNotify(&self.callback) };
            }
        }
    }

    struct Session { id: String, name: String, active: bool, volume: ISimpleAudioVolume }
    fn sessions(devices: &IMMDeviceEnumerator) -> Result<Vec<Session>> {
        unsafe {
            let device = devices.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
            let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            let list = manager.GetSessionEnumerator()?;
            let mut found = Vec::new();
            for i in 0..list.GetCount()? {
                let Ok(control) = list.GetSession(i) else { continue };
                let Ok(state) = control.GetState() else { continue };
                if state == AudioSessionStateExpired { continue; }
                let (Ok(control2), Ok(volume)) = (control.cast::<IAudioSessionControl2>(), control.cast::<ISimpleAudioVolume>()) else { continue };
                let pid = control2.GetProcessId().unwrap_or(0);
                let system = control2.IsSystemSoundsSession() == windows::core::HRESULT(0);
                let path = process_path(pid);
                let (id, name) = if system { ("system".into(), "Sons do sistema".into()) }
                    else if !path.is_empty() {
                        // shortcut: executable stems name apps; read version resources when friendly/UWP names are needed.
                        (path.to_lowercase(), path.rsplit('\\').next().unwrap_or(&path).trim_end_matches(".exe").to_string())
                    } else { (format!("pid:{pid}"), format!("App {pid}")) };
                found.push(Session { id, name, active: state == AudioSessionStateActive, volume });
            }
            Ok(found)
        }
    }
    fn process_path(pid: u32) -> String {
        unsafe {
            let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return String::new() };
            let mut buf = vec![0u16; 32768]; let mut len = buf.len() as u32;
            let result = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
            let _ = CloseHandle(handle);
            if result.is_ok() { String::from_utf16_lossy(&buf[..len as usize]) } else { String::new() }
        }
    }
    fn mixer(devices: &IMMDeviceEnumerator) -> Result<Vec<AppVolume>> {
        let mut readings = Vec::new();
        for s in sessions(devices)? {
            let (Ok(volume), Ok(muted)) = (unsafe { s.volume.GetMasterVolume() }, unsafe { s.volume.GetMute() }) else { continue };
            readings.push((AppVolume { id: s.id, name: s.name, volume_pct: volume_percent(volume), muted: muted.as_bool() }, s.active));
        }
        Ok(group_mixer_apps(readings))
    }

    pub fn run(rx: Receiver<AudioCommand>, tx: Sender<AudioCommand>, on_event: &impl Fn(AudioEvent)) -> Result<()> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()?; }
        let devices: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        let callback: IAudioEndpointVolumeCallback = Events(tx).into();
        let notification: IMMNotificationClient = callback.cast()?;
        unsafe { devices.RegisterEndpointNotificationCallback(&notification)?; }
        let mut endpoints = Endpoints::new(&devices, callback.clone());
        let mut panel = false;
        on_event(AudioEvent::Status(endpoints.status(&devices, panel)));
        loop {
            // shortcut: mixer sessions refresh every 3 s only while open; add session events for instant app-list updates.
            let received = if panel { rx.recv_timeout(Duration::from_secs(3)) } else { rx.recv().map_err(|_| RecvTimeoutError::Disconnected) };
            match received {
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {},
                Ok(AudioCommand::Panel(open)) => panel = open,
                Ok(AudioCommand::DevicesChanged) => {
                    drop(endpoints);
                    endpoints = Endpoints::new(&devices, callback.clone());
                }
                Ok(command) => if let Err(err) = endpoints.command(&devices, command) {
                    on_event(AudioEvent::Error(format!("Áudio: {err:#}")));
                },
            }
            on_event(AudioEvent::Status(endpoints.status(&devices, panel)));
        }
        drop(endpoints);
        unsafe { let _ = devices.UnregisterEndpointNotificationCallback(&notification); }
        drop(notification); drop(callback); drop(devices);
        unsafe { CoUninitialize(); }
        Ok(())
    }
}
