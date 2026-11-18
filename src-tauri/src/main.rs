#![cfg_attr(windows, windows_subsystem = "windows")]
#![recursion_limit = "256"]
#![allow(non_snake_case)]

mod ai;
mod ambient;
mod audio;
mod hotkey;
mod state;
mod storage;
mod stt;
mod text;
mod window;

use audio::capture::list_input_devices;
use state::AppState;
use storage::credentials::CredentialsProvider;
use storage::dictionary::{apply_dictionary, DictionaryEntry};
use storage::history::HistoryEntry;
use storage::settings::{load_settings, save_settings, try_save_settings, WidgetPosition};
use storage::snippets::VoiceSnippet;
use stt::openrouter::OpenRouterClient;
use text::cleaner::{clean_transcript, is_silence_or_hallucination};
use text::injector::TextInjector;


use serde_json::{json, Value};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State};

static IS_RECORDING_SESSION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static WIDGET_DROPDOWN_ORIGIN: std::sync::Mutex<Option<tauri::PhysicalPosition<i32>>> =
    std::sync::Mutex::new(None);

fn start_dynamic_focus_tracker(state: Arc<AppState>) {
    if !IS_RECORDING_SESSION.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::thread::spawn(move || {
            let my_pid = std::process::id();
            while IS_RECORDING_SESSION.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(40));
                if let Some((fg_pid, raw_val)) = TextInjector::get_foreground_window_pid_and_raw() {
                    if fg_pid != 0 && fg_pid != my_pid {
                        let mut target = state.target_window.lock(); {
                            *target = Some(raw_val);
                        }
                    }
                }
            }
        });
    }
}

fn stop_dynamic_focus_tracker() {
    IS_RECORDING_SESSION.store(false, std::sync::atomic::Ordering::SeqCst);
}

fn ensure_widget_on_top(app: &AppHandle) {
    if let Some(widget_win) = app.get_webview_window("widget") {
        let _ = widget_win.unminimize();
        let _ = widget_win.show();
        let _ = widget_win.set_always_on_top(true);
        #[cfg(windows)]
        if let Ok(hwnd_ptr) = widget_win.hwnd() {
            let hwnd = windows::Win32::Foundation::HWND(hwnd_ptr.0 as *mut _);
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::*;
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
    }
}

fn emit_to_frontend(app: &AppHandle, event_name: &str, payload: Option<Value>) {
    use tauri::Emitter;
    let _ = app.emit(event_name, &payload);

    if event_name == "recordingStarted" {
        ensure_widget_on_top(app);
    }

    let js = match &payload {
        Some(p) => format!("if (window.__rift_emit) window.__rift_emit({:?}, {});", event_name, p),
        None => format!("if (window.__rift_emit) window.__rift_emit({:?});", event_name),
    };
    if event_name == "audioLevel" {
        // Audio level waveform updates are only displayed on the floating widget
        if let Some(w) = app.get_webview_window("widget") {
            let _ = w.eval(&js);
        }
    } else {
        for label in &["widget", "main", "scratchpad", "spotlight"] {
            if let Some(w) = app.get_webview_window(label) {
                let _ = w.eval(&js);
            }
        }
    }
}

fn get_usage_stats_val(state: &AppState) -> Value {
    let count = state.whisper_requests_today.load(std::sync::atomic::Ordering::Relaxed);
    let midnight = chrono::Local::now().date_naive().succ_opt()
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().to_string())
        .unwrap_or_else(|| "2026-09-10T00:00:00".to_string());
    json!({
        "whisper": {
            "requestsToday": count,
            "dailyLimit": 2000,
            "requestsThisMinute": 1,
            "minuteLimit": 20,
            "resetsAt": midnight,
        },
        "enhance": {
            "requestsToday": 0,
            "dailyLimit": 7000,
            "requestsThisMinute": 0,
            "minuteLimit": 30,
            "resetsAt": midnight,
        }
    })
}

// -------------------------------------------------------------
// Widget & Appearance Commands
// -------------------------------------------------------------
#[tauri::command]
fn startDragging(app: AppHandle) {
    if let Some(w) = app.get_webview_window("widget") {
        let _ = w.start_dragging();
    }
}

#[tauri::command]
fn startScratchpadDragging(app: AppHandle) {
    if let Some(sp) = app.get_webview_window("scratchpad") {
        let _ = sp.start_dragging();
    }
}

#[tauri::command]
fn getWidgetConfig() -> Value {
    let settings = load_settings();
    json!({
        "widgetVisibility": settings.widget_visibility,
        "followCursorAcrossMonitors": settings.follow_cursor_across_monitors,
        "lockWidgetPosition": settings.lock_widget_position,
        "widgetStyle": settings.widget_style,
        "style": settings.widget_style,
        "pushToTalkKey": settings.push_to_talk_key,
        "pushToTalkMode": settings.push_to_talk_mode,
        "pushToTalkCombo": settings.push_to_talk_combo,
        "customPosition": settings.widget_custom_position,
        "widget_visibility": settings.widget_visibility,
        "follow_cursor_across_monitors": settings.follow_cursor_across_monitors,
        "lock_widget_position": settings.lock_widget_position,
        "widget_style": settings.widget_style,
        "widgetTranscribingStyle": settings.widget_transcribing_style,
        "widget_transcribing_style": settings.widget_transcribing_style,
        "inferenceModel": settings.inference_model,
        "sttProvider": settings.stt_provider,
        "transcriptionMode": settings.transcription_mode,
    })
}

#[tauri::command]
fn setWidgetVisibility(app: AppHandle, mode: String) -> Value {
    let mut settings = load_settings();
    settings.widget_visibility = mode.clone();
    save_settings(&settings);
    let cfg = getWidgetConfig();
    emit_to_frontend(&app, "widgetConfigChanged", Some(cfg));
    json!({ "success": true, "message": format!("Widget visibility set to {}", mode) })
}

#[tauri::command]
fn setFollowCursor(app: AppHandle, enabled: bool) -> Value {
    let mut settings = load_settings();
    settings.follow_cursor_across_monitors = enabled;
    save_settings(&settings);
    let cfg = getWidgetConfig();
    emit_to_frontend(&app, "widgetConfigChanged", Some(cfg));
    emit_to_frontend(&app, "repositionWidget", None);
    json!({ "success": true })
}

#[tauri::command]
fn setLockPosition(app: AppHandle, enabled: bool) -> Value {
    let mut settings = load_settings();
    settings.lock_widget_position = enabled;
    save_settings(&settings);
    let cfg = getWidgetConfig();
    emit_to_frontend(&app, "widgetConfigChanged", Some(cfg));
    emit_to_frontend(&app, "repositionWidget", None);
    json!({ "success": true })
}

#[tauri::command]
fn persistWidgetPosition(x: i32, y: i32) -> Value {
    let mut settings = load_settings();
    settings.widget_custom_position = Some(WidgetPosition { x, y });
    save_settings(&settings);
    json!({ "success": true })
}

#[tauri::command]
fn moveWidgetBy(app: AppHandle, dx: i32, dy: i32) {
    if let Some(w) = app.get_webview_window("widget") {
        let scale = w.scale_factor().unwrap_or(1.0);
        if let Ok(pos) = w.outer_position() {
            let phys_dx = (dx as f64 * scale).round() as i32;
            let phys_dy = (dy as f64 * scale).round() as i32;
            let new_pos = tauri::PhysicalPosition {
                x: pos.x + phys_dx,
                y: pos.y + phys_dy,
            };
            let _ = w.set_position(tauri::Position::Physical(new_pos));
        }
    }
}

static IS_WIDGET_DRAGGING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
fn startWidgetDrag(app: AppHandle) -> Value {
    let settings = load_settings();
    if settings.lock_widget_position {
        return json!({ "locked": true });
    }

    if IS_WIDGET_DRAGGING.compare_exchange(false, true, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).is_err() {
        return json!({ "already_dragging": true });
    }

    let widget_win = match app.get_webview_window("widget") {
        Some(w) => w,
        None => {
            IS_WIDGET_DRAGGING.store(false, std::sync::atomic::Ordering::SeqCst);
            return json!({ "error": "Widget window not found" });
        }
    };

    #[cfg(windows)]
    {
        let hwnd_raw = match widget_win.hwnd() {
            Ok(ptr) => ptr.0 as isize,
            Err(_) => {
                IS_WIDGET_DRAGGING.store(false, std::sync::atomic::Ordering::SeqCst);
                return json!({ "error": "Widget HWND not available" });
            }
        };

        let app_clone = app.clone();
        std::thread::spawn(move || {
            use windows::Win32::Foundation::{HWND, POINT, RECT};
            use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
            use windows::Win32::UI::WindowsAndMessaging::{
                GetCursorPos, GetSystemMetrics, GetWindowRect, SetWindowPos, SM_CXVIRTUALSCREEN,
                SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_NOSIZE,
                SWP_NOZORDER,
            };

            let hwnd = HWND(hwnd_raw as *mut _);

            let mut start_rect = RECT::default();
            let mut start_cur = POINT::default();

            unsafe {
                let _ = GetWindowRect(hwnd, &mut start_rect);
                let _ = GetCursorPos(&mut start_cur);
            }

            let offset_x = start_cur.x - start_rect.left;
            let offset_y = start_cur.y - start_rect.top;

            let width = (start_rect.right - start_rect.left).max(1);
            let height = (start_rect.bottom - start_rect.top).max(1);

            let min_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let min_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
            let virt_w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
            let virt_h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
            let max_x = min_x + (virt_w - width).max(0);
            let max_y = min_y + (virt_h - height).max(0);

            while unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 } {
                let mut cur = POINT::default();
                unsafe {
                    let _ = GetCursorPos(&mut cur);
                    let target_x = (cur.x - offset_x).clamp(min_x, max_x);
                    let target_y = (cur.y - offset_y).clamp(min_y, max_y);
                    let _ = SetWindowPos(
                        hwnd,
                        HWND(0 as *mut _),
                        target_x,
                        target_y,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(8));
            }

            // Drag released: read final position and persist
            let mut final_rect = RECT::default();
            unsafe {
                let _ = GetWindowRect(hwnd, &mut final_rect);
            }

            let mut s = load_settings();
            s.widget_custom_position = Some(WidgetPosition {
                x: final_rect.left,
                y: final_rect.top,
            });
            save_settings(&s);

            IS_WIDGET_DRAGGING.store(false, std::sync::atomic::Ordering::SeqCst);

            let cfg = getWidgetConfig();
            emit_to_frontend(&app_clone, "widgetConfigChanged", Some(cfg));
        });
    }

    #[cfg(not(windows))]
    {
        let _ = widget_win.start_dragging();
        IS_WIDGET_DRAGGING.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    json!({ "success": true })
}

#[tauri::command]
fn saveCurrentWidgetPosition(app: AppHandle) -> Value {
    if let Some(w) = app.get_webview_window("widget") {
        if let Ok(pos) = w.outer_position() {
            let mut s = load_settings();
            s.widget_custom_position = Some(WidgetPosition { x: pos.x, y: pos.y });
            save_settings(&s);
            let cfg = getWidgetConfig();
            emit_to_frontend(&app, "widgetConfigChanged", Some(cfg));
        }
    }
    json!({ "success": true })
}

#[tauri::command]
fn resetWidgetPosition(app: AppHandle) -> Value {
    let mut settings = load_settings();
    settings.widget_custom_position = None;
    save_settings(&settings);
    if let Some(w) = app.get_webview_window("widget") {
        if let Ok(Some(monitor)) = w.primary_monitor() {
            let size = monitor.size();
            let scale = monitor.scale_factor();
            let widget_phys_w = (190.0 * scale) as i32;
            let widget_phys_h = (60.0 * scale) as i32;
            let def_x = ((size.width as i32) - widget_phys_w) / 2;
            let def_y = (size.height as i32) - widget_phys_h - (40.0 * scale) as i32;
            let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x: def_x, y: def_y }));
        }
    }
    let cfg = getWidgetConfig();
    emit_to_frontend(&app, "widgetConfigChanged", Some(cfg));
    emit_to_frontend(&app, "repositionWidget", None);
    json!({ "success": true })
}

#[tauri::command]
fn triggerWidgetTestState(app: AppHandle, state: String) -> Value {
    match state.as_str() {
        "recording" => emit_to_frontend(&app, "recordingStarted", None),
        "processing" => emit_to_frontend(&app, "recordingStopped", None),
        "success" => emit_to_frontend(&app, "textInjected", Some(json!({ "method": "pasted" }))),
        "warning" => emit_to_frontend(&app, "recordingSaved", Some(json!({ "isSilent": true }))),
        "error" => emit_to_frontend(&app, "transcriptionError", Some(json!({ "message": "Test Error" }))),
        _ => {}
    }
    json!({ "success": true })
}

#[tauri::command]
fn setWidgetDropdownOpen(app: AppHandle, open: bool) -> Result<(), String> {
    const WINDOW_WIDTH: f64 = 190.0;
    const COLLAPSED_HEIGHT: f64 = 60.0;
    const EXPANDED_HEIGHT: f64 = 152.0;
    const IDLE_PILL_BOTTOM_OFFSET: f64 = 27.0;
    const OPEN_MENU_BOTTOM_PADDING: f64 = 6.0;

    let Some(window) = app.get_webview_window("widget") else {
        return Ok(());
    };

    let mut saved_origin = WIDGET_DROPDOWN_ORIGIN
        .lock()
        .map_err(|_| "Widget menu position lock is poisoned".to_string())?;

    if !open {
        window
            .eval("document.body.classList.remove('menu-open-above');")
            .map_err(|error| format!("Failed to reset widget menu layout: {error}"))?;
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                WINDOW_WIDTH,
                COLLAPSED_HEIGHT,
            )))
            .map_err(|error| format!("Failed to collapse the widget window: {error}"))?;
        if let Some(position) = *saved_origin {
            window
                .set_position(tauri::Position::Physical(position))
                .map_err(|error| format!("Failed to restore the widget position: {error}"))?;
        }
        *saved_origin = None;
    } else if saved_origin.is_none() {
        let origin = window
            .outer_position()
            .map_err(|error| format!("Failed to read the widget position: {error}"))?;
        let scale = window.scale_factor().unwrap_or(1.0).max(0.5);
        let expanded_height = (EXPANDED_HEIGHT * scale).round() as i32;
        let collapsed_anchor = (IDLE_PILL_BOTTOM_OFFSET * scale).round() as i32;
        let open_bottom_padding = (OPEN_MENU_BOTTOM_PADDING * scale).round() as i32;
        let above_shift = expanded_height - collapsed_anchor - open_bottom_padding;

        let monitor = window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| window.primary_monitor().ok().flatten());

        let (open_above, target_y) = if let Some(monitor) = monitor {
            let area = monitor.work_area();
            let (area_top, area_bottom) = if area.size.height > 0 {
                (
                    area.position.y,
                    area.position.y.saturating_add(area.size.height as i32),
                )
            } else {
                let monitor_position = monitor.position();
                let monitor_size = monitor.size();
                (
                    monitor_position.y,
                    monitor_position.y.saturating_add(monitor_size.height as i32),
                )
            };
            let max_y = area_bottom.saturating_sub(expanded_height);
            let below_fits = origin.y <= max_y;
            let above_y = origin.y.saturating_sub(above_shift);
            let above_fits = above_y >= area_top;

            if below_fits {
                (false, origin.y)
            } else if above_fits {
                (true, above_y)
            } else {
                let available_above = origin.y.saturating_sub(area_top);
                let available_below = area_bottom.saturating_sub(origin.y.saturating_add(expanded_height));
                if available_above > available_below {
                    (true, above_y.clamp(area_top, max_y.max(area_top)))
                } else {
                    (false, origin.y.clamp(area_top, max_y.max(area_top)))
                }
            }
        } else {
            (false, origin.y)
        };

        let layout_script = if open_above {
            "document.body.classList.add('menu-open-above');"
        } else {
            "document.body.classList.remove('menu-open-above');"
        };
        window
            .eval(layout_script)
            .map_err(|error| format!("Failed to place the widget menu: {error}"))?;
        if let Err(error) = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                WINDOW_WIDTH,
                EXPANDED_HEIGHT,
            ))) {
            let _ = window.eval("document.body.classList.remove('menu-open-above');");
            return Err(format!("Failed to expand the widget menu: {error}"));
        }
        if target_y != origin.y {
            if let Err(error) = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x: origin.x,
                    y: target_y,
                })) {
                let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                    WINDOW_WIDTH,
                    COLLAPSED_HEIGHT,
                )));
                let _ = window.eval("document.body.classList.remove('menu-open-above');");
                return Err(format!("Failed to keep the widget menu on screen: {error}"));
            }
        }
        *saved_origin = Some(origin);
    }

    #[cfg(windows)]
    if let Ok(hwnd_ptr) = window.hwnd() {
        let hwnd = windows::Win32::Foundation::HWND(hwnd_ptr.0 as *mut _);
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE,
                SWP_SHOWWINDOW,
            };
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    Ok(())
}

#[tauri::command]
fn openScratchpadWindow(app: AppHandle) -> Result<(), String> {
    crate::window::auxiliary::get_or_create_scratchpad(&app).map(|_| ())
}

#[tauri::command]
fn closeScratchpadWindow(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if state.is_toggle_recording.load(std::sync::atomic::Ordering::SeqCst) {
        stopRecording(app.clone(), state);
    }
    if let Some(sp) = app.get_webview_window("scratchpad") {
        crate::window::auxiliary::schedule_scratchpad_close(sp);
    }
    Ok(())
}

#[tauri::command]
fn minimizeScratchpadWindow(app: AppHandle) {
    if let Some(sp) = app.get_webview_window("scratchpad") {
        let _ = sp.minimize();
    }
}

#[tauri::command]
fn toggleScratchpadPin(app: AppHandle, pin: bool) {
    if let Some(sp) = app.get_webview_window("scratchpad") {
        let _ = sp.set_always_on_top(pin);
    }
}

#[tauri::command]
fn insertScratchpadText(app: AppHandle, state: State<'_, Arc<AppState>>, text: String) {
    if text.trim().is_empty() {
        return;
    }
    let target_hwnd = {
        let target = state.target_window.lock();
        TextInjector::target_from_raw(target.map(|v| v as usize))
    };
    TextInjector::paste_text(&text, target_hwnd);
    emit_to_frontend(&app, "textInjected", Some(json!({ "method": "pasted" })));
}

#[tauri::command]
fn drainScratchpadInbox() -> Value {
    let notes = crate::storage::settings::drain_scratchpad_inbox_notes();
    json!({ "notes": notes })
}

#[tauri::command]
async fn searchLibrary(
    state: State<'_, Arc<AppState>>,
    query: String,
    kind: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<crate::storage::library::LibrarySearchPage, String> {
    let library = state.library.clone();
    let limit = limit.unwrap_or(30);
    let offset = offset.unwrap_or(0);
    tauri::async_runtime::spawn_blocking(move || {
        library.search(&query, kind.as_deref(), limit, offset)
    })
    .await
    .map_err(|error| format!("Library search worker failed: {error}"))?
    .map_err(|error| {
        let message = error.to_string();
        log_status(&format!("Library search failed: {message}"));
        message
    })
}

#[tauri::command]
async fn getLibraryItemContent(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Option<String>, String> {
    let library = state.library.clone();
    tauri::async_runtime::spawn_blocking(move || library.get_content(&id))
        .await
        .map_err(|error| format!("Library content worker failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn getScratchpadDraft(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::storage::library::ScratchpadDraft, String> {
    state.library.get_draft().map_err(|e| e.to_string())
}

#[tauri::command]
fn saveScratchpadDraft(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    content: String,
) -> Result<String, String> {
    let updated_at = state.library.save_draft(&content).map_err(|e| e.to_string())?;
    emit_to_frontend(&app, "libraryUpdated", None);
    Ok(updated_at)
}

#[tauri::command]
async fn polishSelectedText(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<Value, String> {
    let target_raw: Option<isize> = {
        let target = state.target_window.lock();
        *target
    };

    let selected_opt = {
        let hwnd = TextInjector::target_from_raw(target_raw.map(|v| v as usize));
        TextInjector::get_selected_text_from_target(hwnd)
    };

    let selected_text = match selected_opt {
        Some(txt) if !txt.trim().is_empty() => txt,
        _ => {
            emit_to_frontend(
                &app,
                "recordingSaved",
                Some(json!({ "isSilent": true, "message": "No text selected" })),
            );
            return Ok(json!({ "success": false, "message": "No text selected to polish" }));
        }
    };

    emit_to_frontend(
        &app,
        "transcriptionProcessing",
        Some(json!({
            "stage": "enhancing",
            "model": "Polishing...",
            "submodel": "AI Polish",
            "provider": "openrouter",
            "rawModel": "mistral-small-24b"
        })),
    );

    let settings = load_settings();
    let or_key = CredentialsProvider::get_openrouter_api_key().unwrap_or_default();
    if or_key.trim().is_empty() {
        emit_to_frontend(
            &app,
            "transcriptionError",
            Some(json!({ "message": "OpenRouter API Key required" })),
        );
        return Err("OpenRouter API Key required".to_string());
    }

    let polish_instructions = if !settings.formatting_instructions.trim().is_empty() {
        settings.formatting_instructions.clone()
    } else {
        "Polish the selected text directly. Improve grammar, phrasing, clarity, and tone while strictly preserving meaning and intent. Do not add conversational prefixes, quotes, or explanations.".to_string()
    };

    match state.openrouter.enhance_text(&or_key, &selected_text, &polish_instructions, None).await {
        Ok(enhanced) if !enhanced.trim().is_empty() => {
            let entries = state.dictionary.list_entries().unwrap_or_default();
            let final_text = apply_dictionary(&enhanced, &entries);

            let target_hwnd = TextInjector::target_from_raw(target_raw.map(|v| v as usize));
            let (pasted, mut actual_hwnd) = TextInjector::paste_text(&final_text, target_hwnd);
            if pasted {
                emit_to_frontend(&app, "textInjected", Some(json!({ "method": "pasted" })));
            } else {
                let (typed, type_hwnd) = TextInjector::type_unicode_text(&final_text, target_hwnd);
                if typed {
                    emit_to_frontend(&app, "textInjected", Some(json!({ "method": "typed" })));
                    actual_hwnd = type_hwnd.or(actual_hwnd);
                } else {
                    TextInjector::set_clipboard_text(&final_text);
                    emit_to_frontend(&app, "textInjected", Some(json!({ "method": "copied" })));
                }
            }

            let app_title = actual_hwnd.or(target_hwnd).and_then(TextInjector::get_window_title);
            let _ = state.history.insert_entry(
                &final_text,
                0.0,
                app_title.as_deref(),
                Some("mistralai/mistral-small-24b-instruct-2501"),
                Some("openrouter"),
            );

            Ok(json!({ "success": true, "text": final_text }))
        }
        Ok(_) => {
            emit_to_frontend(
                &app,
                "transcriptionError",
                Some(json!({ "message": "Enhancement produced empty text" })),
            );
            Err("Enhancement produced empty text".to_string())
        }
        Err(e) => {
            log::warn!("Polish failed: {}", e);
            emit_to_frontend(
                &app,
                "transcriptionError",
                Some(json!({ "message": format!("Polish failed: {}", e) })),
            );
            Err(e)
        }
    }
}

// -------------------------------------------------------------
// Spotlight & Voice AI Commands (Strictly Groq AI)
// -------------------------------------------------------------
#[tauri::command]
fn openSpotlightWindow(app: AppHandle) {
    let _ = crate::window::auxiliary::get_or_create_spotlight(&app);
}

#[tauri::command]
fn closeSpotlightWindow(app: AppHandle) {
    if let Some(sp) = app.get_webview_window("spotlight") {
        let _ = sp.hide();
        crate::window::auxiliary::trim_application_working_set_async();
    }
}

#[tauri::command]
async fn askSpotlightAi(prompt: String, includeContext: Option<bool>) -> Result<Value, String> {
    let context = if includeContext.unwrap_or(true) {
        let fg = TextInjector::get_foreground_window();
        let app_title = TextInjector::get_window_title(fg);
        let selection = TextInjector::get_selected_text_from_target(Some(fg));
        match (app_title, selection) {
            (Some(title), Some(sel)) => Some(format!("Application: {}\nSelected text: {}", title, sel)),
            (Some(title), None) => Some(format!("Application: {}", title)),
            (None, Some(sel)) => Some(format!("Selected text: {}", sel)),
            (None, None) => None,
        }
    } else {
        None
    };

    match crate::ai::spotlight::query_groq_spotlight(&prompt, context.as_deref()).await {
        Ok(ans) => Ok(json!({ "success": true, "answer": ans })),
        Err(e) => Ok(json!({ "success": false, "error": e })),
    }
}

#[tauri::command]
fn insertSpotlightText(app: AppHandle, text: String) -> Value {
    if let Some(sp) = app.get_webview_window("spotlight") {
        let _ = sp.hide();
    }
    std::thread::sleep(std::time::Duration::from_millis(60));
    TextInjector::paste_text(&text, None);
    let s = load_settings();
    if s.recording_sounds {
        crate::audio::cue::play_success(s.sound_volume);
    }
    emit_to_frontend(&app, "textInjected", Some(json!({ "method": "pasted" })));
    json!({ "success": true })
}

// -------------------------------------------------------------
// Audio Feedback Cue Preview Command
// -------------------------------------------------------------
#[tauri::command]
fn testAudioCue(soundType: Option<String>, volume: Option<f32>) -> Value {
    let s = load_settings();
    let vol = volume.unwrap_or(s.sound_volume);
    match soundType.as_deref() {
        Some("start") => crate::audio::cue::play_start(vol),
        Some("cancel") => crate::audio::cue::play_cancel(vol),
        _ => crate::audio::cue::play_success(vol),
    }
    json!({ "success": true })
}

// -------------------------------------------------------------
// Voice Snippets & Macro Expansions Commands
// -------------------------------------------------------------
#[tauri::command]
fn getSnippets(state: State<'_, Arc<AppState>>) -> Result<Vec<VoiceSnippet>, String> {
    state.snippets.list_snippets().map_err(|e| e.to_string())
}

#[tauri::command]
fn saveSnippet(state: State<'_, Arc<AppState>>, snippet: Value) -> Result<(), String> {
    let s: VoiceSnippet = serde_json::from_value(snippet).map_err(|e| e.to_string())?;
    state.snippets.upsert_snippet(&s).map_err(|e| e.to_string())
}

#[tauri::command]
fn deleteSnippet(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.snippets.delete_snippet(&id).map_err(|e| e.to_string())
}

fn sync_wake_word_engine(app: &AppHandle, state: &Arc<AppState>, force_restart: bool) {
    let settings = load_settings();
    let mut ww = state.wake_word.lock();

    if !settings.wake_word_enabled {
        if ww.is_running() {
            ww.stop();
            log_status("[WakeWord] Hands-free 'Hey Rift' listener stopped.");
        }
        return;
    }

    if ww.is_running() && !force_restart {
        return;
    }

    let manual_flag = state.recorder.lock().is_recording_arc();
    let app_for_level = app.clone();
    let app_for_act = app.clone();
    let app_for_cancel = app.clone();
    let app_for_done = app.clone();
    let state_for_act = Arc::clone(state);
    let state_for_done = Arc::clone(state);

    let last_level = Arc::new(std::sync::Mutex::new(std::time::Instant::now()));

    let res = ww.start(
        settings.microphone_device_id.as_deref(),
        manual_flag,
        move |level| {
            let now = std::time::Instant::now();
            let should_emit = if let Ok(mut l) = last_level.try_lock() {
                if now.duration_since(*l).as_millis() >= 40 {
                    *l = now;
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if should_emit {
                emit_to_frontend(&app_for_level, "audioLevel", Some(json!(level)));
            }
        },
        move || {
            log_status("[WakeWord] 'Hey Rift' detected -> starting hands-free recording");
            let s = load_settings();
            if s.recording_sounds {
                crate::audio::cue::play_start(s.sound_volume);
            }
            crate::audio::mute::mute_system_audio_delayed(120);

            let target_hwnd = TextInjector::get_foreground_window();
            {
                let mut target = state_for_act.target_window.lock();
                *target = Some(target_hwnd.0 as isize);
            }
            start_dynamic_focus_tracker(Arc::clone(&state_for_act));

            emit_to_frontend(&app_for_act, "recordingStarted", None);
            emit_to_frontend(
                &app_for_act,
                "toast",
                Some(json!({ "message": "🎧 'Hey Rift' — Listening...", "type": "info" })),
            );

            if let Some(key) = CredentialsProvider::get_openrouter_api_key() {
                state_for_act.openrouter.prewarm(&key);
            }
            if let Some(key) = CredentialsProvider::get_api_key() {
                state_for_act.groq.prewarm(&key);
            }
        },
        move || {
            log_status("[WakeWord] Hands-free recording cancelled (no speech after wake word)");
            crate::audio::mute::unmute_system_audio();
            stop_dynamic_focus_tracker();
            let s = load_settings();
            if s.recording_sounds {
                crate::audio::cue::play_cancel(s.sound_volume);
            }
            emit_to_frontend(&app_for_cancel, "recordingCancelled", None);
        },
        move |samples_16k| {
            log_status("[WakeWord] Auto-silence completion triggered -> processing utterance");
            crate::audio::mute::unmute_system_audio();
            emit_to_frontend(&app_for_done, "recordingStopped", None);

            let result = crate::audio::capture::process_raw_16k_samples(samples_16k);
            if result.is_short_press || result.is_silent {
                stop_dynamic_focus_tracker();
                let s = load_settings();
                if s.recording_sounds {
                    crate::audio::cue::play_cancel(s.sound_volume);
                }
                emit_to_frontend(&app_for_done, "recordingCancelled", None);
                return;
            }

            process_and_transcribe(
                app_for_done.clone(),
                Arc::clone(&state_for_done),
                Arc::from(result.wav_bytes),
                result.duration_seconds,
            );
        },
    );

    if let Err(e) = res {
        log_status(&format!("[WakeWord] Failed to start listener: {}", e));
    } else {
        log_status("[WakeWord] Hands-free 'Hey Rift' listener active.");
    }
}

// -------------------------------------------------------------
// Settings Commands
// -------------------------------------------------------------
#[tauri::command]
fn getSettings() -> Value {
    let settings = load_settings();
    json!({
        "pushToTalkKey": settings.push_to_talk_key,
        "pushToTalkMode": settings.push_to_talk_mode,
        "pushToTalkCombo": settings.push_to_talk_combo,
        "toggleRecordingKey": settings.toggle_recording_key,
        "widgetVisibility": settings.widget_visibility,
        "widgetStyle": settings.widget_style,
        "widgetTranscribingStyle": settings.widget_transcribing_style,
        "widget_transcribing_style": settings.widget_transcribing_style,
        "followCursorAcrossMonitors": settings.follow_cursor_across_monitors,
        "lockWidgetPosition": settings.lock_widget_position,
        "widgetCustomPosition": settings.widget_custom_position,
        "soundFeedback": settings.recording_sounds,
        "recordingSounds": settings.recording_sounds,
        "soundVolume": settings.sound_volume,
        "pasteDirectly": !settings.copy_instead_of_type,
        "copyInsteadOfType": settings.copy_instead_of_type,
        "launchAtLogin": settings.launch_at_login,
        "launchOnStartup": settings.launch_on_startup,
        "transcriptionMode": settings.transcription_mode,
        "cloudModel": settings.inference_model,
        "inferenceModel": settings.inference_model,
        "sttProvider": settings.stt_provider,
        "inferenceProfile": settings.local_inference_profile,
        "localModelProfile": settings.local_inference_profile,
        "localInferenceProfile": settings.local_inference_profile,
        "localWhisperUnloadOnIdle": settings.local_whisper_unload_on_idle,
        "local_whisper_unload_on_idle": settings.local_whisper_unload_on_idle,
        "microphoneDeviceId": settings.microphone_device_id,
        "autoEnhancePrompt": settings.auto_enhance_prompt,
        "formattingInstructions": settings.formatting_instructions,
        "pauseOtherAudioWhileTalking": settings.pause_other_audio_while_talking,
        "onboardingCompleted": settings.onboarding_completed,
        "onboarding_completed": settings.onboarding_completed,
        "micBoost": settings.mic_boost,
        "mic_boost": settings.mic_boost,
        "voiceCommandsEnabled": settings.voice_commands_enabled,
        "voiceSnippetsEnabled": settings.voice_snippets_enabled,
        "quickActionsEnabled": settings.quick_actions_enabled,
        "contextAwareFormatting": settings.context_aware_formatting,
        "noiseCancellationEnabled": settings.noise_cancellation_enabled,
        "clipboardHistoryEnabled": settings.clipboard_history_enabled,
        "clipboardMaxItems": settings.clipboard_max_items,
        "ambientMemoryEnabled": settings.ambient_memory_enabled,
        "whisperModeEnabled": settings.whisper_mode_enabled,
        "whisper_mode_enabled": settings.whisper_mode_enabled,
        "intentRouterEnabled": settings.intent_router_enabled,
        "intent_router_enabled": settings.intent_router_enabled,
        "wakeWordEnabled": settings.wake_word_enabled,
        "wake_word_enabled": settings.wake_word_enabled,
        "tenglishModeEnabled": settings.tenglish_mode_enabled,
        "tenglish_mode_enabled": settings.tenglish_mode_enabled,
        "tenglishSttTier": settings.tenglish_stt_tier,
        "tenglish_stt_tier": settings.tenglish_stt_tier,
        "tenglishTranslitEngine": settings.tenglish_translit_engine,
        "tenglish_translit_engine": settings.tenglish_translit_engine,
    })
}


#[tauri::command]
fn saveSettings(app: AppHandle, state: State<'_, Arc<AppState>>, patch: Value) -> Result<Value, String> {
    let mut settings = load_settings();
    let previous_clipboard_history_enabled = settings.clipboard_history_enabled;
    let previous_clipboard_max_items = settings.clipboard_max_items;
    let mut mic_changed = false;
    let mut preview_transcribing_style = false;
    if let Some(obj) = patch.as_object() {
        if let Some(v) = obj.get("tenglishModeEnabled").and_then(|x| x.as_bool()).or_else(|| obj.get("tenglish_mode_enabled").and_then(|x| x.as_bool())) {
            settings.tenglish_mode_enabled = v;
        }
        if let Some(v) = obj.get("whisperModeEnabled").and_then(|x| x.as_bool()).or_else(|| obj.get("whisper_mode_enabled").and_then(|x| x.as_bool())) {
            settings.whisper_mode_enabled = v;
        }
        if let Some(v) = obj.get("intentRouterEnabled").and_then(|x| x.as_bool()).or_else(|| obj.get("intent_router_enabled").and_then(|x| x.as_bool())) {
            settings.intent_router_enabled = v;
        }
        if let Some(v) = obj.get("wakeWordEnabled").and_then(|x| x.as_bool()).or_else(|| obj.get("wake_word_enabled").and_then(|x| x.as_bool())) {
            settings.wake_word_enabled = v;
        }
        if let Some(v) = obj.get("noiseCancellationEnabled").and_then(|x| x.as_bool()).or_else(|| obj.get("noise_cancellation_enabled").and_then(|x| x.as_bool())) {
            settings.noise_cancellation_enabled = v;
        }
        if let Some(v) = obj.get("contextAwareFormatting").and_then(|x| x.as_bool()).or_else(|| obj.get("context_aware_formatting").and_then(|x| x.as_bool())) {
            settings.context_aware_formatting = v;
        }
        if let Some(v) = obj.get("clipboardHistoryEnabled").and_then(|x| x.as_bool()).or_else(|| obj.get("clipboard_history_enabled").and_then(|x| x.as_bool())) {
            settings.clipboard_history_enabled = v;
        }
        if let Some(v) = obj.get("clipboardMaxItems").and_then(|x| x.as_u64()).or_else(|| obj.get("clipboard_max_items").and_then(|x| x.as_u64())) {
            settings.clipboard_max_items = (v as usize).clamp(1, 5_000);
        }
        if let Some(v) = obj.get("ambientMemoryEnabled").and_then(|x| x.as_bool()).or_else(|| obj.get("ambient_memory_enabled").and_then(|x| x.as_bool())) {
            settings.ambient_memory_enabled = v;
        }
        if let Some(v) = obj.get("tenglishSttTier").and_then(|x| x.as_str()).or_else(|| obj.get("tenglish_stt_tier").and_then(|x| x.as_str())) {
            settings.tenglish_stt_tier = v.to_string();
        }
        if let Some(v) = obj.get("tenglishTranslitEngine").and_then(|x| x.as_str()).or_else(|| obj.get("tenglish_translit_engine").and_then(|x| x.as_str())) {
            settings.tenglish_translit_engine = v.to_string();
        }
        if let Some(v) = obj.get("micBoost").and_then(|x| x.as_str()).or_else(|| obj.get("mic_boost").and_then(|x| x.as_str())) {
            settings.mic_boost = v.to_string();
        }
        if let Some(v) = obj.get("onboardingCompleted").and_then(|x| x.as_bool()).or_else(|| obj.get("onboarding_completed").and_then(|x| x.as_bool())) {
            settings.onboarding_completed = v;
        }
        if let Some(v) = obj.get("pushToTalkKey").and_then(|x| x.as_str()) {
            settings.push_to_talk_key = v.to_string();
        }
        if let Some(v) = obj.get("pushToTalkMode").and_then(|x| x.as_str()) {
            settings.push_to_talk_mode = v.to_string();
        }
        if let Some(v) = obj.get("pushToTalkCombo").and_then(|x| x.as_str()) {
            settings.push_to_talk_combo = v.to_string();
        }
        if let Some(v) = obj.get("toggleRecordingKey").or_else(|| obj.get("toggle_recording_key")) {
            if v.is_null() {
                settings.toggle_recording_key = None;
            } else if let Some(s) = v.as_str() {
                let trimmed = s.trim();
                if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("disabled") || trimmed.eq_ignore_ascii_case("none") {
                    settings.toggle_recording_key = None;
                } else {
                    settings.toggle_recording_key = Some(trimmed.to_string());
                }
            }
        }
        if let Some(v) = obj.get("widgetVisibility").and_then(|x| x.as_str()) {
            settings.widget_visibility = v.to_string();
        }
        if let Some(v) = obj.get("widgetStyle").and_then(|x| x.as_str()) {
            settings.widget_style = v.to_string();
        }
        if let Some(v) = obj.get("widgetTranscribingStyle").and_then(|x| x.as_str()).or_else(|| obj.get("widget_transcribing_style").and_then(|x| x.as_str())) {
            settings.widget_transcribing_style = v.to_string();
            preview_transcribing_style = true;
        }
        if let Some(v) = obj.get("followCursorAcrossMonitors").and_then(|x| x.as_bool()) {
            settings.follow_cursor_across_monitors = v;
        }
        if let Some(v) = obj.get("lockWidgetPosition").and_then(|x| x.as_bool()) {
            settings.lock_widget_position = v;
        }
        if let Some(v) = obj.get("soundFeedback").and_then(|x| x.as_bool()) {
            settings.recording_sounds = v;
        }
        if let Some(v) = obj.get("recordingSounds").and_then(|x| x.as_bool()) {
            settings.recording_sounds = v;
        }
        if let Some(v) = obj.get("soundVolume").and_then(|x| x.as_f64()) {
            settings.sound_volume = v as f32;
        }
        if let Some(v) = obj.get("voiceCommandsEnabled").and_then(|x| x.as_bool()) {
            settings.voice_commands_enabled = v;
        }
        if let Some(v) = obj.get("voiceSnippetsEnabled").and_then(|x| x.as_bool()) {
            settings.voice_snippets_enabled = v;
        }
        if let Some(v) = obj.get("quickActionsEnabled").and_then(|x| x.as_bool()).or_else(|| obj.get("quick_actions_enabled").and_then(|x| x.as_bool())) {
            settings.quick_actions_enabled = v;
        }

        if let Some(v) = obj.get("pasteDirectly").and_then(|x| x.as_bool()) {
            settings.copy_instead_of_type = !v;
        }
        if let Some(v) = obj.get("copyInsteadOfType").and_then(|x| x.as_bool()) {
            settings.copy_instead_of_type = v;
        }
        if let Some(v) = obj.get("cloudModel").and_then(|x| x.as_str()) {
            settings.inference_model = v.to_string();
            if v.starts_with("groq/") || v == "whisper-large-v3-turbo" || v == "whisper-large-v3" {
                settings.stt_provider = "groq".to_string();
            } else if v.contains('/') || v.contains("scribe") {
                settings.stt_provider = "openrouter".to_string();
            } else {
                settings.stt_provider = "groq".to_string();
            }
        }
        if let Some(v) = obj.get("inferenceModel").and_then(|x| x.as_str()) {
            settings.inference_model = v.to_string();
            if v.starts_with("groq/") || v == "whisper-large-v3-turbo" || v == "whisper-large-v3" {
                settings.stt_provider = "groq".to_string();
            } else if v.contains('/') || v.contains("scribe") {
                settings.stt_provider = "openrouter".to_string();
            } else {
                settings.stt_provider = "groq".to_string();
            }
        }
        if let Some(v) = obj.get("sttProvider").and_then(|x| x.as_str()) {
            let is_openrouter_model = settings.inference_model.starts_with("elevenlabs/")
                || settings.inference_model.starts_with("openai/")
                || settings.inference_model.starts_with("microsoft/")
                || settings.inference_model.contains("scribe");
            let is_groq_model = settings.inference_model.starts_with("groq/");
            if is_openrouter_model {
                settings.stt_provider = "openrouter".to_string();
            } else if is_groq_model {
                settings.stt_provider = "groq".to_string();
            } else {
                settings.stt_provider = v.to_string();
            }
        }
        if let Some(v) = obj.get("transcriptionMode").and_then(|x| x.as_str()) {
            settings.transcription_mode = v.to_string();
        }
        if let Some(v) = obj.get("inferenceProfile").and_then(|x| x.as_str()) {
            settings.local_inference_profile = v.to_string();
        }
        if let Some(v) = obj.get("localModelProfile").and_then(|x| x.as_str()) {
            settings.local_inference_profile = v.to_string();
        }
        if let Some(v) = obj.get("localInferenceProfile").and_then(|x| x.as_str()) {
            settings.local_inference_profile = v.to_string();
        }
        if let Some(v) = obj.get("localWhisperUnloadOnIdle").and_then(|x| x.as_bool()).or_else(|| obj.get("local_whisper_unload_on_idle").and_then(|x| x.as_bool())) {
            settings.local_whisper_unload_on_idle = v;
        }
        if let Some(v) = obj.get("pauseOtherAudioWhileTalking").and_then(|x| x.as_bool()) {
            settings.pause_other_audio_while_talking = v;
        }
        if let Some(v) = obj.get("autoEnhancePrompt").and_then(|x| x.as_bool()) {
            settings.auto_enhance_prompt = v;
        }
        if let Some(v) = obj.get("formattingInstructions").and_then(|x| x.as_str()) {
            settings.formatting_instructions = v.to_string();
        }
        if let Some(v) = obj.get("launchAtLogin").and_then(|x| x.as_bool()).or_else(|| obj.get("launchOnStartup").and_then(|x| x.as_bool())) {
            settings.launch_at_login = v;
            settings.launch_on_startup = v;
            update_windows_startup(v);
        }
        if let Some(v) = obj.get("microphoneDeviceId") {
            mic_changed = true;
            if v.is_null() {
                settings.microphone_device_id = None;
            } else if let Some(s) = v.as_str() {
                if s.trim().is_empty() {
                    settings.microphone_device_id = None;
                } else {
                    settings.microphone_device_id = Some(s.to_string());
                }
            } else if let Some(n) = v.as_i64() {
                settings.microphone_device_id = Some(n.to_string());
            }
        }
    }
    try_save_settings(&settings)?;

    // Reconcile capture immediately when its privacy setting changes. If starting or
    // stopping the listener fails, restore the persisted state and reject the save.
    let clipboard_configuration_changed =
        settings.clipboard_history_enabled != previous_clipboard_history_enabled
            || settings.clipboard_max_items != previous_clipboard_max_items;
    if clipboard_configuration_changed {
        let start_listener = || crate::ambient::clipboard_listener::start_clipboard_listener(
                state.clipboard.clone(),
                state.is_internal_pasting.clone(),
                app.clone(),
                settings.clipboard_max_items,
            );
        let listener_result = if settings.clipboard_history_enabled {
            if previous_clipboard_history_enabled
                && settings.clipboard_max_items != previous_clipboard_max_items
            {
                crate::ambient::clipboard_listener::stop_clipboard_listener()
                    .and_then(|()| start_listener())
            } else {
                start_listener()
            }
        } else {
            crate::ambient::clipboard_listener::stop_clipboard_listener()
        };

        if let Err(error) = listener_result {
            log::error!("Failed to apply clipboard history setting: {error}");
            settings.clipboard_history_enabled = previous_clipboard_history_enabled;
            settings.clipboard_max_items = previous_clipboard_max_items;
            if let Err(restore_error) = try_save_settings(&settings) {
                log::error!("Failed to restore settings after clipboard listener error: {restore_error}");
            }
            if previous_clipboard_history_enabled {
                if let Err(restart_error) = crate::ambient::clipboard_listener::start_clipboard_listener(
                    state.clipboard.clone(),
                    state.is_internal_pasting.clone(),
                    app.clone(),
                    previous_clipboard_max_items,
                ) {
                    log::error!("Failed to restore clipboard listener after configuration error: {restart_error}");
                }
            }
            emit_to_frontend(&app, "settingsChanged", Some(json!({
                "clipboardHistoryEnabled": settings.clipboard_history_enabled,
                "clipboardMaxItems": settings.clipboard_max_items,
            })));
            return Err(error);
        }
    }

    // Sync hands-free "Hey Rift" wake-word engine
    sync_wake_word_engine(&app, state.inner(), mic_changed);

    // Re-arm low-level keyboard hook with active keys dynamically
    let ptt_target = if settings.push_to_talk_mode == "combo" {
        &settings.push_to_talk_combo
    } else {
        &settings.push_to_talk_key
    };
    state.as_ref().hotkey.set_ptt_key(ptt_target);
    state.as_ref().hotkey.set_toggle_key(settings.toggle_recording_key.as_deref());

    let mut cfg = getWidgetConfig();
    if preview_transcribing_style {
        if let Some(cfg_obj) = cfg.as_object_mut() {
            cfg_obj.insert("previewTranscribingStyle".to_string(), json!(true));
        }
    }
    emit_to_frontend(&app, "widgetConfigChanged", Some(cfg));
    emit_to_frontend(&app, "usageUpdated", None);
    emit_to_frontend(&app, "settingsChanged", Some(json!({
        "inferenceModel": settings.inference_model,
        "cloudModel": settings.inference_model,
        "sttProvider": settings.stt_provider,
        "transcriptionMode": settings.transcription_mode,
        "inferenceProfile": settings.local_inference_profile,
        "localModelProfile": settings.local_inference_profile,
        "localWhisperUnloadOnIdle": settings.local_whisper_unload_on_idle,
        "tenglishModeEnabled": settings.tenglish_mode_enabled,
        "whisperModeEnabled": settings.whisper_mode_enabled,
        "intentRouterEnabled": settings.intent_router_enabled,
        "wakeWordEnabled": settings.wake_word_enabled,
        "pushToTalkKey": settings.push_to_talk_key,
        "pushToTalkMode": settings.push_to_talk_mode,
        "pushToTalkCombo": settings.push_to_talk_combo,
        "toggleRecordingKey": settings.toggle_recording_key,
        "widgetStyle": settings.widget_style,
        "widgetTranscribingStyle": settings.widget_transcribing_style,
        "widget_transcribing_style": settings.widget_transcribing_style,
        "clipboardHistoryEnabled": settings.clipboard_history_enabled,
        "clipboardMaxItems": settings.clipboard_max_items,
    })));
    Ok(json!({ "success": true }))
}

#[tauri::command]
fn toggleTenglishMode(app: AppHandle) -> Value {
    let mut settings = load_settings();
    settings.tenglish_mode_enabled = !settings.tenglish_mode_enabled;
    save_settings(&settings);
    emit_to_frontend(&app, "settingsChanged", Some(json!({
        "tenglishModeEnabled": settings.tenglish_mode_enabled,
    })));
    emit_to_frontend(&app, "toast", Some(json!({
        "message": if settings.tenglish_mode_enabled { "Tenglish Mode ON (Telugu → English)" } else { "Tenglish Mode OFF (English)" },
        "type": "info"
    })));
    json!({ "success": true, "tenglishModeEnabled": settings.tenglish_mode_enabled })
}

// -------------------------------------------------------------
// Hotkey Commands (Live Capture & Rebinding)
// -------------------------------------------------------------
#[tauri::command]
fn getHotkeys() -> Value {
    let settings = load_settings();
    json!({
        "pushToTalk": if settings.push_to_talk_mode == "combo" { &settings.push_to_talk_combo } else { &settings.push_to_talk_key },
        "mode": settings.push_to_talk_mode,
        "toggleRecording": settings.toggle_recording_key,
        "showWidget": "Ctrl+Shift+Space",
    })
}

#[tauri::command]
fn startHotkeyCapture(app: AppHandle, state: State<'_, Arc<AppState>>, mode: Option<String>) -> Value {
    let app_capture = app.clone();
    let mode_str = mode.unwrap_or_else(|| "single".to_string());
    state.as_ref().hotkey.start_capture(&mode_str, move |key, combo, m| {
        emit_to_frontend(
            &app_capture,
            "hotkeyCaptured",
            Some(json!({ "key": key, "combo": combo, "mode": m })),
        );
    });
    json!({ "success": true, "mode": mode_str, "status": "listening" })
}

#[tauri::command]
fn cancelHotkeyCapture(state: State<'_, Arc<AppState>>) -> Value {
    state.as_ref().hotkey.cancel_capture();
    json!({ "success": true, "status": "cancelled" })
}

#[tauri::command]
fn saveHotkey(app: AppHandle, state: State<'_, Arc<AppState>>, action: String, combo: String) -> Value {
    let combo = combo.trim().to_string();
    if combo.is_empty() {
        return json!({ "success": false, "message": "Key name cannot be empty." });
    }

    let mut settings = load_settings();
    if action == "toggle_recording" {
        settings.toggle_recording_key = Some(combo.clone());
        state.as_ref().hotkey.set_toggle_key(Some(&combo));
    } else if action == "show_widget" {
        state.as_ref().hotkey.set_show_widget_key(&combo);
    } else {
        if combo.contains('+') {
            settings.push_to_talk_combo = combo.clone();
            settings.push_to_talk_mode = "combo".to_string();
        } else {
            settings.push_to_talk_key = combo.clone();
            settings.push_to_talk_mode = "single".to_string();
        }
        state.as_ref().hotkey.set_ptt_key(&combo);
    }
    save_settings(&settings);

    let cfg = getWidgetConfig();
    emit_to_frontend(&app, "widgetConfigChanged", Some(cfg));
    json!({ "success": true, "action": action, "combo": combo, "message": format!("Hotkey set to {}", combo) })
}

#[tauri::command]
fn clearHotkey(app: AppHandle, state: State<'_, Arc<AppState>>, action: String) -> Value {
    let mut settings = load_settings();
    if action == "toggle_recording" {
        settings.toggle_recording_key = None;
        state.as_ref().hotkey.set_toggle_key(None);
    } else {
        settings.push_to_talk_key = "Not set".to_string();
        settings.push_to_talk_combo = "Not set".to_string();
        state.as_ref().hotkey.set_ptt_key("");
    }
    save_settings(&settings);
    let cfg = getWidgetConfig();
    emit_to_frontend(&app, "widgetConfigChanged", Some(cfg));
    json!({ "success": true, "action": action, "message": "Hotkey cleared" })
}

#[tauri::command]
fn verifyHotkey(app: AppHandle, action: Option<String>) -> Value {
    let act = action.unwrap_or_else(|| "push_to_talk".to_string());
    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        emit_to_frontend(&app_clone, "hotkeyVerified", Some(json!({ "holdMs": 450, "action": act })));
    });
    json!({ "success": true, "status": "armed" })
}

// -------------------------------------------------------------
// Audio & Microphone Commands
// -------------------------------------------------------------
#[tauri::command]
fn getMicrophones() -> Value {
    let mics = list_input_devices();
    json!({ "microphones": mics })
}

#[tauri::command]
fn listMicrophones() -> Value {
    getMicrophones()
}

#[tauri::command]
fn setMicrophone(deviceId: i32) -> Value {
    let mut settings = load_settings();
    settings.microphone_device_id = Some(deviceId.to_string());
    save_settings(&settings);
    json!({ "success": true, "message": format!("Microphone updated to device {}", deviceId) })
}

// -------------------------------------------------------------
// Groq Key & Cloud Commands
// -------------------------------------------------------------
#[tauri::command]
fn getGroqKeyStatus() -> Value {
    match CredentialsProvider::get_api_key() {
        Some(k) if !k.is_empty() => {
            let masked = if k.len() > 8 {
                format!("gsk_...{}", &k[k.len() - 4..])
            } else {
                "gsk_...****".to_string()
            };
            json!({
                "isConfigured": true,
                "hasKey": true,
                "status": "configured",
                "maskedKey": masked,
            })
        }
        _ => json!({
            "isConfigured": false,
            "hasKey": false,
            "status": "not_configured",
            "maskedKey": "",
        }),
    }
}

#[tauri::command]
async fn testGroqKey(key: String) -> Value {
    let clean_key = CredentialsProvider::sanitize_key(&key);
    if clean_key.is_empty() {
        return json!({ "success": false, "valid": false, "latencyMs": 0, "message": "API key cannot be empty" });
    }

    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .tcp_nodelay(true)
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp = client
        .get("https://api.groq.com/openai/v1/models")
        .bearer_auth(&clean_key)
        .send()
        .await;

    let latency_ms = start.elapsed().as_millis() as u64;

    match resp {
        Ok(res) if res.status().is_success() => {
            json!({
                "success": true,
                "valid": true,
                "latencyMs": latency_ms,
                "message": format!("Groq API key is valid and connected! ({}ms)", latency_ms)
            })
        }
        Ok(res) => {
            let status = res.status();
            let msg = if status == reqwest::StatusCode::UNAUTHORIZED {
                "Invalid API Key. Please verify your key at console.groq.com/keys".to_string()
            } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                "Rate limit exceeded on Groq Cloud. Please try again shortly.".to_string()
            } else {
                format!("Groq API error ({})", status)
            };
            json!({
                "success": false,
                "valid": false,
                "latencyMs": latency_ms,
                "message": msg
            })
        }
        Err(e) => {
            json!({
                "success": false,
                "valid": false,
                "latencyMs": 0,
                "message": format!("Connection error: {}", e)
            })
        }
    }
}

#[tauri::command]
async fn testSavedGroqKey() -> Value {
    match CredentialsProvider::get_api_key() {
        Some(key) => testGroqKey(key).await,
        None => json!({ "success": false, "valid": false, "message": "No Groq API key is saved." }),
    }
}

#[tauri::command]
fn saveGroqKey(key: String) -> Value {
    let key = key.trim().to_string();
    if let Err(e) = CredentialsProvider::set_api_key(&key) {
        return json!({ "success": false, "message": e.to_string() });
    }
    json!({ "success": true, "message": "Groq API key saved." })
}

#[tauri::command]
fn deleteGroqKey() -> Value {
    match CredentialsProvider::delete_api_key() {
        Ok(()) => json!({ "success": true }),
        Err(error) => json!({ "success": false, "message": error }),
    }
}

#[tauri::command]
fn saveApiKey(key: String) -> Result<(), String> {
    CredentialsProvider::set_api_key(&key).map_err(|e| e.to_string())
}

// -------------------------------------------------------------
// OpenRouter Key & Cloud Commands
// -------------------------------------------------------------
#[tauri::command]
fn getOpenRouterKeyStatus() -> Value {
    match CredentialsProvider::get_openrouter_api_key() {
        Some(k) if !k.is_empty() => {
            let masked = if k.len() > 10 {
                format!("sk-or-...{}", &k[k.len() - 4..])
            } else {
                "sk-or-...****".to_string()
            };
            json!({
                "isConfigured": true,
                "hasKey": true,
                "status": "configured",
                "maskedKey": masked,
            })
        }
        _ => json!({
            "isConfigured": false,
            "hasKey": false,
            "status": "not_configured",
            "maskedKey": "",
        }),
    }
}

#[tauri::command]
async fn testOpenRouterKey(key: String) -> Value {
    let clean_key = CredentialsProvider::sanitize_key(&key);
    if clean_key.is_empty() {
        return json!({ "success": false, "valid": false, "latencyMs": 0, "message": "API key cannot be empty" });
    }

    let start = std::time::Instant::now();
    let or_client = OpenRouterClient::new();
    match or_client.verify_key(&clean_key).await {
        Ok(data) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            let usage = data.get("data").and_then(|d| d.get("usage")).and_then(|u| u.as_f64());
            let limit = data.get("data").and_then(|d| d.get("limit")).and_then(|l| l.as_f64());
            let is_free_tier = data.get("data").and_then(|d| d.get("is_free_tier")).and_then(|f| f.as_bool());

            let mut details = format!("OpenRouter key connected ({}ms)!", latency_ms);
            let mut balance_val = None;
            if let Ok(cinfo) = or_client.fetch_credits(&clean_key).await {
                balance_val = Some(cinfo.remaining_balance);
                details = format!("Connected! Credits remaining: ${:.2} (Used: ${:.2} / Total: ${:.2})", cinfo.remaining_balance, cinfo.total_usage, cinfo.total_credits);
            } else if let (Some(u), Some(l)) = (usage, limit) {
                let remaining = (l - u).max(0.0);
                details = format!("Connected! Credits remaining: ${:.2} (Used: ${:.2} / Limit: ${:.2})", remaining, u, l);
            } else if let Some(u) = usage {
                details = format!("Connected! Account usage to date: ${:.2}", u);
            }

            json!({
                "success": true,
                "valid": true,
                "latencyMs": latency_ms,
                "message": details,
                "usage": usage,
                "limit": limit,
                "balance": balance_val,
                "isFreeTier": is_free_tier,
                "data": data.get("data"),
            })
        }
        Err(e) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            json!({
                "success": false,
                "valid": false,
                "latencyMs": latency_ms,
                "message": e
            })
        }
    }
}

#[tauri::command]
async fn testSavedOpenRouterKey() -> Value {
    match CredentialsProvider::get_openrouter_api_key() {
        Some(key) => testOpenRouterKey(key).await,
        None => json!({ "success": false, "valid": false, "message": "No OpenRouter API key is saved." }),
    }
}

#[tauri::command]
fn saveOpenRouterKey(key: String) -> Value {
    let key = key.trim().to_string();
    if let Err(e) = CredentialsProvider::set_openrouter_api_key(&key) {
        return json!({ "success": false, "message": e.to_string() });
    }
    json!({ "success": true, "message": "OpenRouter API key saved." })
}

#[tauri::command]
fn deleteOpenRouterKey() -> Value {
    match CredentialsProvider::delete_openrouter_api_key() {
        Ok(()) => json!({ "success": true }),
        Err(error) => json!({ "success": false, "message": error }),
    }
}

#[tauri::command]
fn saveOpenRouterApiKey(key: String) -> Result<(), String> {
    CredentialsProvider::set_openrouter_api_key(&key).map_err(|e| e.to_string())
}


// -------------------------------------------------------------
// History Commands
// -------------------------------------------------------------
#[tauri::command]
fn getHistory(state: State<'_, Arc<AppState>>, filter: Option<Value>) -> Result<Vec<HistoryEntry>, String> {
    let limit = filter
        .as_ref()
        .and_then(|f| {
            if let Some(s) = f.as_str() {
                serde_json::from_str::<Value>(s).ok()
            } else {
                Some(f.clone())
            }
        })
        .and_then(|f| f.get("limit").and_then(|l| l.as_u64()))
        .map(|l| l as usize)
        .unwrap_or(50);

    state.as_ref().history.get_recent(limit).map_err(|e| e.to_string())
}

#[tauri::command]
async fn searchHistory(
    state: State<'_, Arc<AppState>>,
    query: String,
    filter: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<crate::storage::history::HistorySearchPage, String> {
    let history = state.history.clone();
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    tauri::async_runtime::spawn_blocking(move || {
        history.search_history(&query, &filter, limit, offset)
    })
    .await
    .map_err(|error| format!("History search worker failed: {error}"))?
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn deleteHistoryEntry(app: AppHandle, state: State<'_, Arc<AppState>>, id: Value) -> Value {
    let id_num: i64 = match id {
        Value::Number(n) => n.as_i64().unwrap_or(0),
        Value::String(s) => s.parse().unwrap_or(0),
        _ => 0,
    };
    let success = state.as_ref().history.delete_entry(id_num).is_ok();
    if success {
        emit_to_frontend(&app, "historyCleared", None);
    }
    json!({ "success": success, "id": id_num })
}

#[tauri::command]
fn clearHistory(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.as_ref().history.clear_all().map_err(|e| e.to_string())?;
    emit_to_frontend(&app, "historyCleared", None);
    Ok(())
}

#[tauri::command]
fn getUsageStats(state: State<'_, Arc<AppState>>) -> Value {
    get_usage_stats_val(&state)
}

// -------------------------------------------------------------
// Smart Clipboard History Commands
// -------------------------------------------------------------
#[tauri::command]
fn getClipboardHistory(
    state: State<'_, Arc<AppState>>,
    limit: Option<usize>,
    category: Option<String>,
    query: Option<String>,
) -> Result<Vec<crate::storage::clipboard::ClipboardEntry>, String> {
    state
        .clipboard
        .get_recent_clips(
            limit.unwrap_or(50),
            category.as_deref(),
            query.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn toggleClipPin(state: State<'_, Arc<AppState>>, id: i64) -> Result<bool, String> {
    state.clipboard.toggle_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn deleteClip(state: State<'_, Arc<AppState>>, id: i64) -> Result<(), String> {
    state.clipboard.delete_clip(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn clearUnpinnedClips(state: State<'_, Arc<AppState>>) -> Result<usize, String> {
    state.clipboard.clear_unpinned().map_err(|e| e.to_string())
}

#[tauri::command]
fn copyClipById(state: State<'_, Arc<AppState>>, id: i64) -> Result<(), String> {
    if let Ok(Some(clip)) = state.clipboard.get_clip_by_id(id) {
        state.is_internal_pasting.store(true, std::sync::atomic::Ordering::SeqCst);
        let success = TextInjector::set_clipboard_text(&clip.content);
        let flag = state.is_internal_pasting.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            flag.store(false, std::sync::atomic::Ordering::SeqCst);
        });
        if success {
            Ok(())
        } else {
            Err("Failed to copy to clipboard".to_string())
        }
    } else {
        Err("Clip not found".to_string())
    }
}

#[tauri::command]
fn pasteClipById(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
    id: i64,
) -> Result<(), String> {
    if let Ok(Some(clip)) = state.clipboard.get_clip_by_id(id) {
        let target_raw = *state.target_window.lock();
        let target_hwnd = TextInjector::target_from_raw(target_raw.map(|v| v as usize));

        state.is_internal_pasting.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = TextInjector::paste_text(&clip.content, target_hwnd);
        let flag = state.is_internal_pasting.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            flag.store(false, std::sync::atomic::Ordering::SeqCst);
        });

        let s = load_settings();
        if s.recording_sounds {
            crate::audio::cue::play_success(s.sound_volume);
        }

        let preview_clean = clip.preview.chars().take(50).collect::<String>();
        emit_to_frontend(&app, "toast", Some(json!({
            "message": format!("📋 Pasted clip #{}: \"{}\"", clip.id, preview_clean),
            "type": "info"
        })));

        Ok(())
    } else {
        Err("Clip not found".to_string())
    }
}

#[tauri::command]
fn getClipboardCounts(state: State<'_, Arc<AppState>>) -> Result<Value, String> {
    let (total, pinned) = state.clipboard.get_counts().map_err(|e| e.to_string())?;
    Ok(json!({ "total": total, "pinned": pinned }))
}


fn compute_remaining_usage(state: &AppState, settings: &crate::storage::settings::AppSettings) -> Value {
    let mode = settings.transcription_mode.to_lowercase();
    let is_openrouter = settings.inference_model.starts_with("elevenlabs/")
        || settings.inference_model.starts_with("openai/")
        || settings.inference_model.starts_with("microsoft/")
        || settings.inference_model.contains("scribe")
        || (settings.stt_provider == "openrouter" && !settings.inference_model.starts_with("groq/"));

    let primary_is_groq = !is_openrouter && (
        settings.stt_provider == "groq"
        || settings.inference_model.starts_with("groq/")
        || (mode == "auto" && !settings.inference_model.contains('/'))
    );

    if mode == "local" {
        return json!({
            "provider": "local",
            "model": settings.local_inference_profile,
            "quotaType": "unlimited",
            "remainingSeconds": -1.0,
            "formattedTime": "Unlimited",
            "subtext": format!("{} · Offline Whisper", settings.local_inference_profile),
            "dotColor": "#34d399",
            "balance": null,
            "remainingRequests": null
        });
    }

    if primary_is_groq {
        let (used_secs, used_reqs) = state.history.get_today_groq_audio_seconds().unwrap_or((0.0, 0));
        let memory_requests = state.whisper_requests_today.load(std::sync::atomic::Ordering::Relaxed) as u64;
        let total_used_reqs = used_reqs.max(memory_requests);
        let rate_info = state.groq.get_rate_limit_info();
        let remaining_requests = match rate_info.remaining_requests {
            Some(r) => r.min(2000u64.saturating_sub(total_used_reqs)),
            None => 2000u64.saturating_sub(total_used_reqs),
        };
        let daily_limit_seconds = 28800.0; // 8 hours daily limit
        let remaining_seconds = (daily_limit_seconds - used_secs).max(0.0);

        let formatted_time = if remaining_seconds >= 3600.0 {
            let h = (remaining_seconds / 3600.0) as u64;
            let m = ((remaining_seconds % 3600.0) / 60.0) as u64;
            format!("{}h {}m", h, m)
        } else if remaining_seconds >= 60.0 {
            let m = (remaining_seconds / 60.0) as u64;
            let s = (remaining_seconds % 60.0) as u64;
            format!("{}m {}s", m, s)
        } else {
            format!("{:.0}s", remaining_seconds)
        };

        let friendly_name = if settings.inference_model.contains("turbo") || settings.inference_model.is_empty() {
            "Groq Turbo"
        } else {
            "Groq Whisper"
        };

        let dot_color = if remaining_seconds > 1800.0 {
            "#38bdf8"
        } else if remaining_seconds > 300.0 {
            "#fbbf24"
        } else {
            "#f87171"
        };

        return json!({
            "provider": "groq",
            "model": settings.inference_model,
            "quotaType": "daily_free_tier",
            "remainingSeconds": remaining_seconds,
            "formattedTime": formatted_time,
            "subtext": format!("{} · {} reqs left today", friendly_name, remaining_requests),
            "dotColor": dot_color,
            "balance": null,
            "remainingRequests": remaining_requests
        });
    }

    // OpenRouter Cloud
    let or_key = CredentialsProvider::get_openrouter_api_key();
    if or_key.is_none() || or_key.as_ref().map(|k| k.trim().is_empty()).unwrap_or(true) {
        return json!({
            "provider": "openrouter",
            "model": settings.inference_model,
            "quotaType": "paid_credits",
            "remainingSeconds": 0.0,
            "formattedTime": "No Key",
            "subtext": "Configure in Settings",
            "dotColor": "#fbbf24",
            "balance": 0.0,
            "remainingRequests": null
        });
    }

    let cached = state.openrouter.get_cached_credits();
    let should_fetch = match cached.last_updated {
        None => true,
        Some(t) => t.elapsed() > std::time::Duration::from_secs(120),
    };
    if should_fetch {
        if let Some(ref key) = or_key {
            let client = state.openrouter.clone();
            let k = key.clone();
            tauri::async_runtime::spawn(async move {
                let _ = client.fetch_credits(&k).await;
            });
        }
    }

    let balance = cached.remaining_balance;
    let price_per_minute = if settings.inference_model.contains("scribe") {
        0.00092
    } else {
        0.006
    };
    let cost_per_sec = price_per_minute / 60.0;
    let remaining_seconds = if cost_per_sec > 0.0 && balance > 0.0 {
        balance / cost_per_sec
    } else {
        0.0
    };

    let formatted_time = if remaining_seconds >= 3600.0 {
        let h = (remaining_seconds / 3600.0) as u64;
        let m = ((remaining_seconds % 3600.0) / 60.0) as u64;
        format!("{}h {}m", h, m)
    } else if remaining_seconds >= 60.0 {
        let m = (remaining_seconds / 60.0) as u64;
        let s = (remaining_seconds % 60.0) as u64;
        format!("{}m {}s", m, s)
    } else {
        format!("{:.0}s", remaining_seconds)
    };

    let friendly_name = if settings.inference_model.contains("scribe") {
        "ElevenLabs Scribe v2"
    } else if settings.inference_model.contains("turbo") {
        "OpenAI Turbo"
    } else {
        "OpenAI Whisper"
    };

    let dot_color = if balance > 1.0 {
        "#818cf8"
    } else if balance > 0.20 {
        "#fbbf24"
    } else {
        "#f87171"
    };

    json!({
        "provider": "openrouter",
        "model": settings.inference_model,
        "quotaType": "paid_credits",
        "remainingSeconds": remaining_seconds,
        "formattedTime": formatted_time,
        "subtext": format!("{} · ${:.2} bal", friendly_name, balance),
        "dotColor": dot_color,
        "balance": balance,
        "remainingRequests": null
    })
}

#[tauri::command]
fn getModelRemainingUsage(state: State<'_, Arc<AppState>>) -> Value {
    let settings = load_settings();
    compute_remaining_usage(&state, &settings)
}

#[tauri::command]
fn getDashboardStats(state: State<'_, Arc<AppState>>) -> Result<Value, String> {
    let stats = state.history.get_dashboard_stats().map_err(|e| e.to_string())?;
    let settings = load_settings();
    let usage = get_usage_stats_val(&state);
    let today_requests = stats.today_requests.max(usage["whisper"]["requestsToday"].as_u64().unwrap_or(0));
    let remaining_usage = compute_remaining_usage(&state, &settings);

    Ok(json!({
        "totalWords": stats.total_words,
        "totalSpeakingSeconds": stats.total_speaking_seconds,
        "totalRecordings": stats.total_recordings,
        "avgPaceWpm": stats.avg_pace_wpm,
        "todayRequests": today_requests,
        "dailyLimit": 2000,
        "currentModel": settings.inference_model,
        "transcriptionMode": settings.transcription_mode,
        "sttProvider": settings.stt_provider,
        "localInferenceProfile": settings.local_inference_profile,
        "remainingUsage": remaining_usage,
    }))
}

// -------------------------------------------------------------
// Custom Dictionary Commands
// -------------------------------------------------------------
#[tauri::command]
fn getDictionary(state: State<'_, Arc<AppState>>) -> Result<Vec<DictionaryEntry>, String> {
    state.as_ref().dictionary.list_entries().map_err(|e| e.to_string())
}

#[tauri::command]
fn addDictionaryEntry(state: State<'_, Arc<AppState>>, entry: Value) -> Result<DictionaryEntry, String> {
    let phrase = entry.get("phrase").and_then(|v| v.as_str()).unwrap_or("");
    let replacement = entry.get("replacement").and_then(|v| v.as_str()).unwrap_or("");
    let tag = entry.get("tag").and_then(|v| v.as_str());
    state.as_ref().dictionary.add_entry(phrase, replacement, tag).map_err(|e| e.to_string())
}

#[tauri::command]
fn updateDictionaryEntry(state: State<'_, Arc<AppState>>, id: String, patch: Value) -> Result<(), String> {
    let phrase = patch.get("phrase").and_then(|v| v.as_str());
    let replacement = patch.get("replacement").and_then(|v| v.as_str());
    let enabled = patch.get("enabled").and_then(|v| v.as_bool());
    let tag = patch.get("tag").and_then(|v| v.as_str());
    state.as_ref().dictionary.update_entry(&id, phrase, replacement, enabled, tag).map_err(|e| e.to_string())
}

#[tauri::command]
fn deleteDictionaryEntry(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.as_ref().dictionary.delete_entry(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn importDictionary() -> Value {
    json!({ "success": true, "count": 0, "errors": [] })
}

#[tauri::command]
fn exportDictionary() -> Value {
    json!({ "success": true })
}

#[tauri::command]
fn testDictionary(state: State<'_, Arc<AppState>>, text: String) -> String {
    let entries = state.as_ref().dictionary.list_entries().unwrap_or_default();
    apply_dictionary(&text, &entries)
}

// -------------------------------------------------------------
// AI Models & Processing Commands
// -------------------------------------------------------------
#[tauri::command]
fn getLocalModelsStatus() -> Value {
    let models = stt::local::get_supported_models();
    json!({ "models": models })
}

#[tauri::command]
async fn downloadLocalModel(app: AppHandle, profile: String) -> Result<Value, String> {
    let prof = profile.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = stt::local::download_model_file(&prof, app_clone.clone()).await {
            stt::local::emit_to_frontend(
                &app_clone,
                "modelDownloadComplete",
                Some(json!({
                    "profile": prof,
                    "success": false,
                    "message": e
                })),
            );
        }
    });
    Ok(json!({ "success": true, "profile": profile, "status": "downloading" }))
}

#[tauri::command]
fn deleteLocalModel(profile: String) -> Value {
    match stt::local::delete_local_model_file(&profile) {
        Ok(_) => json!({ "success": true, "profile": profile }),
        Err(e) => json!({ "success": false, "profile": profile, "message": e }),
    }
}

#[tauri::command]
fn verifyLocalModel(profile: String) -> Value {
    match stt::local::verify_local_model(&profile) {
        Ok(val) => val,
        Err(e) => json!({ "success": false, "profile": profile, "message": e }),
    }
}

#[tauri::command]
fn getAvailableCloudModels() -> Value {
    json!({ "models": [
        "groq/whisper-large-v3-turbo",
        "groq/whisper-large-v3",
        "elevenlabs/scribe-v2",
        "openai/whisper-large-v3-turbo",
        "openai/whisper-large-v3"
    ] })
}

#[tauri::command]
fn getTranscriptionMode() -> Value {
    let settings = load_settings();
    json!({ "mode": settings.transcription_mode })
}

#[tauri::command]
fn setTranscriptionMode(app: AppHandle, mode: String) -> Value {
    let mut settings = load_settings();
    settings.transcription_mode = mode.clone();
    save_settings(&settings);
    emit_to_frontend(&app, "settingsChanged", Some(json!({
        "transcriptionMode": settings.transcription_mode,
        "inferenceModel": settings.inference_model,
        "cloudModel": settings.inference_model,
        "inferenceProfile": settings.local_inference_profile,
        "localModelProfile": settings.local_inference_profile,
    })));
    json!({ "success": true, "mode": mode })
}

#[tauri::command]
fn setLocalInferenceProfile(app: AppHandle, profile: String) -> Value {
    let mut settings = load_settings();
    settings.local_inference_profile = profile.clone();
    save_settings(&settings);
    emit_to_frontend(&app, "settingsChanged", Some(json!({
        "transcriptionMode": settings.transcription_mode,
        "inferenceProfile": settings.local_inference_profile,
        "localModelProfile": settings.local_inference_profile,
    })));
    json!({ "success": true, "profile": profile })
}

#[tauri::command]
async fn testEnhance(state: State<'_, Arc<AppState>>, text: String, instructions: String) -> Result<Value, String> {
    if let Some(or_key) = CredentialsProvider::get_openrouter_api_key() {
        if !or_key.trim().is_empty() {
            if let Ok(enhanced) = state.openrouter.enhance_text(&or_key, &text, &instructions, None).await {
                if !enhanced.trim().is_empty() {
                    return Ok(json!({
                        "success": true,
                        "enhancedText": enhanced.trim(),
                        "text": enhanced.trim()
                    }));
                }
            }
        }
    }

    if let Some(api_key) = CredentialsProvider::get_api_key() {
        if !api_key.trim().is_empty() {
            let client = reqwest::Client::new();
            let system_prompt = if instructions.trim().is_empty() {
                "You are an AI speech-to-text post-processor. Clean up the user's spoken transcript by removing filler words ('um', 'uh', 'you know'), fixing grammar and capitalization, and punctuating naturally without changing the core meaning. Return ONLY the polished text with no preamble or commentary.".to_string()
            } else {
                format!("You are an AI speech-to-text post-processor. Apply the following custom formatting instructions to the user's transcript:\n{}\n\nReturn ONLY the formatted text with no preamble or commentary.", instructions.trim())
            };

            let req_body = json!({
                "model": "llama-3.3-70b-versatile",
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": text }
                ],
                "temperature": 0.0
            });

            if let Ok(resp) = client.post("https://api.groq.com/openai/v1/chat/completions")
                .bearer_auth(&api_key.trim())
                .json(&req_body)
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(data) = resp.json::<Value>().await {
                        if let Some(content) = data["choices"][0]["message"]["content"].as_str() {
                            return Ok(json!({
                                "success": true,
                                "enhancedText": content.trim(),
                                "text": content.trim()
                            }));
                        }
                    }
                }
            }
        }
    }

    let cleaned = clean_transcript(&text);
    Ok(json!({
        "success": true,
        "enhancedText": cleaned,
        "text": cleaned
    }))
}

#[tauri::command]
fn testCleanup(text: String) -> String {
    clean_transcript(&text)
}

#[tauri::command]
fn copyToClipboard(text: String) -> bool {
    TextInjector::set_clipboard_text(&text)
}

#[tauri::command]
fn checkForUpdates() -> Value {
    json!({
        "hasUpdate": false,
        "version": "1.0.0",
        "message": "You are running the latest version of Rift (v1.0.0)."
    })
}

#[tauri::command]
fn isLaunchAtLoginActive() -> bool {
    load_settings().launch_at_login
}

fn update_windows_startup(enabled: bool) {
    #[cfg(windows)]
    {
        use std::process::Command;
        #[cfg(target_os = "windows")]
        use std::os::windows::process::CommandExt;

        if enabled {
            if let Ok(exe_path) = std::env::current_exe() {
                let exe_str = exe_path.to_string_lossy().to_string();
                let mut cmd = Command::new("reg");
                cmd.args(&[
                    "add",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    "Rift",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &format!("\"{}\"", exe_str),
                    "/f",
                ]);
                #[cfg(target_os = "windows")]
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                let _ = cmd.output();
            }
        } else {
            let mut cmd = Command::new("reg");
            cmd.args(&[
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "Rift",
                "/f",
            ]);
            #[cfg(target_os = "windows")]
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            let _ = cmd.output();
        }
    }
}

// -------------------------------------------------------------
// Window Management
// -------------------------------------------------------------
#[tauri::command]
fn openSettingsWindow(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        use tauri::Emitter;
        let _ = app.emit("navigateTo", "settings");
        show_and_focus_window(&window);
    }
}

#[tauri::command]
fn openOnboardingWindow(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval("window.location.href = 'screens/onboarding-welcome.html';");
        show_and_focus_window(&window);
    }
}

#[tauri::command]
fn closeSettingsWindow(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        crate::window::auxiliary::trim_application_working_set_async();
    }
}

#[tauri::command]
fn quitApplication(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn cancelRecording(app: AppHandle, state: State<'_, Arc<AppState>>) {
    let _transition = state.recording_transition.lock();
    crate::audio::mute::unmute_system_audio();
    state.is_toggle_recording.store(false, std::sync::atomic::Ordering::SeqCst);
    {
        let mut recorder = state.recorder.lock();
        recorder.cancel();
    }
    stop_dynamic_focus_tracker();
    let s = load_settings();
    if s.recording_sounds {
        crate::audio::cue::play_cancel(s.sound_volume);
    }
    emit_to_frontend(&app, "recordingCancelled", None);
}

#[tauri::command]
fn toggleAmbientMemory(_app: AppHandle, state: State<'_, Arc<AppState>>, enable: bool) {
    let mut recorder = state.ambient_recorder.lock();
    if enable {
        let _ = recorder.start();
        log_status("Ambient Memory started.");
    } else {
        recorder.stop();
        log_status("Ambient Memory stopped.");
    }
}

#[tauri::command]
fn startRecording(app: AppHandle, state: State<'_, Arc<AppState>>) -> Value {
    let _transition = state.recording_transition.lock();
    if state.is_toggle_recording.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::SeqCst,
        std::sync::atomic::Ordering::SeqCst,
    ).is_err() {
        return json!({ "success": true, "isRecording": true });
    }
    let s = load_settings();
    if s.recording_sounds {
        crate::audio::cue::play_start(s.sound_volume);
    }
    crate::audio::mute::mute_system_audio_delayed(180);
    let target_hwnd = TextInjector::get_foreground_window();
    let app_for_level = app.clone();
    let last_level = Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
    let boost = s.mic_boost_multiplier();
    let dev = s.microphone_device_id;
    let on_stop = recording_stop_handler(app.clone(), Arc::clone(&*state));
    let start_result = state.recorder.lock().start(dev.as_deref(), boost, move |level| {
        let now = std::time::Instant::now();
        let should_emit = if let Ok(mut l) = last_level.try_lock() {
            if now.duration_since(*l).as_millis() >= 40 {
                *l = now;
                true
            } else {
                false
            }
        } else {
            false
        };
        if should_emit {
            emit_to_frontend(&app_for_level, "audioLevel", Some(json!(level)));
        }
    }, on_stop);

    if let Err(error) = start_result {
        state.is_toggle_recording.store(false, std::sync::atomic::Ordering::SeqCst);
        crate::audio::mute::unmute_system_audio();
        if s.recording_sounds {
            crate::audio::cue::play_cancel(s.sound_volume);
        }
        log::error!("Failed to start audio capture: {error}");
        emit_to_frontend(&app, "recordingError", Some(json!({ "message": error })));
        return json!({ "success": false, "isRecording": false });
    }

    {
        let mut target = state.target_window.lock();
        if target.is_none() {
            *target = Some(target_hwnd.0 as isize);
        }
    }
    start_dynamic_focus_tracker(Arc::clone(&state));
    emit_to_frontend(&app, "recordingStarted", None);
    if let Some(key) = CredentialsProvider::get_openrouter_api_key() {
        state.openrouter.prewarm(&key);
    }
    if let Some(gkey) = CredentialsProvider::get_api_key() {
        state.groq.prewarm(&gkey);
    }
    json!({ "success": true, "isRecording": true })
}

#[tauri::command]
fn stopRecording(app: AppHandle, state: State<'_, Arc<AppState>>) {
    stop_recording_inner(app, Arc::clone(&*state));
}

fn recording_stop_handler(
    app: AppHandle,
    state: Arc<AppState>,
) -> impl Fn(crate::audio::capture::CaptureStopReason) + Send + Sync + 'static {
    move |reason| {
        let app = app.clone();
        let state = Arc::clone(&state);
        std::thread::spawn(move || {
            if matches!(reason, crate::audio::capture::CaptureStopReason::MaximumDuration) {
                emit_to_frontend(
                    &app,
                    "toast",
                    Some(json!({
                        "message": "Recording stopped at the 10-minute limit.",
                        "type": "info"
                    })),
                );
            }
            stop_recording_inner(app, state);
        });
    }
}

fn stop_recording_inner(app: AppHandle, state: Arc<AppState>) {
    let _transition = state.recording_transition.lock();
    if !state
        .is_toggle_recording
        .swap(false, std::sync::atomic::Ordering::SeqCst)
    {
        return;
    }
    crate::audio::mute::unmute_system_audio();
    emit_to_frontend(&app, "recordingStopped", None);
    let result = state.recorder.lock().stop();
    drop(_transition);

    std::thread::spawn(move || {
        if let Some(error) = result.error.as_ref() {
            stop_dynamic_focus_tracker();
            emit_to_frontend(&app, "recordingError", Some(json!({ "message": error })));
            return;
        }

        if result.is_short_press {
            stop_dynamic_focus_tracker();
            let settings = load_settings();
            if settings.recording_sounds {
                crate::audio::cue::play_cancel(settings.sound_volume);
            }
            emit_to_frontend(&app, "recordingCancelled", None);
            return;
        }

        if result.is_silent {
            stop_dynamic_focus_tracker();
            let settings = load_settings();
            if settings.recording_sounds {
                crate::audio::cue::play_cancel(settings.sound_volume);
            }
            emit_to_frontend(&app, "recordingSaved", Some(json!({ "isSilent": true })));
            return;
        }

        process_and_transcribe(
            app,
            state,
            Arc::from(result.wav_bytes),
            result.duration_seconds,
        );
    });
}

#[tauri::command]
fn toggleRecording(app: AppHandle, state: State<'_, Arc<AppState>>) -> Value {
    let is_rec = state.is_toggle_recording.load(std::sync::atomic::Ordering::SeqCst);
    if is_rec {
        stopRecording(app, state);
        json!({ "isRecording": false })
    } else {
        let result = startRecording(app, state);
        json!({ "isRecording": result.get("isRecording").and_then(Value::as_bool).unwrap_or(false) })
    }
}

#[tauri::command]
fn retryLastTranscription(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<Value, String> {
    let cached = {
        let guard = state.last_recording.lock();
        guard.clone()
    };

    let (wav_bytes, duration) = cached
        .ok_or_else(|| "No recording is available to retry in this session".to_string())?;

    process_and_transcribe(app, Arc::clone(&state), wav_bytes, duration);
    Ok(json!({ "success": true, "duration": duration }))
}

#[tauri::command]
fn hasSavedRecording(state: State<'_, Arc<AppState>>) -> Value {
    let cached = {
        let guard = state.last_recording.lock();
        guard.clone()
    };
    if let Some((_, dur)) = cached {
        return json!({ "hasRecording": true, "duration": dur });
    }
    json!({ "hasRecording": false, "duration": 0.0 })
}

// -------------------------------------------------------------
// Ambient Memory Logic
// -------------------------------------------------------------
async fn process_ambient_memory(
    app: AppHandle,
    state: Arc<AppState>,
    wav_bytes: Vec<u8>,
) {
    emit_to_frontend(&app, "transcriptionProgress", Some(json!({ "progress": 20 })));
    
    // 1. Transcribe the audio
    let transcript = {
        let groq_api_key = CredentialsProvider::get_api_key().unwrap_or_default();
        let _s = load_settings();
        let lang = "en".to_string();
        
        let client = reqwest::Client::new();
        let part = reqwest::multipart::Part::bytes(wav_bytes)
            .file_name("ambient.wav")
            .mime_str("audio/wav")
            .unwrap();
            
        let mut form = reqwest::multipart::Form::new()
            .text("model", "whisper-large-v3-turbo")
            .part("file", part)
            .text("response_format", "text");
            
        if lang != "auto" {
            form = form.text("language", lang);
        }

        let resp = client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .bearer_auth(groq_api_key)
            .multipart(form)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                r.text().await.unwrap_or_default()
            }
            _ => String::new(),
        }
    };

    if is_silence_or_hallucination(&transcript) {
        emit_to_frontend(&app, "ambientMemoryResult", Some(json!({ "error": "No speech detected in the last 3 minutes." })));
        return;
    }

    emit_to_frontend(&app, "transcriptionProgress", Some(json!({ "progress": 60 })));

    // 2. Send to LLM for summary
    let system_prompt = "You are an ambient memory assistant. Read the following transcript of the user's recent environment/meeting from the last 3 minutes. Provide a concise, bulleted summary of the key points, action items, or implicitly answer any question the user just asked. Do not include any preamble, just the summary.";
    
    let result = if let Some(or_key) = CredentialsProvider::get_openrouter_api_key().filter(|k| !k.trim().is_empty()) {
        state.openrouter.enhance_text(&or_key, &transcript, system_prompt, None).await.unwrap_or_default()
    } else if let Some(api_key) = CredentialsProvider::get_api_key().filter(|k| !k.trim().is_empty()) {
        let client = reqwest::Client::new();
        let req_body = json!({
            "model": "llama-3.3-70b-versatile",
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": transcript }
            ],
            "temperature": 0.0
        });
        
        let mut final_text = String::new();
        if let Ok(r) = client.post("https://api.groq.com/openai/v1/chat/completions").bearer_auth(&api_key).json(&req_body).send().await {
            if let Ok(data) = r.json::<Value>().await {
                if let Some(content) = data["choices"][0]["message"]["content"].as_str() {
                    final_text = content.trim().to_string();
                }
            }
        }
        final_text
    } else {
        transcript.clone()
    };

    if !result.is_empty() {
        emit_to_frontend(&app, "ambientMemoryResult", Some(json!({ "text": result, "transcript": transcript })));
    } else {
        emit_to_frontend(&app, "ambientMemoryResult", Some(json!({ "error": "Failed to process ambient memory." })));
    }
}

// -------------------------------------------------------------
// Shared Audio Capture & Transcription Routine
// -------------------------------------------------------------
fn process_and_transcribe(
    app: AppHandle,
    state: Arc<AppState>,
    wav_bytes: Arc<[u8]>,
    duration_seconds: f64,
) {
    // 1. Stage in-memory for instant retry
    {
        let mut last = state.last_recording.lock();
        *last = Some((Arc::clone(&wav_bytes), duration_seconds));
    }

    let settings = load_settings();
    let (initial_display_model, initial_provider) = if settings.tenglish_mode_enabled {
        if settings.stt_provider == "groq" {
            ("Whisper Large (Telugu · Groq)".to_string(), "groq".to_string())
        } else {
            ("ElevenLabs Scribe v2".to_string(), "openrouter".to_string())
        }
    } else if settings.transcription_mode == "local" {
        (format!("Local ({})", settings.local_inference_profile), "local".to_string())
    } else if settings.stt_provider == "groq" || settings.inference_model.starts_with("groq/") {
        let name = if settings.inference_model.contains("whisper-large-v3-turbo") || settings.inference_model.contains("turbo") || settings.inference_model.is_empty() {
            "Whisper Turbo (Groq)".to_string()
        } else {
            "Whisper Large (Groq)".to_string()
        };
        (name, "groq".to_string())
    } else if settings.inference_model.contains("scribe") {
        ("ElevenLabs Scribe v2".to_string(), "openrouter".to_string())
    } else if settings.inference_model.contains("whisper-large-v3-turbo") {
        ("Whisper Turbo (OpenRouter)".to_string(), "openrouter".to_string())
    } else if settings.inference_model.contains("whisper-large-v3") {
        ("Whisper Large (OpenRouter)".to_string(), "openrouter".to_string())
    } else {
        ("ElevenLabs Scribe v2".to_string(), "openrouter".to_string())
    };

    emit_to_frontend(&app, "recordingStopped", Some(json!({
        "model": &initial_display_model,
        "provider": &initial_provider,
        "rawModel": &settings.inference_model,
    })));
    emit_to_frontend(&app, "transcriptionProcessing", Some(json!({
        "stage": "transcribing",
        "model": &initial_display_model,
        "provider": &initial_provider,
        "rawModel": &settings.inference_model,
    })));

    tauri::async_runtime::spawn(async move {
        let mode = settings.transcription_mode.to_lowercase(); // "cloud", "local", "auto"
        let target_raw = {
            let target = state.target_window.lock();
            *target
        };
        let window_ctx = if settings.context_aware_formatting {
            crate::ambient::context::get_window_context(target_raw)
        } else {
            crate::ambient::context::WindowContext::default()
        };

        let mut transcribed_result: Result<(String, String, &'static str), String> = Err("No transcription result".to_string());
        let wav_for_local = Arc::clone(&wav_bytes);

        if mode == "local" {
            let local_profile = settings.local_inference_profile.clone();
            let res = tokio::task::spawn_blocking(move || {
                stt::local::transcribe_local_shared(wav_for_local, &local_profile)
            }).await.map_err(|e| e.to_string()).and_then(|r| r);

            match res {
                Ok(text) => {
                    transcribed_result = Ok((text, format!("local-{}", settings.local_inference_profile), "local"));
                }
                Err(e) => {
                    stop_dynamic_focus_tracker();
                    emit_to_frontend(&app, "transcriptionError", Some(json!({ "message": e, "canRetry": true, "duration": duration_seconds })));
                    emit_to_frontend(&app, "toast", Some(json!({
                        "message": "Local transcription error · Audio saved! Click to Retry",
                        "type": "warning",
                        "canRetry": true
                    })));
                    return;
                }
            }
        } else if mode == "cloud" {
            let or_key = CredentialsProvider::get_openrouter_api_key();
            let groq_key = CredentialsProvider::get_api_key();
            let provider = settings.stt_provider.to_lowercase();
            let target_lang = if settings.tenglish_mode_enabled { Some("te") } else { None };

            let (primary_is_groq, gmodel_str, or_model_str) = if settings.tenglish_mode_enabled {
                let has_groq = groq_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false);
                if has_groq && provider == "groq" {
                    (true, "whisper-large-v3".to_string(), "elevenlabs/scribe-v2".to_string())
                } else {
                    let om = if settings.inference_model.contains("whisper") || settings.tenglish_stt_tier.contains("turbo") {
                        "openai/whisper-large-v3-turbo".to_string()
                    } else {
                        "elevenlabs/scribe-v2".to_string()
                    };
                    (false, "whisper-large-v3".to_string(), om)
                }
            } else {
                let is_openrouter = settings.inference_model.starts_with("elevenlabs/")
                    || settings.inference_model.starts_with("openai/")
                    || settings.inference_model.starts_with("microsoft/")
                    || settings.inference_model.contains("scribe")
                    || (provider == "openrouter" && !settings.inference_model.starts_with("groq/"));

                let is_groq = !is_openrouter && (provider == "groq" || settings.inference_model.starts_with("groq/") || settings.inference_model.contains("whisper"));

                let gm = if let Some(m) = settings.inference_model.strip_prefix("groq/") {
                    m.to_string()
                } else if settings.inference_model.contains('/') {
                    "whisper-large-v3-turbo".to_string()
                } else {
                    settings.inference_model.clone()
                };
                let om = if settings.inference_model.is_empty() || settings.inference_model.starts_with("groq/") {
                    "elevenlabs/scribe-v2".to_string()
                } else {
                    settings.inference_model.clone()
                };
                (is_groq, gm, om)
            };
            let gmodel = &gmodel_str;
            let or_model = or_model_str;

            // Extract custom vocabulary & dictionary terms for acoustic keyword biasing
            let entries = state.dictionary.list_entries().unwrap_or_default();
            let mut custom_phrases: Vec<String> = Vec::new();
            if settings.tenglish_mode_enabled {
                custom_phrases.push("తెలుగు సంభాషణ, వాట్సాప్ చాట్, కాల్, ఆఫీస్, మీటింగ్, lunch, message, WhatsApp".to_string());
            }
            for e in &entries {
                if e.enabled {
                    if !e.phrase.trim().is_empty() {
                        custom_phrases.push(e.phrase.clone());
                    }
                    if !e.replacement.trim().is_empty() && e.replacement != e.phrase {
                        custom_phrases.push(e.replacement.clone());
                    }
                }
            }
            if settings.context_aware_formatting {
                let mut hints = window_ctx.get_domain_vocabulary_hints();
                custom_phrases.append(&mut hints);
            }
            let phrases_ref = if !custom_phrases.is_empty() { Some(custom_phrases.as_slice()) } else { None };
            let groq_prompt = if !custom_phrases.is_empty() { Some(custom_phrases.join(", ")) } else { None };

            let mut cloud_success = false;

            if primary_is_groq {
                // Primary Cloud Provider: Groq (Free · Ultra Fast)
                if let Some(ref gkey) = groq_key {
                    if !gkey.is_empty() {
                        let friendly_name = if settings.tenglish_mode_enabled {
                            if gmodel.contains("turbo") {
                                "Whisper Turbo (Telugu · Groq)"
                            } else {
                                "Whisper Large (Telugu · Groq)"
                            }
                        } else if gmodel.contains("turbo") {
                            "Whisper Turbo (Groq)"
                        } else {
                            "Whisper Large (Groq)"
                        };
                        emit_to_frontend(
                            &app,
                            "transcriptionProcessing",
                            Some(json!({
                                "stage": "transcribing",
                                "model": friendly_name,
                                "rawModel": gmodel,
                                "provider": "groq"
                            })),
                        );
                        match state.groq.transcribe(gkey, Arc::clone(&wav_bytes), gmodel, groq_prompt.as_deref(), target_lang).await {
                            Ok(raw_text) => {
                                transcribed_result = Ok((raw_text, gmodel.to_string(), "groq"));
                                cloud_success = true;
                            }
                            Err(err) => {
                                log::warn!("Groq transcription error: {}. Falling back to OpenRouter.", err);
                                if let Some(ref okey) = or_key {
                                    if !okey.is_empty() {
                                        emit_to_frontend(&app, "transcriptionProcessing", Some(json!({ "model": "OpenRouter (Backup)", "provider": "openrouter", "rawModel": &or_model, "isFallback": true })));
                                        match state.openrouter.transcribe(okey, Arc::clone(&wav_bytes), &or_model, phrases_ref, target_lang).await {
                                            Ok(raw_text) => {
                                                transcribed_result = Ok((raw_text, or_model.clone(), "openrouter"));
                                                cloud_success = true;
                                                emit_to_frontend(
                                                    &app,
                                                    "toast",
                                                    Some(json!({ "message": "Transcribed via OpenRouter Fallback", "type": "info" })),
                                                );
                                            }
                                            Err(_) => {}
                                        }
                                    }
                                }
                                if !cloud_success {
                                    stop_dynamic_focus_tracker();
                                    emit_to_frontend(&app, "transcriptionError", Some(json!({ "message": format!("Groq error: {}", err), "canRetry": true, "duration": duration_seconds })));
                                    emit_to_frontend(&app, "toast", Some(json!({
                                        "message": "Network error · Audio saved! Click to Retry",
                                        "type": "warning",
                                        "canRetry": true
                                    })));
                                    return;
                                }
                            }
                        }
                    }
                }
                if !cloud_success {
                    stop_dynamic_focus_tracker();
                    emit_to_frontend(
                        &app,
                        "transcriptionError",
                        Some(json!({ "message": "Missing Groq API Key. Configure in Settings.", "canRetry": true, "duration": duration_seconds })),
                    );
                    emit_to_frontend(&app, "toast", Some(json!({
                        "message": "Missing Groq Key · Audio saved! Click to Retry",
                        "type": "warning",
                        "canRetry": true
                    })));
                    return;
                }
            } else {
                // Primary Cloud Provider: OpenRouter
                if let Some(ref key) = or_key {
                    if !key.is_empty() {
                        let friendly_name = if settings.tenglish_mode_enabled {
                            if or_model.contains("scribe") {
                                "ElevenLabs Scribe v2 (Telugu · Studio)"
                            } else {
                                "Whisper Large (Telugu · Studio)"
                            }
                        } else if or_model.contains("scribe") {
                            "ElevenLabs Scribe v2"
                        } else if or_model.contains("whisper-large-v3-turbo") || or_model.contains("turbo") {
                            "Whisper Turbo"
                        } else if or_model.contains("whisper-large-v3") {
                            "Whisper Large"
                        } else {
                            "OpenRouter STT"
                        };
                        emit_to_frontend(
                            &app,
                            "transcriptionProcessing",
                            Some(json!({
                                "stage": "transcribing",
                                "model": friendly_name,
                                "rawModel": &or_model,
                                "provider": "openrouter"
                            })),
                        );
                        match state.openrouter.transcribe(key, Arc::clone(&wav_bytes), &or_model, phrases_ref, target_lang).await {
                            Ok(raw_text) => {
                                transcribed_result = Ok((raw_text, or_model.clone(), "openrouter"));
                                cloud_success = true;
                            }
                            Err(err) => {
                                log::warn!("OpenRouter transcription error: {}. Falling back to Groq.", err);
                                emit_to_frontend(&app, "transcriptionProcessing", Some(json!({ "model": "Whisper Turbo (Backup)", "provider": "groq", "rawModel": "whisper-large-v3-turbo", "isFallback": true })));
                                if let Some(ref gkey) = groq_key {
                                    if !gkey.is_empty() {
                                        match state.groq.transcribe(gkey, Arc::clone(&wav_bytes), "whisper-large-v3-turbo", groq_prompt.as_deref(), target_lang).await {
                                            Ok(raw_text) => {
                                                transcribed_result = Ok((raw_text, "whisper-large-v3-turbo".to_string(), "groq"));
                                                cloud_success = true;
                                                emit_to_frontend(
                                                    &app,
                                                    "toast",
                                                    Some(json!({ "message": "Transcribed via Groq Fallback (OpenRouter unavailable)", "type": "info" })),
                                                );
                                            }
                                            Err(gerr) => {
                                                log::warn!("Groq fallback error: {}", gerr);
                                            }
                                        }
                                    }
                                }
                                if !cloud_success {
                                    stop_dynamic_focus_tracker();
                                    emit_to_frontend(&app, "transcriptionError", Some(json!({ "message": format!("OpenRouter error: {}", err), "canRetry": true, "duration": duration_seconds })));
                                    emit_to_frontend(&app, "toast", Some(json!({
                                        "message": "Network error · Audio saved! Click to Retry",
                                        "type": "warning",
                                        "canRetry": true
                                    })));
                                    return;
                                }
                            }
                        }
                    }
                }
                if !cloud_success && or_key.is_none() {
                    // Fall back to Groq if OpenRouter key not configured
                    if let Some(ref gkey) = groq_key {
                        if !gkey.is_empty() {
                            match state.groq.transcribe(gkey, Arc::clone(&wav_bytes), "whisper-large-v3-turbo", groq_prompt.as_deref(), target_lang).await {
                                Ok(raw_text) => {
                                    transcribed_result = Ok((raw_text, "whisper-large-v3-turbo".to_string(), "groq"));
                                    cloud_success = true;
                                }
                                Err(gerr) => {
                                    stop_dynamic_focus_tracker();
                                    emit_to_frontend(&app, "transcriptionError", Some(json!({ "message": gerr, "canRetry": true, "duration": duration_seconds })));
                                    emit_to_frontend(&app, "toast", Some(json!({
                                        "message": "Network error · Audio saved! Click to Retry",
                                        "type": "warning",
                                        "canRetry": true
                                    })));
                                    return;
                                }
                            }
                        }
                    }
                }
                if !cloud_success {
                    stop_dynamic_focus_tracker();
                    emit_to_frontend(
                        &app,
                        "transcriptionError",
                        Some(json!({ "message": "Missing OpenRouter API Key. Configure in Settings.", "canRetry": true, "duration": duration_seconds })),
                    );
                    emit_to_frontend(&app, "toast", Some(json!({
                        "message": "Missing OpenRouter Key · Audio saved! Click to Retry",
                        "type": "warning",
                        "canRetry": true
                    })));
                    return;
                }
            }
        } else {
            // "auto" mode: Dynamic Cascade based on user's selected cloud model:
            // If user selected Groq: Tier 1 Groq -> Tier 2 OpenRouter -> Tier 3 Local Whisper
            // If user selected OpenRouter: Tier 1 OpenRouter -> Tier 2 Groq -> Tier 3 Local Whisper
            let or_key = CredentialsProvider::get_openrouter_api_key();
            let groq_key = CredentialsProvider::get_api_key();
            let mut cascade_success = false;
            let target_lang = if settings.tenglish_mode_enabled { Some("te") } else { None };

            let (primary_is_groq, gmodel_str, or_model_str) = if settings.tenglish_mode_enabled {
                (false, "whisper-large-v3-turbo".to_string(), "elevenlabs/scribe-v2".to_string())
            } else {
                let is_openrouter = settings.inference_model.starts_with("elevenlabs/")
                    || settings.inference_model.starts_with("openai/")
                    || settings.inference_model.starts_with("microsoft/")
                    || settings.inference_model.contains("scribe")
                    || (settings.stt_provider == "openrouter" && !settings.inference_model.starts_with("groq/"));

                let is_groq = !is_openrouter && (settings.stt_provider == "groq" || settings.inference_model.starts_with("groq/") || settings.inference_model.contains("whisper"));

                let gm = if let Some(m) = settings.inference_model.strip_prefix("groq/") {
                    m.to_string()
                } else if settings.inference_model.contains('/') {
                    "whisper-large-v3-turbo".to_string()
                } else {
                    settings.inference_model.clone()
                };
                let om = if settings.inference_model.is_empty() || settings.inference_model.starts_with("groq/") {
                    "elevenlabs/scribe-v2".to_string()
                } else {
                    settings.inference_model.clone()
                };
                (is_groq, gm, om)
            };
            let gmodel = &gmodel_str;
            let or_model = or_model_str;

            // Extract custom vocabulary & dictionary terms for acoustic keyword biasing
            let entries = state.dictionary.list_entries().unwrap_or_default();
            let mut custom_phrases: Vec<String> = Vec::new();
            if settings.tenglish_mode_enabled {
                custom_phrases.push("తెలుగు సంభాషణ, వాట్సాప్ చాట్, కాల్, ఆఫీస్, మీటింగ్, lunch, message, WhatsApp".to_string());
            }
            for e in &entries {
                if e.enabled {
                    if !e.phrase.trim().is_empty() {
                        custom_phrases.push(e.phrase.clone());
                    }
                    if !e.replacement.trim().is_empty() && e.replacement != e.phrase {
                        custom_phrases.push(e.replacement.clone());
                    }
                }
            }
            if settings.context_aware_formatting {
                let mut hints = window_ctx.get_domain_vocabulary_hints();
                custom_phrases.append(&mut hints);
            }
            let phrases_ref = if !custom_phrases.is_empty() { Some(custom_phrases.as_slice()) } else { None };
            let groq_prompt = if !custom_phrases.is_empty() { Some(custom_phrases.join(", ")) } else { None };

            if primary_is_groq {
                // Tier 1: Try Groq (User Selected)
                if let Some(ref gkey) = groq_key {
                    if !gkey.is_empty() {
                        let friendly_name = if settings.tenglish_mode_enabled {
                            if gmodel.contains("turbo") {
                                "Whisper Turbo (Telugu · Groq)"
                            } else {
                                "Whisper Large (Telugu · Groq)"
                            }
                        } else if gmodel.contains("turbo") {
                            "Whisper Turbo (Groq)"
                        } else {
                            "Whisper Large (Groq)"
                        };
                        emit_to_frontend(
                            &app,
                            "transcriptionProcessing",
                            Some(json!({
                                "stage": "transcribing",
                                "model": friendly_name,
                                "rawModel": gmodel,
                                "provider": "groq"
                            })),
                        );
                        match state.groq.transcribe(gkey, Arc::clone(&wav_bytes), gmodel, groq_prompt.as_deref(), target_lang).await {
                            Ok(raw_text) => {
                                transcribed_result = Ok((raw_text, gmodel.to_string(), "groq"));
                                cascade_success = true;
                            }
                            Err(e) => {
                                log::warn!("Groq transcription error in auto mode: {}. Falling back to OpenRouter.", e);
                            }
                        }
                    }
                }

                // Tier 2: Fallback to OpenRouter
                if !cascade_success {
                    emit_to_frontend(&app, "transcriptionProcessing", Some(json!({ "model": "OpenRouter (Backup)", "provider": "openrouter", "rawModel": &or_model, "isFallback": true })));
                    if let Some(ref okey) = or_key {
                        if !okey.is_empty() {
                            match state.openrouter.transcribe(okey, Arc::clone(&wav_bytes), &or_model, phrases_ref, target_lang).await {
                                Ok(raw_text) => {
                                    transcribed_result = Ok((raw_text, or_model.clone(), "openrouter"));
                                    cascade_success = true;
                                    emit_to_frontend(
                                        &app,
                                        "toast",
                                        Some(json!({ "message": "Transcribed via OpenRouter Backup", "type": "info" })),
                                    );
                                }
                                Err(e) => {
                                    log::warn!("OpenRouter fallback error in auto mode: {}. Falling back to offline model.", e);
                                }
                            }
                        }
                    }
                }
            } else {
                // Tier 1: Try OpenRouter (User Selected)
                if let Some(ref okey) = or_key {
                    if !okey.is_empty() {
                        let friendly_name = if settings.tenglish_mode_enabled {
                            if or_model.contains("scribe") {
                                "ElevenLabs Scribe v2 (Telugu · Studio)"
                            } else {
                                "Whisper Large (Telugu · Studio)"
                            }
                        } else if or_model.contains("scribe") {
                            "ElevenLabs Scribe v2"
                        } else if or_model.contains("whisper-large-v3-turbo") || or_model.contains("turbo") {
                            "Whisper Turbo"
                        } else if or_model.contains("whisper-large-v3") {
                            "Whisper Large"
                        } else {
                            "OpenRouter STT"
                        };
                        emit_to_frontend(
                            &app,
                            "transcriptionProcessing",
                            Some(json!({
                                "stage": "transcribing",
                                "model": friendly_name,
                                "rawModel": &or_model,
                                "provider": "openrouter"
                            })),
                        );
                        match state.openrouter.transcribe(okey, Arc::clone(&wav_bytes), &or_model, phrases_ref, target_lang).await {
                            Ok(raw_text) => {
                                transcribed_result = Ok((raw_text, or_model.clone(), "openrouter"));
                                cascade_success = true;
                            }
                            Err(e) => {
                                log::warn!("OpenRouter transcription error in auto mode: {}. Falling back to Groq.", e);
                            }
                        }
                    }
                }

                // Tier 2: Fallback to Groq
                if !cascade_success {
                    emit_to_frontend(&app, "transcriptionProcessing", Some(json!({ "model": "Whisper Turbo (Backup)", "provider": "groq", "rawModel": "whisper-large-v3-turbo", "isFallback": true })));
                    if let Some(ref gkey) = groq_key {
                        if !gkey.is_empty() {
                            match state.groq.transcribe(gkey, Arc::clone(&wav_bytes), "whisper-large-v3-turbo", groq_prompt.as_deref(), target_lang).await {
                                Ok(raw_text) => {
                                    transcribed_result = Ok((raw_text, "whisper-large-v3-turbo".to_string(), "groq"));
                                    cascade_success = true;
                                    emit_to_frontend(
                                        &app,
                                        "toast",
                                        Some(json!({ "message": "Transcribed via Groq Backup", "type": "info" })),
                                    );
                                }
                                Err(e) => {
                                    log::warn!("Groq transcription error in auto mode: {}. Falling back to offline model.", e);
                                }
                            }
                        }
                    }
                }
            }

            // Tier 3: Try Offline Whisper Local
            if !cascade_success {
                emit_to_frontend(&app, "transcriptionProcessing", Some(json!({ "model": "Offline Whisper", "provider": "local", "rawModel": &settings.local_inference_profile, "isFallback": true })));
                let local_profile = settings.local_inference_profile.clone();
                let res = tokio::task::spawn_blocking(move || {
                stt::local::transcribe_local_shared(wav_for_local, &local_profile)
                }).await.map_err(|e| e.to_string()).and_then(|r| r);

                match res {
                    Ok(text) => {
                        transcribed_result = Ok((text, format!("offline-{}", settings.local_inference_profile), "local"));
                        emit_to_frontend(
                            &app,
                            "toast",
                            Some(json!({ "message": "Transcribed via Offline Whisper Fallback", "type": "info" })),
                        );
                    }
                    Err(e) => {
                        stop_dynamic_focus_tracker();
                        emit_to_frontend(
                            &app,
                            "transcriptionError",
                            Some(json!({ "message": format!("Transcription failed across cloud and local engines: {}", e), "canRetry": true, "duration": duration_seconds })),
                        );
                        emit_to_frontend(&app, "toast", Some(json!({
                            "message": "Network error · Audio saved! Click to Retry",
                            "type": "warning",
                            "canRetry": true
                        })));
                        return;
                    }
                }
            }
        }

        match transcribed_result {
            Ok((raw_text, used_model, engine_used)) => {
                let raw_text = crate::ai::intent_router::strip_wake_word_prefix(&raw_text);
                // Telugu -> Tenglish transliteration (Setup 1: ElevenLabs Scribe v2 + Google Gemini 2.5 Flash)
                let (raw_text, _translit_prov) = if !raw_text.trim().is_empty()
                    && (settings.tenglish_mode_enabled
                        || crate::text::transliterate::contains_telugu(&raw_text))
                {
                    let translit_label = "Gemini 2.5 Flash";
                    let translit_provider = "openrouter";

                    emit_to_frontend(
                        &app,
                        "transcriptionProcessing",
                        Some(json!({
                            "stage": "transliterating",
                            "model": translit_label,
                            "provider": translit_provider,
                            "rawModel": "google/gemini-2.5-flash",
                        })),
                    );
                    log_status(&format!("[Tenglish] Transliterating via {translit_label} ({translit_provider})"));
                    let or_key = CredentialsProvider::get_openrouter_api_key();
                    let groq_key = CredentialsProvider::get_api_key();
                    let (res_text, prov) = crate::text::transliterate::transliterate_telugu(
                        state.openrouter.client(),
                        &raw_text,
                        or_key.as_deref(),
                        groq_key.as_deref(),
                        "openrouter_gemini",
                        Some(engine_used),
                    )
                    .await;
                    log_status(&format!("[Tenglish] Transliterated ({prov}): '{res_text}'"));
                    (res_text, Some(prov))
                } else {
                    (raw_text, None)
                };

                let cleaned = clean_transcript(&raw_text);
                let cleaned = crate::ai::intent_router::strip_wake_word_prefix(&cleaned);

                // Apply custom dictionary
                let entries = state.dictionary.list_entries().unwrap_or_default();
                let mut final_text = apply_dictionary(&cleaned, &entries);

                // Guard: If transcription is empty or recognized as silence/hallucination, do NOT paste or save!
                // NOTE: English silence/hallucination phrases do not apply to Tenglish/Telugu!
                let is_hallucination = if settings.tenglish_mode_enabled {
                    false
                } else {
                    is_silence_or_hallucination(&final_text)
                };

                if final_text.trim().is_empty() || is_hallucination {
                    stop_dynamic_focus_tracker();
                    emit_to_frontend(
                        &app,
                        "recordingSaved",
                        Some(json!({ "isSilent": true, "message": "No speech detected" })),
                    );
                    return;
                }

                // -----------------------------------------------------------------
                // Universal Voice Intent Router (Modes D, B, C; Mode A = 0ms fast-path)
                // -----------------------------------------------------------------
                let mut intent_router_handled = false;
                if settings.intent_router_enabled && !settings.tenglish_mode_enabled {
                    // Mode D: Background Scratchpad Note ("Note to self...", "Add to scratchpad...")
                    if let Some(note_body) = crate::ai::intent_router::detect_background_note(&final_text) {
                        stop_dynamic_focus_tracker();
                        let note_persisted = match state.library.append_voice_note(&note_body) {
                            Ok(()) => true,
                            Err(error) => {
                                log::error!("Failed to persist Scratchpad voice note: {}", error);
                                crate::storage::settings::append_scratchpad_inbox_note(&note_body);
                                false
                            }
                        };
                        if note_persisted {
                            emit_to_frontend(&app, "libraryUpdated", None);
                        }
                        emit_to_frontend(
                            &app,
                            "appendScratchpadNote",
                            Some(json!({ "text": &note_body, "persisted": note_persisted })),
                        );

                        if settings.recording_sounds {
                            crate::audio::cue::play_success(settings.sound_volume);
                        }

                        let preview: String = note_body.chars().take(55).collect();
                        emit_to_frontend(
                            &app,
                            "toast",
                            Some(json!({
                                "message": format!("📝 Saved to Scratchpad: \"{}\"", preview),
                                "type": "info"
                            })),
                        );
                        emit_to_frontend(
                            &app,
                            "transcriptionSuccess",
                            Some(json!({
                                "text": &note_body,
                                "duration": duration_seconds,
                                "words": note_body.split_whitespace().count(),
                                "model": &used_model,
                                "engine": engine_used,
                                "targetApp": "Scratchpad (Background)",
                            })),
                        );
                        emit_to_frontend(
                            &app,
                            "transcriptionComplete",
                            Some(json!({
                                "text": &note_body,
                                "duration": duration_seconds,
                                "words": note_body.split_whitespace().count(),
                                "model": &used_model,
                                "engine": engine_used,
                                "targetApp": "Scratchpad (Background)",
                            })),
                        );
                        emit_to_frontend(
                            &app,
                            "textInjected",
                            Some(json!({ "method": "pasted" })),
                        );

                        let _ = state.history.insert_entry(
                            &format!("[Scratchpad Note] {}", note_body),
                            duration_seconds,
                            Some("Rift Scratchpad"),
                            Some(&used_model),
                            Some(engine_used),
                        );
                        if engine_used == "openrouter" {
                            let price_per_min = if used_model.contains("scribe") { 0.00092 } else { 0.006 };
                            state.openrouter.deduct_credits_local(duration_seconds, price_per_min);
                        }
                        emit_to_frontend(&app, "usageUpdated", None);
                        return;
                    }

                    // Mode B: Inline Selection Magic Edit (only probes target selection if phrase looks like an edit instruction)
                    if crate::ai::intent_router::looks_like_selection_instruction(&final_text) {
                        state.is_internal_pasting.store(true, std::sync::atomic::Ordering::SeqCst);
                        let sel_hwnd = TextInjector::target_from_raw(target_raw.map(|v| v as usize));
                        let selected_opt = TextInjector::get_selected_text_from_target(sel_hwnd);
                        state.is_internal_pasting.store(false, std::sync::atomic::Ordering::SeqCst);

                        if let Some(selected_text) = selected_opt.filter(|s| !s.trim().is_empty()) {
                            emit_to_frontend(
                                &app,
                                "transcriptionProcessing",
                                Some(json!({
                                    "stage": "enhancing",
                                    "model": "Intent: Magic Edit",
                                    "provider": "groq",
                                    "rawModel": "llama-3.3-70b-versatile"
                                })),
                            );
                            match crate::ai::intent_router::execute_selection_transform(
                                &final_text,
                                &selected_text,
                                &window_ctx,
                            )
                            .await
                            {
                                Ok(transformed) if !transformed.trim().is_empty() => {
                                    final_text = apply_dictionary(&transformed, &entries);
                                    intent_router_handled = true;
                                    emit_to_frontend(
                                        &app,
                                        "toast",
                                        Some(json!({
                                            "message": "🧭 Voice Intent: Rewrote highlighted selection",
                                            "type": "info"
                                        })),
                                    );
                                }
                                Err(e) => {
                                    log::warn!("[IntentRouter] Selection transform failed: {}", e);
                                }
                                _ => {}
                            }
                        }
                    }

                    // Mode C: Generative Write / Code / Command Output
                    if !intent_router_handled {
                        if let Some(gen_prompt) = crate::ai::intent_router::detect_generative_prompt(&final_text) {
                            emit_to_frontend(
                                &app,
                                "transcriptionProcessing",
                                Some(json!({
                                    "stage": "enhancing",
                                    "model": "Intent: Generating",
                                    "provider": "groq",
                                    "rawModel": "llama-3.3-70b-versatile"
                                })),
                            );
                            match crate::ai::intent_router::execute_generative_output(&gen_prompt, &window_ctx).await {
                                Ok(generated) if !generated.trim().is_empty() => {
                                    final_text = generated;
                                    intent_router_handled = true;
                                    emit_to_frontend(
                                        &app,
                                        "toast",
                                        Some(json!({
                                            "message": "🧭 Voice Intent: Generated output at cursor",
                                            "type": "info"
                                        })),
                                    );
                                }
                                Err(e) => {
                                    log::warn!("[IntentRouter] Generative output failed: {}", e);
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // AI Auto-Enhancement & Context-Aware Smart Formatting (Skip if already formatted by Tenglish or Intent Router)
                // Fast-Path: Normal transcribing pastes immediately (<0.1ms) without the blocking secondary LLM call!
                // Only invoke LLM when user explicitly enabled auto_enhance_prompt in Settings.
                let should_enhance = !intent_router_handled
                    && settings.auto_enhance_prompt
                    && !settings.tenglish_mode_enabled
                    && (settings.context_aware_formatting || !settings.formatting_instructions.trim().is_empty());

                if should_enhance {
                    let submodel_label = match window_ctx.domain {
                        crate::ambient::context::AppDomain::Coding => "Code Mode",
                        crate::ambient::context::AppDomain::Terminal => "Terminal Mode",
                        crate::ambient::context::AppDomain::Messaging => "Chat Mode",
                        crate::ambient::context::AppDomain::Document => "Doc Mode",
                        _ => "AI Polish",
                    };

                    let ctx_addon = if settings.context_aware_formatting {
                        window_ctx.get_system_prompt_addon()
                    } else {
                        None
                    };

                    let groq_key = CredentialsProvider::get_api_key();
                    let or_key = CredentialsProvider::get_openrouter_api_key();
                    let mut enhanced_text: Option<String> = None;

                    // Tier 1: Try Groq LPU (llama-3.1-8b-instant) for ~100ms ultra-fast polish if Groq key exists
                    if let Some(ref gkey) = groq_key {
                        if !gkey.trim().is_empty() {
                            emit_to_frontend(
                                &app,
                                "transcriptionProcessing",
                                Some(json!({
                                    "stage": "enhancing",
                                    "model": "Formatting...",
                                    "submodel": submodel_label,
                                    "provider": "groq",
                                    "rawModel": "llama-3.1-8b-instant"
                                })),
                            );

                            let custom_part = if !settings.formatting_instructions.trim().is_empty() {
                                format!("\nCustom formatting: {}", settings.formatting_instructions.trim())
                            } else {
                                String::new()
                            };
                            let prompt_system = format!(
                                "You are an AI speech-to-text post-processor for direct input dictation. Clean up the spoken transcript for direct typing into {}. Remove verbal fillers ('um', 'uh', 'you know'). Punctuate and capitalize naturally. Output ONLY the polished text. Never explain, never add quotes, never answer questions.{}\n{}",
                                submodel_label,
                                custom_part,
                                ctx_addon.as_deref().unwrap_or("")
                            );

                            let client = state.groq.client().clone();
                            let req_body = json!({
                                "model": "llama-3.1-8b-instant",
                                "messages": [
                                    { "role": "system", "content": prompt_system },
                                    { "role": "user", "content": &final_text }
                                ],
                                "temperature": 0.0,
                                "max_tokens": 1024
                            });

                            if let Ok(resp) = client.post("https://api.groq.com/openai/v1/chat/completions")
                                .bearer_auth(gkey.trim())
                                .json(&req_body)
                                .send()
                                .await
                            {
                                if let Ok(v) = resp.json::<serde_json::Value>().await {
                                    if let Some(content) = v["choices"][0]["message"]["content"].as_str() {
                                        let trimmed = content.trim().trim_matches('"').trim_matches('\'').trim();
                                        if !trimmed.is_empty() && !is_silence_or_hallucination(trimmed) {
                                            enhanced_text = Some(trimmed.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Tier 2: Fallback to OpenRouter if Groq was not available or failed
                    if enhanced_text.is_none() {
                        if let Some(ref okey) = or_key {
                            if !okey.trim().is_empty() {
                                emit_to_frontend(
                                    &app,
                                    "transcriptionProcessing",
                                    Some(json!({
                                        "stage": "enhancing",
                                        "model": "Formatting...",
                                        "submodel": submodel_label,
                                        "provider": "openrouter",
                                        "rawModel": "mistral-small-24b"
                                    })),
                                );
                                if let Ok(enhanced) = state.openrouter.enhance_text(okey, &final_text, &settings.formatting_instructions, ctx_addon.as_deref()).await {
                                    if !enhanced.trim().is_empty() && !is_silence_or_hallucination(&enhanced) {
                                        enhanced_text = Some(enhanced);
                                    }
                                }
                            }
                        }
                    }

                    if let Some(enhanced) = enhanced_text {
                        // Re-apply dictionary as a strict guarantee for custom user terms
                        final_text = apply_dictionary(&enhanced, &entries);
                    }
                }

                // Voice-Powered Quick Actions — check BEFORE voice editing commands and snippets
                if !intent_router_handled && settings.quick_actions_enabled {
                    let action = crate::text::quick_actions::detect_quick_action(&final_text);
                    if action != crate::text::quick_actions::QuickAction::None {
                        stop_dynamic_focus_tracker();
                        let action_hwnd = TextInjector::target_from_raw(target_raw.map(|v| v as usize));
                        let description = crate::text::quick_actions::execute_quick_action(&action, Some(&app), action_hwnd);
                        if settings.recording_sounds {
                            crate::audio::cue::play_success(settings.sound_volume);
                        }
                        emit_to_frontend(&app, "toast", Some(json!({
                            "message": format!("⚡ Quick Action: {}", description),
                            "type": "info"
                        })));
                        let _ = state.history.insert_entry(
                            &format!("[Quick Action] {}", description),
                            duration_seconds,
                            Some("System Automation"),
                            Some(&used_model),
                            Some(engine_used),
                        );
                        if engine_used == "openrouter" {
                            let price_per_min = if used_model.contains("scribe") { 0.00092 } else { 0.006 };
                            state.openrouter.deduct_credits_local(duration_seconds, price_per_min);
                        }
                        emit_to_frontend(&app, "usageUpdated", None);
                        emit_to_frontend(&app, "transcriptionComplete", Some(json!({
                            "text": format!("[Quick Action] {}", description),
                            "duration": duration_seconds,
                            "words": 0,
                            "model": &used_model,
                            "engine": engine_used,
                        })));
                        return;
                    }
                }

                // Smart Clipboard History Voice Recall (Fast-Path: Skip during Tenglish Mode)
                if !intent_router_handled && settings.clipboard_history_enabled && !settings.tenglish_mode_enabled {
                    if let Some(intent) = crate::ai::clipboard_recall::detect_clipboard_recall_intent(&final_text) {
                        stop_dynamic_focus_tracker();
                        let matched_clip: Option<crate::storage::clipboard::ClipboardEntry> = match intent {
                            crate::ai::clipboard_recall::ClipboardRecallIntent::RelativeIndex(idx) => {
                                state.clipboard.get_clip_by_relative_index(idx).ok().flatten()
                            }
                            crate::ai::clipboard_recall::ClipboardRecallIntent::Category(ref cat) => {
                                state.clipboard.get_recent_clips(1, Some(cat), None).ok().and_then(|mut v| {
                                    if !v.is_empty() {
                                        Some(v.remove(0))
                                    } else {
                                        None
                                    }
                                })
                            }
                            crate::ai::clipboard_recall::ClipboardRecallIntent::SemanticDescription(ref desc) => {
                                if let Ok(recent) = state.clipboard.get_recent_clips(15, None, None) {
                                    let matched_id = crate::ai::clipboard_recall::query_groq_clipboard_match(desc, &recent).await.ok().flatten();
                                    matched_id.and_then(|id| recent.into_iter().find(|c| c.id == id))
                                } else {
                                    None
                                }
                            }
                        };

                        if let Some(clip) = matched_clip {
                            state.is_internal_pasting.store(true, std::sync::atomic::Ordering::SeqCst);
                            let clip_hwnd = TextInjector::target_from_raw(target_raw.map(|v| v as usize));
                            let _ = TextInjector::paste_text(&clip.content, clip_hwnd);
                            let pasting_flag = state.is_internal_pasting.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(300));
                                pasting_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                            });

                            if settings.recording_sounds {
                                crate::audio::cue::play_success(settings.sound_volume);
                            }

                            let preview_clean = clip.preview.chars().take(60).collect::<String>();
                            emit_to_frontend(&app, "toast", Some(json!({
                                "message": format!("📋 Pasted clip #{}: \"{}\"", clip.id, preview_clean),
                                "type": "info"
                            })));

                            let _ = state.history.insert_entry(
                                &format!("[Clipboard Recall #{}] {}", clip.id, preview_clean),
                                duration_seconds,
                                Some("Clipboard Recall"),
                                Some(&used_model),
                                Some(engine_used),
                            );

                            if engine_used == "openrouter" {
                                let price_per_min = if used_model.contains("scribe") { 0.00092 } else { 0.006 };
                                state.openrouter.deduct_credits_local(duration_seconds, price_per_min);
                            }
                            emit_to_frontend(&app, "usageUpdated", None);
                            emit_to_frontend(&app, "transcriptionComplete", Some(json!({
                                "text": format!("[Clipboard Recall #{}] {}", clip.id, preview_clean),
                                "duration": duration_seconds,
                                "words": clip.content.split_whitespace().count(),
                                "model": &used_model,
                                "engine": engine_used,
                            })));
                            return;
                        } else {
                            emit_to_frontend(&app, "toast", Some(json!({
                                "message": "⚠️ No matching clipboard item found",
                                "type": "warning"
                            })));
                            if settings.recording_sounds {
                                crate::audio::cue::play_cancel(settings.sound_volume);
                            }
                            emit_to_frontend(&app, "recordingCancelled", None);
                            return;
                        }
                    }
                }

                let target_hwnd = TextInjector::target_from_raw(target_raw.map(|v| v as usize));

                // Feature 2: Inline Voice Editing & Formatting Commands (Fast-Path: Skip during Tenglish Mode or Intent Router)
                if !intent_router_handled && settings.voice_commands_enabled && !settings.tenglish_mode_enabled {
                    match crate::text::commands::evaluate_voice_command(&final_text) {
                        crate::text::commands::VoiceCommandAction::Undo => {
                            stop_dynamic_focus_tracker();
                            TextInjector::simulate_undo(target_hwnd);
                            if settings.recording_sounds {
                                crate::audio::cue::play_cancel(settings.sound_volume);
                            }
                            emit_to_frontend(&app, "toast", Some(json!({ "message": "Voice Command: Undo (Scratch That)", "type": "info" })));
                            emit_to_frontend(&app, "recordingCancelled", None);
                            return;
                        }
                        crate::text::commands::VoiceCommandAction::FormattedText(formatted) => {
                            final_text = formatted;
                        }
                        crate::text::commands::VoiceCommandAction::None => {}
                    }
                }

                // Feature 1: Smart Voice Snippets & Macro Expansions
                if !intent_router_handled && settings.voice_snippets_enabled {
                    if let Ok(snippets) = state.snippets.list_snippets() {
                        if let Some(expanded) = crate::text::snippets::match_and_expand(&final_text, &snippets) {
                            final_text = expanded;
                        }
                    }
                }

                // Stop the dynamic focus tracker now that we have captured the target window and are injecting
                stop_dynamic_focus_tracker();

                // Paste into target window, type directly, or copy
                // Guard with is_internal_pasting so Rift's clipboard listener does not race
                // with the target application during the paste and 350ms clipboard restore cycle.
                state.is_internal_pasting.store(true, std::sync::atomic::Ordering::SeqCst);
                let actual_hwnd = if !settings.copy_instead_of_type {
                    let (success, used_hwnd) = TextInjector::paste_text(&final_text, target_hwnd);
                    if success {
                        emit_to_frontend(&app, "textInjected", Some(json!({ "method": "pasted" })));
                        used_hwnd
                    } else {
                        let (typed, type_hwnd) = TextInjector::type_unicode_text(&final_text, target_hwnd);
                        if typed {
                            emit_to_frontend(&app, "textInjected", Some(json!({ "method": "typed" })));
                            type_hwnd.or(used_hwnd)
                        } else {
                            TextInjector::set_clipboard_text(&final_text);
                            emit_to_frontend(&app, "textInjected", Some(json!({ "method": "copied" })));
                            target_hwnd
                        }
                    }
                } else {
                    TextInjector::set_clipboard_text(&final_text);
                    emit_to_frontend(&app, "textInjected", Some(json!({ "method": "copied" })));
                    target_hwnd
                };

                let paste_flag = state.is_internal_pasting.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    paste_flag.store(false, std::sync::atomic::Ordering::SeqCst);
                });

                if settings.recording_sounds {
                    crate::audio::cue::play_success(settings.sound_volume);
                }


                let target_app_title = actual_hwnd.or(target_hwnd).and_then(TextInjector::get_window_title);
                let words_count = final_text.split_whitespace().count();

                // 1. Emit success and completion IMMEDIATELY for zero perceived UI latency
                emit_to_frontend(
                    &app,
                    "transcriptionSuccess",
                    Some(json!({
                        "text": &final_text,
                        "duration": duration_seconds,
                        "words": words_count,
                        "model": &used_model,
                        "engine": engine_used,
                        "targetApp": &target_app_title,
                    })),
                );
                emit_to_frontend(
                    &app,
                    "transcriptionComplete",
                    Some(json!({
                        "text": &final_text,
                        "duration": duration_seconds,
                        "words": words_count,
                        "model": &used_model,
                        "engine": engine_used,
                        "targetApp": &target_app_title,
                    })),
                );

                let my_pid = std::process::id();
                let is_rift_window = TextInjector::get_foreground_window_pid_and_raw()
                    .map(|(pid, _)| pid == my_pid)
                    .unwrap_or(false);
                if is_rift_window {
                    let preview: String = final_text.chars().take(60).collect();
                    emit_to_frontend(
                        &app,
                        "toast",
                        Some(json!({
                            "message": format!("📋 Copied to clipboard: \"{}\"", preview),
                            "type": "info"
                        })),
                    );
                }

                // 2. Offload SQLite history insertion, quota tracking, and credit deductions to background thread
                let state_bg = Arc::clone(&state);
                let app_bg = app.clone();
                let text_for_db = final_text.clone();
                let model_for_db = used_model.clone();
                let target_for_db = target_app_title.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = state_bg.history.insert_entry(
                        &text_for_db,
                        duration_seconds,
                        target_for_db.as_deref(),
                        Some(&model_for_db),
                        Some(engine_used),
                    );

                    if engine_used == "openrouter" {
                        let price_per_min = if model_for_db.contains("scribe") { 0.00092 } else { 0.006 };
                        state_bg.openrouter.deduct_credits_local(duration_seconds, price_per_min);
                        if let Some(ref okey) = CredentialsProvider::get_openrouter_api_key() {
                            let _ = state_bg.openrouter.fetch_credits(okey).await;
                        }
                    }

                    state_bg.whisper_requests_today.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let stats = get_usage_stats_val(&state_bg);
                    emit_to_frontend(&app_bg, "usageUpdated", Some(stats));
                });
            }
            Err(err) => {
                emit_to_frontend(
                    &app,
                    "transcriptionError",
                    Some(json!({ "message": err })),
                );
            }
        }
    });
}

// -------------------------------------------------------------
// Application Lifecycle & Single Instance
// -------------------------------------------------------------
#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn CreateMutexW(lpMutexAttributes: *const std::ffi::c_void, bInitialOwner: i32, lpName: *const u16) -> isize;
    fn CreateEventW(lpEventAttributes: *const std::ffi::c_void, bManualReset: i32, bInitialState: i32, lpName: *const u16) -> isize;
    fn OpenEventW(dwDesiredAccess: u32, bInheritHandle: i32, lpName: *const u16) -> isize;
    fn SetEvent(hEvent: isize) -> i32;
    fn WaitForSingleObject(hHandle: isize, dwMilliseconds: u32) -> u32;
    fn CloseHandle(hObject: isize) -> i32;
    fn GetLastError() -> u32;
}

#[cfg(windows)]
const ERROR_ALREADY_EXISTS: u32 = 183;
#[cfg(windows)]
const EVENT_MODIFY_STATE: u32 = 0x0002;
#[cfg(windows)]
const _SYNCHRONIZE: u32 = 0x00100000;
#[cfg(windows)]
const INFINITE: u32 = 0xFFFFFFFF;

pub fn show_and_focus_window(window: &tauri::WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();

    #[cfg(windows)]
    if let Ok(hwnd_ptr) = window.hwnd() {
        let hwnd = windows::Win32::Foundation::HWND(hwnd_ptr.0 as *mut _);
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                ShowWindow, SetForegroundWindow, BringWindowToTop, SetWindowPos,
                HWND_TOPMOST, HWND_NOTOPMOST, SW_RESTORE, SW_SHOW, SWP_SHOWWINDOW, SWP_NOSIZE, SWP_NOMOVE,
                GetForegroundWindow, GetWindowThreadProcessId,
            };
            use windows::Win32::System::Threading::AttachThreadInput;

            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = ShowWindow(hwnd, SW_SHOW);

            // Bypass Windows foreground restrictions by toggling HWND_TOPMOST
            let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            if window.label() != "widget" && window.label() != "scratchpad" {
                let _ = SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            }

            let win_thread = GetWindowThreadProcessId(hwnd, None);
            let fg = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg, None);

            if fg_thread != 0 && fg_thread != win_thread {
                let _ = AttachThreadInput(win_thread, fg_thread, true);
                let _ = BringWindowToTop(hwnd);
                let _ = SetForegroundWindow(hwnd);
                let _ = AttachThreadInput(win_thread, fg_thread, false);
            } else {
                let _ = BringWindowToTop(hwnd);
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }
}

#[cfg(windows)]
fn kill_stale_rift_instances() {
    let current_pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS
    };
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    unsafe {
        if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    if entry.th32ProcessID != current_pid && entry.th32ProcessID != 0 {
                        let name = String::from_utf16_lossy(&entry.szExeFile);
                        let clean_name = name.trim_matches('\0').to_lowercase();
                        if clean_name == "rift.exe" || clean_name == "rift-rust.exe" {
                            if let Ok(h_proc) = OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID) {
                                let _ = TerminateProcess(h_proc, 1);
                                let _ = windows::Win32::Foundation::CloseHandle(h_proc);
                            }
                        }
                    }
                    if Process32NextW(snap, &mut entry).is_err() {
                        break;
                    }
                }
            }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
        }
    }
}

#[cfg(not(windows))]
fn kill_stale_rift_instances() {}

pub(crate) fn log_status(msg: &str) {
    let dir = crate::storage::settings::get_app_data_dir();
    let path = dir.join("app.log");
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] [PID:{}] {}\n", ts, std::process::id(), msg);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

fn main() {
    log_status("=== Rift instance launched ===");
    #[cfg(windows)]
    let (_primary_mutex, show_event, ack_event) = {
        // 0. Single-Instance Check & Activation: Prevent multiple tray icons, reliably restore main window
        let mutex_name: Vec<u16> = "Local\\Rift_SingleInstance_Mutex\0".encode_utf16().collect();
        let mut mutex = unsafe { CreateMutexW(std::ptr::null(), 1, mutex_name.as_ptr()) };
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            log_status("SingleInstance mutex already exists - checking for running instances");
            // 1. Check if another Rift process is ACTUALLY alive before attempting handoff or exit
            let mut another_proc_exists = false;
            unsafe {
                use windows::Win32::System::Diagnostics::ToolHelp::{
                    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
                };
                let my_pid = std::process::id();
                if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                    let mut entry = PROCESSENTRY32W::default();
                    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
                    if Process32FirstW(snap, &mut entry).is_ok() {
                        loop {
                            if entry.th32ProcessID != my_pid {
                                let name = String::from_utf16_lossy(&entry.szExeFile);
                                let clean_name = name.trim_matches('\0').to_lowercase();
                                if clean_name == "rift.exe" || clean_name == "rift-rust.exe" {
                                    another_proc_exists = true;
                                    break;
                                }
                            }
                            if Process32NextW(snap, &mut entry).is_err() {
                                break;
                            }
                        }
                    }
                    let _ = windows::Win32::Foundation::CloseHandle(snap);
                }
            }

            let mut handoff_successful = false;
            if another_proc_exists {
                log_status("Another Rift process is alive - attempting signaling handoff");
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;
                    const ASFW_ANY: u32 = 0xFFFFFFFF;
                    let _ = AllowSetForegroundWindow(ASFW_ANY);
                }

                let event_name: Vec<u16> = "Local\\Rift_ShowMain_Event\0".encode_utf16().collect();
                let ack_event_name: Vec<u16> = "Local\\Rift_ShowMain_Ack_Event\0".encode_utf16().collect();

                // Create or open ACK event (auto-reset, initially unsignaled)
                let h_ack = unsafe { CreateEventW(std::ptr::null(), 0, 0, ack_event_name.as_ptr()) };

                for _ in 0..6 {
                    let h_event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, event_name.as_ptr()) };
                    if h_event != 0 {
                        unsafe {
                            SetEvent(h_event);
                            CloseHandle(h_event);
                        }
                        log_status("Signaled primary instance via show_event - waiting for ACK");
                        if h_ack != 0 {
                            let wait_res = unsafe { WaitForSingleObject(h_ack, 800) };
                            if wait_res == 0 {
                                log_status("Primary instance acknowledged show request! Handoff complete.");
                                handoff_successful = true;
                            } else {
                                log_status(&format!("Primary instance did not acknowledge within 800ms (res={})", wait_res));
                            }
                        } else {
                            handoff_successful = true;
                        }
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }

                if h_ack != 0 {
                    unsafe { CloseHandle(h_ack); }
                }

                if handoff_successful {
                    log_status("Exiting secondary instance cleanly after successful handoff");
                    std::process::exit(0);
                }
                log_status("Primary instance unresponsive, headless, or failed to acknowledge - terminating stale instance and taking over");
            } else {
                log_status("No other active Rift process found - taking over stale mutex and continuing");
            }

            unsafe {
                if mutex != 0 {
                    CloseHandle(mutex);
                }
            }
            kill_stale_rift_instances();
            std::thread::sleep(std::time::Duration::from_millis(150));
            mutex = unsafe { CreateMutexW(std::ptr::null(), 1, mutex_name.as_ptr()) };
            log_status("Recovered and acquired primary mutex after killing stale instances");
        } else {
            log_status("Primary instance mutex acquired");
        }

        let event_name: Vec<u16> = "Local\\Rift_ShowMain_Event\0".encode_utf16().collect();
        let show_event = unsafe { CreateEventW(std::ptr::null(), 0, 0, event_name.as_ptr()) };
        let ack_event_name: Vec<u16> = "Local\\Rift_ShowMain_Ack_Event\0".encode_utf16().collect();
        let ack_event = unsafe { CreateEventW(std::ptr::null(), 0, 0, ack_event_name.as_ptr()) };
        (mutex, show_event, ack_event)
    };

    #[cfg(not(windows))]
    let (_primary_mutex, show_event, ack_event): (isize, isize, isize) = (0, 0, 0);

    let log_path = std::env::current_exe()
        .map(|p| p.parent().unwrap_or(&p).join("rift.log"))
        .unwrap_or_else(|_| std::path::PathBuf::from("rift.log"));
    let log_clone = log_path.clone();
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let msg = format!("PANIC: {:?}\nBacktrace:\n{}\n", info, bt);
        log_status(&msg);
        let _ = std::fs::write(&log_clone, &msg);
    }));

    log_status("Initializing AppState");
    let app_state = match AppState::new() {
        Ok(s) => {
            log_status("AppState created successfully");
            Arc::new(s)
        }
        Err(e) => {
            log_status(&format!("Failed to initialize application state: {}", e));
            let _ = std::fs::write(&log_path, format!("Failed to initialize application state: {}\n", e));
            return;
        }
    };

    let ambient_tracker_state = Arc::clone(&app_state);

    // Pre-fetch OpenRouter credits on startup if key is configured
    if let Some(or_key) = CredentialsProvider::get_openrouter_api_key() {
        if !or_key.trim().is_empty() {
            let or_client = app_state.openrouter.clone();
            tauri::async_runtime::spawn(async move {
                let _ = or_client.fetch_credits(&or_key).await;
            });
        }
    }

    let app = tauri::Builder::default()
        .manage(Arc::clone(&app_state))
        .invoke_handler(tauri::generate_handler![
            startDragging,
            startWidgetDrag,
            startScratchpadDragging,
            getWidgetConfig,
            setWidgetVisibility,
            setFollowCursor,
            setLockPosition,
            persistWidgetPosition,
            moveWidgetBy,
            saveCurrentWidgetPosition,
            resetWidgetPosition,
            triggerWidgetTestState,
            getSettings,
            saveSettings,
            toggleTenglishMode,
            getHotkeys,
            saveHotkey,
            clearHotkey,
            startHotkeyCapture,
            cancelHotkeyCapture,
            verifyHotkey,
            getMicrophones,
            listMicrophones,
            setMicrophone,
            getGroqKeyStatus,
            testGroqKey,
            testSavedGroqKey,
            saveGroqKey,
            deleteGroqKey,
            saveApiKey,
            getOpenRouterKeyStatus,
            testOpenRouterKey,
            testSavedOpenRouterKey,
            saveOpenRouterKey,
            deleteOpenRouterKey,
            saveOpenRouterApiKey,
            getHistory,
            searchHistory,
            searchLibrary,
            getLibraryItemContent,
            getScratchpadDraft,
            saveScratchpadDraft,
            deleteHistoryEntry,
            clearHistory,
            getUsageStats,
            getDashboardStats,
            getModelRemainingUsage,
            getDictionary,
            addDictionaryEntry,
            updateDictionaryEntry,
            deleteDictionaryEntry,
            importDictionary,
            exportDictionary,
            testDictionary,
            getLocalModelsStatus,
            downloadLocalModel,
            deleteLocalModel,
            verifyLocalModel,
            getAvailableCloudModels,
            getTranscriptionMode,
            setTranscriptionMode,
            setLocalInferenceProfile,
            testEnhance,
            testCleanup,
            copyToClipboard,
            checkForUpdates,
            isLaunchAtLoginActive,
            openSettingsWindow,
            openOnboardingWindow,
            closeSettingsWindow,
            quitApplication,
            cancelRecording,
            startRecording,
            stopRecording,
            toggleRecording,
            toggleAmbientMemory,
            setWidgetDropdownOpen,
            openScratchpadWindow,
            closeScratchpadWindow,
            minimizeScratchpadWindow,
            toggleScratchpadPin,
            insertScratchpadText,
            drainScratchpadInbox,
            polishSelectedText,
            testAudioCue,
            getSnippets,
            saveSnippet,
            deleteSnippet,
            openSpotlightWindow,
            closeSpotlightWindow,
            askSpotlightAi,
            insertSpotlightText,
            getClipboardHistory,
            toggleClipPin,
            deleteClip,
            clearUnpinnedClips,
            copyClipById,
            pasteClipById,
            getClipboardCounts,
            retryLastTranscription,
            hasSavedRecording,
        ])

        .setup(move |app| {
            log_status("Inside setup() hook - configuring windows and listeners");
            // 0a. Ambient Focus Tracker: continuously tracks active external application window
            let tracker_state = ambient_tracker_state.clone();
            std::thread::spawn(move || {
                let my_pid = std::process::id();
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(120));
                    if let Some((fg_pid, raw_val)) = TextInjector::get_foreground_window_pid_and_raw() {
                        if fg_pid != 0 && fg_pid != my_pid {
                            let mut target = tracker_state.target_window.lock(); {
                                *target = Some(raw_val);
                            }
                        }
                    }
                }
            });

            // 0b. Spawn listener for subsequent shortcut launches
            #[cfg(windows)]
            if show_event != 0 {
                let app_for_event = app.handle().clone();
                std::thread::spawn(move || {
                    log_status("show_event listener thread started");
                    while show_event != 0 {
                        let res = unsafe { WaitForSingleObject(show_event, INFINITE) };
                        log_status(&format!("show_event WaitForSingleObject returned {}", res));
                        if res == 0 {
                            let app_inner = app_for_event.clone();
                            let _ = app_for_event.run_on_main_thread(move || {
                                log_status("show_event: run_on_main_thread executing - restoring windows");
                                // 1. Restore widget window
                                if let Some(widget_win) = app_inner.get_webview_window("widget") {
                                    log_status("show_event: widget window found, restoring and ensuring top");
                                    let _ = widget_win.unminimize();
                                    let _ = widget_win.show();
                                    let _ = widget_win.set_always_on_top(true);
                                }
                                // 2. Restore main window
                                if let Some(main_win) = app_inner.get_webview_window("main") {
                                    log_status("show_event: main window found, restoring and focusing");
                                    show_and_focus_window(&main_win);
                                } else {
                                    log_status("show_event: WARNING: main window not found on app_inner!");
                                }
                                // 3. Acknowledge to secondary instance
                                if ack_event != 0 {
                                    unsafe { SetEvent(ack_event); }
                                    log_status("show_event: signaled ack_event");
                                }
                            });
                        } else {
                            log_status(&format!("show_event: WaitForSingleObject failed with code {}, exiting thread", res));
                            break;
                        }
                    }
                });
            }

            // 1. Setup floating widget window
            if let Some(widget_win) = app.get_webview_window("widget") {
                window::widget_window::setup_widget_window(&widget_win);

                let settings = load_settings();

                if let Ok(Some(monitor)) = widget_win.primary_monitor() {
                    let size = monitor.size();
                    let scale = monitor.scale_factor();
                    let widget_phys_w = (190.0 * scale) as i32;
                    let widget_phys_h = (60.0 * scale) as i32;
                    let def_x = ((size.width as i32) - widget_phys_w) / 2;
                    let def_y = (size.height as i32) - widget_phys_h - (40.0 * scale) as i32;

                    let final_pos = if let Some(custom) = &settings.widget_custom_position {
                        let max_x = ((size.width as i32) - widget_phys_w).max(0);
                        let max_y = ((size.height as i32) - widget_phys_h).max(0);
                        let clamped_x = custom.x.clamp(0, max_x);
                        let clamped_y = custom.y.clamp(0, max_y);
                        tauri::PhysicalPosition { x: clamped_x, y: clamped_y }
                    } else {
                        tauri::PhysicalPosition { x: def_x, y: def_y }
                    };

                    let _ = widget_win.set_position(tauri::Position::Physical(final_pos));
                }

                // Explicitly show widget and keep on top
                let _ = widget_win.show();
                let _ = widget_win.set_always_on_top(true);

                let is_widget_ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let is_widget_ready_flag = is_widget_ready.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                    is_widget_ready_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                });

                let widget_clone = widget_win.clone();
                widget_win.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            let _ = widget_clone.hide();
                        }
                        tauri::WindowEvent::Moved(pos) => {
                            if is_widget_ready.load(std::sync::atomic::Ordering::SeqCst) {
                                let mut s = load_settings();
                                if !s.lock_widget_position {
                                    s.widget_custom_position = Some(WidgetPosition { x: pos.x, y: pos.y });
                                    save_settings(&s);
                                }
                            }
                        }
                        _ => {}
                    }
                });
            }

            // 2. Setup main window: show on initial launch, and hide to tray on close
            if let Some(main_win) = app.get_webview_window("main") {
                let _ = main_win.center();
                let settings = load_settings();
                let has_api_key = CredentialsProvider::get_api_key().is_some() || CredentialsProvider::get_openrouter_api_key().is_some();
                if !settings.onboarding_completed || !has_api_key {
                    let _ = main_win.eval("window.location.href = 'screens/onboarding-welcome.html';");
                }

                show_and_focus_window(&main_win);

                let main_clone = main_win.clone();
                main_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = main_clone.hide();
                        crate::window::auxiliary::trim_application_working_set_async();
                    }
                });
            }

            // 3. Setup Single System Tray with App Icon
            let spotlight_item = MenuItem::with_id(app, "spotlight", "Ask Rift (Groq AI)", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let history_item = MenuItem::with_id(app, "history", "History", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&spotlight_item, &settings_item, &history_item, &quit_item])?;

            let mut tray_builder = TrayIconBuilder::with_id("rift-tray")
                .menu(&menu)
                .tooltip("Rift Voice Dictation")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "spotlight" => {
                        openSpotlightWindow(app.clone());
                    }
                    "settings" | "history" => {
                        if let Some(window) = app.get_webview_window("main") {
                            use tauri::Emitter;
                            let destination = if event.id.as_ref() == "settings" {
                                "settings"
                            } else {
                                "history"
                            };
                            let _ = app.emit("navigateTo", destination);
                            show_and_focus_window(&window);
                        }
                    }
                    "quit" => {
                        stt::local::stop_resident_server();
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            show_and_focus_window(&window);
                        }
                    }
                });

            let tray_icon = app.default_window_icon()
                .cloned()
                .or_else(|| tauri::image::Image::from_bytes(include_bytes!("../icons/icon.ico")).ok())
                .or_else(|| tauri::image::Image::from_bytes(include_bytes!("../icons/tray_icon.png")).ok())
                .or_else(|| tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png")).ok());

            if let Some(icon) = tray_icon {
                tray_builder = tray_builder.icon(icon);
            }

            let tray = tray_builder.build(app)?;
            log_status("System tray icon registered successfully and permanently");
            // CRITICAL: In Tauri 2.x, TrayIcon's Drop implementation removes the tray icon from Windows!
            // Leaking/forgetting this handle ensures the tray icon stays alive for the entire process lifetime.
            std::mem::forget(tray);

            let settings = load_settings();
            crate::audio::cue::prewarm(settings.sound_volume);

            // 3b. Setup Smart Clipboard History Listener (Win32 Event-Driven)
            if settings.clipboard_history_enabled {
                if let Err(error) = crate::ambient::clipboard_listener::start_clipboard_listener(
                    app_state.clipboard.clone(),
                    app_state.is_internal_pasting.clone(),
                    app.handle().clone(),
                    settings.clipboard_max_items,
                ) {
                    log::error!("Failed to start clipboard history listener: {error}");
                }
            }

            // 3c. Start Ambient Memory if enabled
            if settings.ambient_memory_enabled {
                let mut recorder = app_state.ambient_recorder.lock(); {
                    let _ = recorder.start();
                    log_status("Ambient Memory started from settings on launch.");
                }
            }

            // 3d. Start Hands-Free "Hey Rift" Wake Word Engine if enabled
            if settings.wake_word_enabled {
                sync_wake_word_engine(app.handle(), &app_state, false);
            }

            // 4. Setup Hotkey Listener (Push-to-Talk + Toggle Recording)
            let app_handle = app.handle().clone();
            let state_for_press = Arc::clone(&app_state);
            let state_for_release = Arc::clone(&app_state);
            let state_for_toggle = Arc::clone(&app_state);
            let app_handle_for_press = app_handle.clone();
            let app_handle_for_release = app_handle.clone();
            let app_handle_for_toggle = app_handle.clone();
            let ptt_key = if settings.push_to_talk_mode == "combo" {
                settings.push_to_talk_combo.clone()
            } else {
                settings.push_to_talk_key.clone()
            };
            let toggle_key = settings.toggle_recording_key.clone();

            app_state.hotkey.start(
                &ptt_key,
                toggle_key.as_deref(),
                Some("Ctrl+Shift+Space"),
                // On PTT Press
                move || {
                    log_status("PTT Key Pressed -> starting recording and emitting recordingStarted");

                    let _transition = state_for_press.recording_transition.lock();
                    if state_for_press.is_toggle_recording.compare_exchange(
                        false,
                        true,
                        std::sync::atomic::Ordering::SeqCst,
                        std::sync::atomic::Ordering::SeqCst,
                    ).is_err() {
                        return;
                    }

                    let s = load_settings();
                    if s.recording_sounds {
                        crate::audio::cue::play_start(s.sound_volume);
                    }
                    crate::audio::mute::mute_system_audio_delayed(180);

                    let target_hwnd = TextInjector::get_foreground_window();
                    {
                        let mut target = state_for_press.target_window.lock();
                        *target = Some(target_hwnd.0 as isize);
                    }
                    let app_for_level = app_handle_for_press.clone();
                    let last_level = Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
                    let boost = s.mic_boost_multiplier();
                    let dev = s.microphone_device_id;
                    let on_stop = recording_stop_handler(
                        app_handle_for_press.clone(),
                        Arc::clone(&state_for_press),
                    );
                    let start_result = state_for_press.recorder.lock().start(dev.as_deref(), boost, move |level| {
                        let now = std::time::Instant::now();
                        let should_emit = if let Ok(mut l) = last_level.try_lock() {
                            if now.duration_since(*l).as_millis() >= 40 {
                                *l = now;
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        if should_emit {
                            emit_to_frontend(&app_for_level, "audioLevel", Some(json!(level)));
                        }
                    }, on_stop);

                    if let Err(error) = start_result {
                        state_for_press.is_toggle_recording.store(false, std::sync::atomic::Ordering::SeqCst);
                        crate::audio::mute::unmute_system_audio();
                        emit_to_frontend(&app_handle_for_press, "recordingError", Some(json!({ "message": error })));
                        return;
                    }

                    start_dynamic_focus_tracker(Arc::clone(&state_for_press));
                    emit_to_frontend(&app_handle_for_press, "recordingStarted", None);
                    if let Some(key) = CredentialsProvider::get_openrouter_api_key() {
                        state_for_press.openrouter.prewarm(&key);
                    }
                    if let Some(key) = CredentialsProvider::get_api_key() {
                        state_for_press.groq.prewarm(&key);
                    }
                },
                // On PTT Release
                move || {
                    log_status("PTT Key Released -> stopping recorder");
                    let state = Arc::clone(&state_for_release);
                    let app = app_handle_for_release.clone();
                    std::thread::spawn(move || {
                        stop_recording_inner(app, state);
                    });
                },
                // On Toggle Recording Press
                move || {
                    let is_recording = state_for_toggle.is_toggle_recording.load(std::sync::atomic::Ordering::SeqCst);
                    log_status(&format!("Toggle Key Pressed -> current is_recording={}", is_recording));
                    if !is_recording {
                        let _transition = state_for_toggle.recording_transition.lock();
                        if state_for_toggle.is_toggle_recording.compare_exchange(
                            false,
                            true,
                            std::sync::atomic::Ordering::SeqCst,
                            std::sync::atomic::Ordering::SeqCst,
                        ).is_err() {
                            return;
                        }
                        // Start recording
                        let s = load_settings();
                        if s.recording_sounds {
                            crate::audio::cue::play_start(s.sound_volume);
                        }
                        crate::audio::mute::mute_system_audio_delayed(180);

                        let target_hwnd = TextInjector::get_foreground_window();
                        {
                            let mut target = state_for_toggle.target_window.lock();
                            *target = Some(target_hwnd.0 as isize);
                        }
                        let app_for_level = app_handle_for_toggle.clone();
                        let last_level = Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
                        let boost = s.mic_boost_multiplier();
                        let dev = s.microphone_device_id;
                        let on_stop = recording_stop_handler(
                            app_handle_for_toggle.clone(),
                            Arc::clone(&state_for_toggle),
                        );
                        let mut recorder = state_for_toggle.recorder.lock();
                        let start_result = recorder.start(dev.as_deref(), boost, move |level| {
                            let now = std::time::Instant::now();
                            let should_emit = if let Ok(mut l) = last_level.try_lock() {
                                if now.duration_since(*l).as_millis() >= 40 {
                                    *l = now;
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            };
                            if should_emit {
                                emit_to_frontend(&app_for_level, "audioLevel", Some(json!(level)));
                            }
                        }, on_stop);
                        drop(recorder);
                        if let Err(error) = start_result {
                            state_for_toggle.is_toggle_recording.store(false, std::sync::atomic::Ordering::SeqCst);
                            crate::audio::mute::unmute_system_audio();
                            emit_to_frontend(&app_handle_for_toggle, "recordingError", Some(json!({ "message": error })));
                            return;
                        }
                        start_dynamic_focus_tracker(Arc::clone(&state_for_toggle));
                        emit_to_frontend(&app_handle_for_toggle, "recordingStarted", None);
                        if let Some(key) = CredentialsProvider::get_openrouter_api_key() {
                            state_for_toggle.openrouter.prewarm(&key);
                        }
                        if let Some(key) = CredentialsProvider::get_api_key() {
                            state_for_toggle.groq.prewarm(&key);
                        }
                    } else {
                        stop_recording_inner(
                            app_handle_for_toggle.clone(),
                            Arc::clone(&state_for_toggle),
                        );
                    }
                },

                // On Show Widget Hotkey (Ctrl+Shift+Space)
                {
                    let app_handle_for_widget = app_handle.clone();
                    move || {
                        if let Some(widget_win) = app_handle_for_widget.get_webview_window("widget") {
                            let _ = widget_win.unminimize();
                            let _ = widget_win.show();
                            let _ = widget_win.set_always_on_top(true);
                            emit_to_frontend(&app_handle_for_widget, "widgetBroughtToFront", None);
                        }
                    }
                },
                // On Ambient Memory Trigger (Ctrl+Shift+M)
                {
                    let app_handle_for_ambient = app_handle.clone();
                    let state_for_ambient = Arc::clone(&app_state);
                    move || {
                        log_status("Ambient Memory Triggered!");
                        let wav_bytes = {
                            let recorder = state_for_ambient.ambient_recorder.lock();
                            recorder.extract_wav_bytes()
                        };
                        
                        if !wav_bytes.is_empty() {
                            emit_to_frontend(&app_handle_for_ambient, "ambientMemoryStarted", None);
                            // Process in background
                            let app_clone = app_handle_for_ambient.clone();
                            let state_clone = Arc::clone(&state_for_ambient);
                            tauri::async_runtime::spawn(async move {
                                process_ambient_memory(app_clone, state_clone, wav_bytes).await;
                            });
                        }
                    }
                },
            );

            log_status("setup() hook completed successfully");
            Ok(())
        });

    log_status("Building Tauri application context");
    let app = match app.build(tauri::generate_context!()) {
        Ok(a) => {
            log_status("Tauri application context built successfully");
            a
        }
        Err(e) => {
            log_status(&format!("BUILD ERROR: {:?}", e));
            let _ = std::fs::write("build.log", format!("BUILD ERROR: {:?}\n", e));
            return;
        }
    };

    log_status("Starting app.run event loop");
    app.run(|_app_handle, event| {
        match &event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                log_status("RunEvent::ExitRequested - preventing exit");
                api.prevent_exit();
            }
            tauri::RunEvent::Exit => {
                log_status("RunEvent::Exit received");
                stt::local::stop_resident_server();
            }
            tauri::RunEvent::Ready => {
                log_status("RunEvent::Ready received");
            }
            tauri::RunEvent::Resumed => {
                log_status("RunEvent::Resumed received");
            }
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            tauri::RunEvent::Opened { .. } => {
                log_status("RunEvent::Opened received - restoring main window");
                if let Some(main_win) = _app_handle.get_webview_window("main") {
                    show_and_focus_window(&main_win);
                }
            }
            tauri::RunEvent::WindowEvent { label, event, .. } => {
                match event {
                    tauri::WindowEvent::CloseRequested { .. } => {
                        log_status(&format!("WindowEvent::CloseRequested on window '{}'", label));
                    }
                    tauri::WindowEvent::Destroyed => {
                        log_status(&format!("WindowEvent::Destroyed on window '{}'", label));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    });
}
