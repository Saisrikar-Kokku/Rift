use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

static SCRATCHPAD_LIFECYCLE_LOCK: Mutex<()> = Mutex::new(());
static SCRATCHPAD_LIFECYCLE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Retrieves the existing Scratchpad window, or lazily constructs it on demand.
/// Drafts are persisted separately, so a closed window can release its WebView resources.
pub fn get_or_create_scratchpad(app: &AppHandle) -> Result<WebviewWindow, String> {
    let _lifecycle = SCRATCHPAD_LIFECYCLE_LOCK
        .lock()
        .map_err(|_| "Scratchpad lifecycle lock was poisoned".to_string())?;
    SCRATCHPAD_LIFECYCLE_GENERATION.fetch_add(1, Ordering::AcqRel);

    if let Some(window) = app.get_webview_window("scratchpad") {
        crate::show_and_focus_window(&window);
        return Ok(window);
    }

    log::info!("Lazily constructing Rift Scratchpad auxiliary window...");
    let window = WebviewWindowBuilder::new(
        app,
        "scratchpad",
        WebviewUrl::App("screens/scratchpad.html".into()),
    )
    .title("Rift Scratch Pad")
    .inner_size(440.0, 540.0)
    .min_inner_size(360.0, 380.0)
    .decorations(false)
    .transparent(false)
    .always_on_top(true)
    .skip_taskbar(false)
    .resizable(true)
    .visible(false)
    .build()
    .map_err(|e| format!("Failed to create scratchpad window: {}", e))?;

    let _ = window.center();
    let _ = window.set_always_on_top(true);
    crate::show_and_focus_window(&window);
    Ok(window)
}

/// Destroys the Scratchpad after a short delay so callers can restore focus
/// before clipboard insertion. A reopen during the delay cancels this close.
pub fn schedule_scratchpad_close(window: WebviewWindow) {
    let generation = match SCRATCHPAD_LIFECYCLE_LOCK.lock() {
        Ok(_lifecycle) => SCRATCHPAD_LIFECYCLE_GENERATION
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1),
        Err(_) => {
            log::error!("Could not schedule Scratchpad close: lifecycle lock was poisoned");
            return;
        }
    };

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let _lifecycle = match SCRATCHPAD_LIFECYCLE_LOCK.lock() {
            Ok(lifecycle) => lifecycle,
            Err(_) => {
                log::error!("Could not close Scratchpad: lifecycle lock was poisoned");
                return;
            }
        };

        if SCRATCHPAD_LIFECYCLE_GENERATION.load(Ordering::Acquire) != generation {
            return;
        }

        match window.destroy() {
            Ok(()) => trim_application_working_set_async(),
            Err(error) => log::warn!("Failed to destroy the Scratchpad window: {error}"),
        }
    });
}

/// Retrieves the existing Spotlight window, or lazily constructs it on demand.
pub fn get_or_create_spotlight(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window("spotlight") {
        let _ = window.center();
        crate::show_and_focus_window(&window);
        return Ok(window);
    }

    log::info!("Lazily constructing Ask Rift Spotlight auxiliary window...");
    let window = WebviewWindowBuilder::new(
        app,
        "spotlight",
        WebviewUrl::App("screens/spotlight.html".into()),
    )
    .title("Ask Rift - Spotlight Voice AI")
    .inner_size(640.0, 440.0)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .visible(false)
    .build()
    .map_err(|e| format!("Failed to create spotlight window: {}", e))?;

    let _ = window.center();
    let w_clone = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = w_clone.hide();
            trim_application_working_set_async();
        }
    });

    crate::show_and_focus_window(&window);
    Ok(window)
}

/// Asynchronously flushes inactive physical memory pages back to the Windows OS pool.
/// Safely trims Rift.exe host process without disturbing active WebView2 render pipelines.
pub fn trim_application_working_set_async() {
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        #[cfg(windows)]
        {
            use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
            // Safely trim Rift host working set pages
            unsafe {
                let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
            }
        }
    });
}
