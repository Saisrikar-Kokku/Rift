use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Retrieves the existing Scratchpad window, or lazily constructs it on demand.
/// Preserves notes and state across open/close cycles by hiding rather than destroying.
pub fn get_or_create_scratchpad(app: &AppHandle) -> Result<WebviewWindow, String> {
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
/// Debounced by 600ms to allow window exit animations and UI transitions to finish cleanly.
pub fn trim_application_working_set_async() {
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        #[cfg(windows)]
        {
            use windows::Win32::Foundation::CloseHandle;
            use windows::Win32::System::Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
            };
            use windows::Win32::System::Threading::{
                GetCurrentProcess, OpenProcess, SetProcessWorkingSetSize, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA,
            };

            // 1. Trim Rift.exe native process
            unsafe {
                let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
            }

            // 2. Enumerate child msedgewebview2 processes and trim their working sets
            let my_pid = std::process::id();
            if let Ok(snap) = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) } {
                let mut entry = PROCESSENTRY32W::default();
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

                if unsafe { Process32FirstW(snap, &mut entry) }.is_ok() {
                    loop {
                        if entry.th32ParentProcessID == my_pid {
                            if let Ok(child_handle) = unsafe {
                                OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, false, entry.th32ProcessID)
                            } {
                                unsafe {
                                    let _ = SetProcessWorkingSetSize(child_handle, usize::MAX, usize::MAX);
                                    let _ = CloseHandle(child_handle);
                                }
                            }
                        }
                        if unsafe { Process32NextW(snap, &mut entry) }.is_err() {
                            break;
                        }
                    }
                }
                unsafe {
                    let _ = CloseHandle(snap);
                }
            }
        }
    });
}
