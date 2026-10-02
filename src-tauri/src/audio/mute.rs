use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use log::{info, warn};

// Track whether we muted the audio, and what the original state was before we touched it
// None = not currently muted by us
// Some(was_muted) = currently muted by us; was_muted is whether it was muted before we acted
static PRIOR_MUTE_STATE: Mutex<Option<bool>> = Mutex::new(None);
static MUTE_SESSION_ID: AtomicU64 = AtomicU64::new(0);

#[cfg(windows)]
pub fn mute_system_audio_delayed(delay_ms: u64) {
    let settings = crate::storage::settings::load_settings();
    if !settings.pause_other_audio_while_talking {
        return;
    }

    let session_id = MUTE_SESSION_ID.fetch_add(1, Ordering::SeqCst) + 1;

    std::thread::spawn(move || {
        if delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }

        if MUTE_SESSION_ID.load(Ordering::SeqCst) != session_id {
            // Cancelled by an unmute or another recording event
            return;
        }

        let mut guard = match PRIOR_MUTE_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if guard.is_some() {
            // Already muted by us in an active recording session
            return;
        }

        match get_endpoint_volume() {
            Ok(endpoint_volume) => {
                unsafe {
                    let current_mute = match endpoint_volume.GetMute() {
                        Ok(m) => m,
                        Err(e) => {
                            warn!("Failed to query audio mute status: {:?}", e);
                            return;
                        }
                    };

                    let was_muted = current_mute.as_bool();
                    *guard = Some(was_muted);

                    if !was_muted {
                        if let Err(e) = endpoint_volume.SetMute(windows::Win32::Foundation::BOOL(1), std::ptr::null()) {
                            warn!("Failed to mute system audio: {:?}", e);
                        } else {
                            info!("Background audio muted for dictation (delayed {}ms)", delay_ms);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Could not access audio endpoint for muting: {:?}", e);
            }
        }
    });
}

#[cfg(windows)]
pub fn mute_system_audio() {
    mute_system_audio_delayed(0);
}

#[cfg(windows)]
pub fn unmute_system_audio() {
    // Invalidate any pending delayed mute immediately
    MUTE_SESSION_ID.fetch_add(1, Ordering::SeqCst);

    let mut guard = match PRIOR_MUTE_STATE.lock() {
        Ok(g) => g,
        Err(_) => return,
    };

    let was_muted = match guard.take() {
        Some(state) => state,
        None => return, // We didn't mute it
    };

    // If it was already muted by the user before recording started, leave it muted
    if was_muted {
        return;
    }

    match get_endpoint_volume() {
        Ok(endpoint_volume) => {
            unsafe {
                if let Err(e) = endpoint_volume.SetMute(windows::Win32::Foundation::BOOL(0), std::ptr::null()) {
                    warn!("Failed to unmute system audio: {:?}", e);
                } else {
                    info!("Background audio unmuted after dictation");
                }
            }
        }
        Err(e) => {
            warn!("Could not access audio endpoint for unmuting: {:?}", e);
        }
    }
}

#[cfg(windows)]
fn get_endpoint_volume() -> Result<windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume, windows::core::Error> {
    use windows::Win32::System::Com::*;
    use windows::Win32::Media::Audio::*;
    use windows::Win32::Media::Audio::Endpoints::*;

    unsafe {
        // Initialize COM for this thread if not already initialized
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device: IMMDevice = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
        let endpoint_volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;

        Ok(endpoint_volume)
    }
}

#[cfg(not(windows))]
pub fn mute_system_audio_delayed(_delay_ms: u64) {}

#[cfg(not(windows))]
pub fn mute_system_audio() {}

#[cfg(not(windows))]
pub fn unmute_system_audio() {}
