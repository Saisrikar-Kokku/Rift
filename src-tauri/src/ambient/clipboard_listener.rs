use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::DataExchange::{AddClipboardFormatListener, RemoveClipboardFormatListener};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GetForegroundWindow,
    GetMessageW, GetWindowThreadProcessId, TranslateMessage,
    HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE,
};

use crate::storage::clipboard::ClipboardStore;
use crate::text::injector::TextInjector;

static LISTENER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Convert an executable name like "code.exe" or "chrome.exe" to a friendly label
fn friendly_app_name(proc_name: &str) -> String {
    let lower = proc_name.to_lowercase();
    match lower.as_str() {
        "chrome.exe" => "Google Chrome".to_string(),
        "msedge.exe" => "Microsoft Edge".to_string(),
        "firefox.exe" => "Firefox".to_string(),
        "code.exe" => "VS Code".to_string(),
        "cursor.exe" => "Cursor".to_string(),
        "devenv.exe" => "Visual Studio".to_string(),
        "idea64.exe" => "IntelliJ IDEA".to_string(),
        "pycharm64.exe" => "PyCharm".to_string(),
        "notepad.exe" => "Notepad".to_string(),
        "notepad++.exe" => "Notepad++".to_string(),
        "slack.exe" => "Slack".to_string(),
        "discord.exe" => "Discord".to_string(),
        "telegram.exe" => "Telegram".to_string(),
        "whatsapp.exe" => "WhatsApp".to_string(),
        "teams.exe" | "ms-teams.exe" => "Teams".to_string(),
        "winword.exe" => "Microsoft Word".to_string(),
        "excel.exe" => "Microsoft Excel".to_string(),
        "powerpnt.exe" => "PowerPoint".to_string(),
        "outlook.exe" => "Outlook".to_string(),
        "explorer.exe" => "File Explorer".to_string(),
        "windowsterminal.exe" | "wt.exe" => "Windows Terminal".to_string(),
        "cmd.exe" => "Command Prompt".to_string(),
        "powershell.exe" | "pwsh.exe" => "PowerShell".to_string(),
        "obsidian.exe" => "Obsidian".to_string(),
        "notion.exe" => "Notion".to_string(),
        "spotify.exe" => "Spotify".to_string(),
        _ => {
            if let Some(stem) = lower.strip_suffix(".exe") {
                let mut c = stem.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            } else {
                proc_name.to_string()
            }
        }
    }
}

/// Categorize clipboard text content into "code", "url", "email", "filepath", or "text"
pub fn categorize_clip(text: &str) -> &'static str {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "text";
    }

    // 1. Email address (single-line regex)
    if !trimmed.contains('\n') && !trimmed.contains('\r') && !trimmed.contains(' ') {
        if let Ok(re_email) = regex::Regex::new(r"(?i)^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$") {
            if re_email.is_match(trimmed) {
                return "email";
            }
        }
    }

    // 2. URL (single-line starting with http, https, or www.)
    if !trimmed.contains('\n') && !trimmed.contains('\r') && !trimmed.contains(' ') {
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("www.") {
            return "url";
        }
    }

    // 3. File Path (Windows drive letter C:\ or network share \\ or unix /)
    if !trimmed.contains('\n') && !trimmed.contains('\r') {
        if trimmed.starts_with(r"\\") {
            return "filepath";
        }
        let bytes = trimmed.as_bytes();
        if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
            return "filepath";
        }
        if trimmed.starts_with('/') && (trimmed.contains("/src/") || trimmed.contains("/Users/") || trimmed.contains("/home/")) {
            return "filepath";
        }
    }

    // 4. Code Detection
    if is_code_content(trimmed) {
        return "code";
    }

    "text"
}

fn is_code_content(text: &str) -> bool {
    let lower = text.to_lowercase();

    // Structural code signals
    let has_braces = text.contains('{') && text.contains('}');
    let has_tags = (text.contains("<div") || text.contains("<span") || text.contains("</div>") || text.contains("/>")) && text.contains('>');
    let has_arrow = text.contains("=>") || text.contains("->");
    let has_equality = text.contains("==") || text.contains("!=") || text.contains("===");
    let has_semis = text.matches(';').count() >= 2;

    if has_braces || has_tags || has_arrow || has_equality || has_semis {
        return true;
    }

    // Keyword checks
    let code_keywords = [
        "fn ", "pub fn ", "def ", "function ", "const ", "let mut ", "import ",
        "export ", "class ", "return ", "console.log", "println!", "SELECT ",
        "FROM ", "WHERE ", "UPDATE ", "INSERT INTO ", "curl -", "docker run",
        "npm install", "cargo build", "git commit", "git push", "pip install",
    ];

    for kw in &code_keywords {
        if text.contains(kw) || lower.contains(&kw.to_lowercase()) {
            return true;
        }
    }

    // Indented block check (multiple lines starting with 4 spaces or tab)
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() >= 3 {
        let indented_lines = lines.iter().filter(|l| l.starts_with("    ") || l.starts_with('\t')).count();
        if indented_lines >= 2 {
            return true;
        }
    }

    false
}

/// Starts the Win32 message-driven clipboard listener in a background thread.
/// Consumes 0.0% CPU when idle because it waits on GetMessageW notifications.
pub fn start_clipboard_listener(
    store: Arc<ClipboardStore>,
    is_internal_pasting: Arc<AtomicBool>,
    app_handle: AppHandle,
) {
    if LISTENER_RUNNING.swap(true, Ordering::SeqCst) {
        return; // Already running
    }

    thread::Builder::new()
        .name("rift-clipboard-listener".to_string())
        .spawn(move || {
            log::info!("Starting Rift Win32 Clipboard Listener");

            unsafe {
                let hwnd: HWND = match CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("STATIC"),
                    w!("RiftClipboardListenerWindow"),
                    WINDOW_STYLE(0),
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    None,
                    None,
                    None,
                ) {
                    Ok(h) if !h.0.is_null() => h,
                    _ => {
                        log::error!("Failed to create message-only window for clipboard listener");
                        LISTENER_RUNNING.store(false, Ordering::SeqCst);
                        return;
                    }
                };


                if AddClipboardFormatListener(hwnd).is_err() {
                    log::error!("Failed to register AddClipboardFormatListener");
                    let _ = DestroyWindow(hwnd);
                    LISTENER_RUNNING.store(false, Ordering::SeqCst);
                    return;
                }

                log::info!("AddClipboardFormatListener succeeded. Entering message loop.");

                let my_pid = std::process::id();
                let mut msg = MSG::default();

                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.message == WM_CLIPBOARDUPDATE {
                        // 1. If Rift itself is currently injecting text via simulated paste, ignore
                        if is_internal_pasting.load(Ordering::SeqCst) {
                            let _ = TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                            continue;
                        }

                        // 2. Brief 25ms yield to allow source application to finish writing multi-part formats
                        thread::sleep(Duration::from_millis(25));

                        // 3. Extract text from clipboard safely
                        if let Some(clipboard_text) = TextInjector::get_clipboard_text() {
                            let trimmed = clipboard_text.trim();
                            if !trimmed.is_empty() {
                                // 4. Detect foreground application
                                let fg_hwnd = GetForegroundWindow();
                                let mut fg_pid = 0u32;
                                GetWindowThreadProcessId(fg_hwnd, Some(&mut fg_pid));

                                let source_app = if fg_pid != 0 {
                                    if fg_pid == my_pid {
                                        Some("Rift".to_string())
                                    } else {
                                        let raw_name = crate::ambient::context::get_window_context(Some(fg_hwnd.0 as isize)).process_name;
                                        if !raw_name.is_empty() {
                                            Some(friendly_app_name(&raw_name))
                                        } else {
                                            None
                                        }
                                    }
                                } else {
                                    None
                                };

                                let category = categorize_clip(trimmed);

                                // 5. Insert into SQLite store
                                match store.insert_clip(trimmed, source_app.as_deref(), category) {
                                    Ok(Some(new_id)) => {
                                        log::info!(
                                            "New clip #{}: [{}] {} chars from {:?}",
                                            new_id,
                                            category,
                                            trimmed.len(),
                                            source_app
                                        );

                                        // 6. Notify open frontend windows (Scratchpad, Home, etc.)
                                        let _ = app_handle.emit(
                                            "clipboardChanged",
                                            serde_json::json!({
                                                "id": new_id,
                                                "category": category,
                                                "source_app": source_app,
                                                "preview": trimmed.chars().take(80).collect::<String>()
                                            }),
                                        );
                                    }
                                    Ok(None) => {
                                        // Skipped duplicate consecutive clip
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to store clipboard entry: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                let _ = RemoveClipboardFormatListener(hwnd);
                let _ = DestroyWindow(hwnd);
                LISTENER_RUNNING.store(false, Ordering::SeqCst);
                log::info!("Clipboard listener message loop exited.");
            }
        })
        .expect("Failed to spawn clipboard listener thread");
}
