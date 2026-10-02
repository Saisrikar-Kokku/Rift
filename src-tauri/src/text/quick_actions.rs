#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::ptr::{null, null_mut};
use std::thread;
use std::time::Duration;
use tauri::Manager;
#[cfg(windows)]
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, WPARAM};
#[cfg(windows)]
use windows::Win32::System::Threading::GetCurrentProcessId;
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindow, IsWindowVisible, PostMessageW, SetForegroundWindow,
    SetWindowPos, ShowWindow, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, WM_CLOSE,
};

#[cfg(windows)]
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut std::ffi::c_void,
        lpOperation: *const u16,
        lpFile: *const u16,
        lpParameters: *const u16,
        lpDirectory: *const u16,
        nShowCmd: i32,
    ) -> isize;
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn LockWorkStation() -> i32;
    fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: i32) -> i32;
}

// Windows Virtual Key Codes for Media & Snapshot
#[cfg(windows)]
const VK_SNAPSHOT: u8 = 0x2C;
#[cfg(windows)]
const VK_VOLUME_MUTE: u8 = 0xAD;
#[cfg(windows)]
const VK_VOLUME_DOWN: u8 = 0xAE;
#[cfg(windows)]
const VK_VOLUME_UP: u8 = 0xAF;

use crate::text::injector::WindowTarget;

/// Represents a recognized system or app action triggered via voice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickAction {
    OpenApp { spoken: String, executable: String, label: String },
    SwitchToApp { spoken: String },
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    LockPC,
    TakeScreenshot,
    MuteAudio,
    UnmuteAudio,
    VolumeUp,
    VolumeDown,
    RiftOpenSettings,
    RiftOpenScratchpad,
    RiftOpenSpotlight,
    None,
}

/// Convert a string slice to null-terminated UTF-16 wide vector for Win32 calls.
#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// Map spoken app names to their Windows executable or protocol target and display label.
pub fn resolve_app_executable(spoken_name: &str) -> Option<(&'static str, &'static str)> {
    let clean = spoken_name.trim().to_lowercase();
    match clean.as_str() {
        "chrome" | "google chrome" => Some(("chrome", "Google Chrome")),
        "firefox" | "mozilla" | "mozilla firefox" => Some(("firefox", "Firefox")),
        "edge" | "microsoft edge" | "ms edge" => Some(("msedge", "Microsoft Edge")),
        "brave" | "brave browser" => Some(("brave", "Brave Browser")),
        "vs code" | "visual studio code" | "code" | "vscode" => Some(("code", "VS Code")),
        "visual studio" | "devenv" => Some(("devenv", "Visual Studio")),
        "notepad" => Some(("notepad", "Notepad")),
        "calculator" | "calc" => Some(("calc", "Calculator")),
        "file explorer" | "explorer" | "files" | "my computer" => Some(("explorer", "File Explorer")),
        "terminal" | "command prompt" | "cmd" => Some(("cmd", "Command Prompt")),
        "windows terminal" | "wt" => Some(("wt", "Windows Terminal")),
        "powershell" => Some(("powershell", "PowerShell")),
        "slack" => Some(("slack", "Slack")),
        "discord" => Some(("discord", "Discord")),
        "spotify" => Some(("spotify", "Spotify")),
        "teams" | "microsoft teams" => Some(("ms-teams", "Microsoft Teams")),
        "outlook" => Some(("outlook", "Outlook")),
        "word" | "microsoft word" | "winword" => Some(("winword", "Microsoft Word")),
        "excel" | "microsoft excel" => Some(("excel", "Microsoft Excel")),
        "powerpoint" | "microsoft powerpoint" | "powerpnt" => Some(("powerpnt", "PowerPoint")),
        "paint" | "mspaint" => Some(("mspaint", "Paint")),
        "task manager" | "taskmgr" => Some(("taskmgr", "Task Manager")),
        "windows settings" => Some(("ms-settings:", "Windows Settings")),
        _ => None,
    }
}

/// Detect whether transcribed speech corresponds to a quick action.
pub fn detect_quick_action(text: &str) -> QuickAction {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();
    let stripped = lower
        .trim_end_matches(['.', ',', '!', '?', ';', ':', '-', ' '])
        .trim();

    // 1. System actions
    match stripped {
        "lock my pc"
        | "lock pc"
        | "lock computer"
        | "lock the computer"
        | "lock the pc"
        | "lock screen"
        | "lock workstation" => return QuickAction::LockPC,

        "take a screenshot"
        | "take screenshot"
        | "screenshot this"
        | "screen shot this"
        | "take screen shot"
        | "capture screen"
        | "capture screenshot" => return QuickAction::TakeScreenshot,

        "mute"
        | "mute audio"
        | "mute sound"
        | "mute volume" => return QuickAction::MuteAudio,

        "unmute"
        | "unmute audio"
        | "unmute sound"
        | "unmute volume" => return QuickAction::UnmuteAudio,

        "volume up"
        | "turn up volume"
        | "increase volume"
        | "raise volume"
        | "louder" => return QuickAction::VolumeUp,

        "volume down"
        | "turn down volume"
        | "decrease volume"
        | "lower volume"
        | "quieter" => return QuickAction::VolumeDown,

        "close this window"
        | "close window"
        | "close this"
        | "close current window" => return QuickAction::CloseWindow,

        "minimize this window"
        | "minimize this"
        | "minimize window"
        | "minimize current window" => return QuickAction::MinimizeWindow,

        "maximize this window"
        | "maximize this"
        | "maximize window"
        | "maximize current window" => return QuickAction::MaximizeWindow,

        // Rift Internal
        "open settings"
        | "open rift settings"
        | "show settings"
        | "show rift settings"
        | "rift settings" => return QuickAction::RiftOpenSettings,

        "open scratchpad"
        | "show scratchpad"
        | "open notes"
        | "show notes"
        | "rift scratchpad" => return QuickAction::RiftOpenScratchpad,

        "open spotlight"
        | "show spotlight"
        | "ask rift"
        | "open ai assistant"
        | "rift spotlight" => return QuickAction::RiftOpenSpotlight,

        _ => {}
    }

    // 2. Switch to app: "switch to [app]"
    if let Some(target) = stripped.strip_prefix("switch to ") {
        let app_query = target.trim();
        if !app_query.is_empty() {
            return QuickAction::SwitchToApp {
                spoken: app_query.to_string(),
            };
        }
    }

    // 3. Open/Launch app: "open [app]" or "launch [app]"
    let app_candidate = if let Some(target) = stripped.strip_prefix("open ") {
        Some(target.trim())
    } else if let Some(target) = stripped.strip_prefix("launch ") {
        Some(target.trim())
    } else if let Some(target) = stripped.strip_prefix("start ") {
        Some(target.trim())
    } else {
        None
    };

    if let Some(candidate) = app_candidate {
        if let Some((exe, label)) = resolve_app_executable(candidate) {
            return QuickAction::OpenApp {
                spoken: candidate.to_string(),
                executable: exe.to_string(),
                label: label.to_string(),
            };
        }
    }

    QuickAction::None
}

/// Context passed to EnumWindows to find a window by title.
#[cfg(windows)]
struct WindowSearchContext {
    queries: Vec<String>,
    found_hwnd: Option<HWND>,
    found_title: String,
}

#[cfg(windows)]
unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return BOOL(1);
    }

    let mut buf = vec![0u16; (len + 1) as usize];
    let actual_len = GetWindowTextW(hwnd, &mut buf);
    if actual_len > 0 {
        let title = String::from_utf16_lossy(&buf[..actual_len as usize]);
        let lower_title = title.to_lowercase();
        let ctx = &mut *(lparam.0 as *mut WindowSearchContext);

        for q in &ctx.queries {
            if lower_title.contains(q) {
                ctx.found_hwnd = Some(hwnd);
                ctx.found_title = title;
                return BOOL(0); // Stop enumeration
            }
        }
    }

    BOOL(1) // Continue
}

/// Bring an HWND to the foreground reliably on Windows.
#[cfg(windows)]
fn bring_hwnd_to_front(hwnd: HWND) -> bool {
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = ShowWindow(hwnd, SW_SHOW);

        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let _ = SetWindowPos(
            hwnd,
            HWND_NOTOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );

        let win_thread = GetWindowThreadProcessId(hwnd, None);
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);

        if fg_thread != 0 && fg_thread != win_thread {
            let _ = AttachThreadInput(win_thread, fg_thread, 1);
            let _ = BringWindowToTop(hwnd);
            let res = SetForegroundWindow(hwnd);
            let _ = AttachThreadInput(win_thread, fg_thread, 0);
            res.as_bool()
        } else {
            let _ = BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd).as_bool()
        }
    }
}

/// Execute a detected quick action and return a human-readable confirmation string.
#[cfg(windows)]
pub fn execute_quick_action(
    action: &QuickAction,
    app: Option<&tauri::AppHandle>,
    target_hwnd: Option<WindowTarget>,
) -> String {
    match action {
        QuickAction::OpenApp { executable, label, .. } => {
            let op = to_wide("open");
            let target = to_wide(executable);
            let ret = unsafe {
                ShellExecuteW(
                    null_mut(),
                    op.as_ptr(),
                    target.as_ptr(),
                    null(),
                    null(),
                    1, // SW_SHOWNORMAL
                )
            };
            if ret > 32 {
                format!("Opened {}", label)
            } else {
                format!("Failed to open {} (code {})", label, ret)
            }
        }

        QuickAction::SwitchToApp { spoken } => {
            // Build list of search query candidates
            let mut queries = vec![spoken.to_lowercase()];
            if let Some((_, label)) = resolve_app_executable(spoken) {
                queries.push(label.to_lowercase());
            }

            let mut ctx = WindowSearchContext {
                queries,
                found_hwnd: None,
                found_title: String::new(),
            };

            unsafe {
                let _ = EnumWindows(
                    Some(enum_windows_proc),
                    LPARAM(&mut ctx as *mut _ as isize),
                );
            }

            if let Some(hwnd) = ctx.found_hwnd {
                if bring_hwnd_to_front(hwnd) {
                    let short_title = if ctx.found_title.len() > 30 {
                        format!("{}...", &ctx.found_title[..27])
                    } else {
                        ctx.found_title
                    };
                    format!("Switched to {}", short_title)
                } else {
                    format!("Found window for '{}' but could not focus", spoken)
                }
            } else {
                // If not found in open windows, attempt to launch if recognized
                if let Some((exe, label)) = resolve_app_executable(spoken) {
                    let op = to_wide("open");
                    let target = to_wide(exe);
                    unsafe {
                        let _ = ShellExecuteW(
                            null_mut(),
                            op.as_ptr(),
                            target.as_ptr(),
                            null(),
                            null(),
                            1,
                        );
                    }
                    format!("{} was not open, launched it", label)
                } else {
                    format!("No open window found matching '{}'", spoken)
                }
            }
        }

        QuickAction::CloseWindow => {
            let candidate = target_hwnd.filter(|h| !h.0.is_null()).unwrap_or_else(|| unsafe { GetForegroundWindow() });
            if !candidate.0.is_null() && unsafe { IsWindow(candidate).as_bool() } {
                // Safety: Never close Rift's own processes
                let current_pid = unsafe { GetCurrentProcessId() };
                let mut target_pid = 0u32;
                unsafe { GetWindowThreadProcessId(candidate, Some(&mut target_pid)); }
                if target_pid == current_pid {
                    return "Ignored close for Rift window".to_string();
                }

                unsafe {
                    let _ = PostMessageW(candidate, WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                "Closed active window".to_string()
            } else {
                "No active window to close".to_string()
            }
        }

        QuickAction::MinimizeWindow => {
            let candidate = target_hwnd.filter(|h| !h.0.is_null()).unwrap_or_else(|| unsafe { GetForegroundWindow() });
            if !candidate.0.is_null() && unsafe { IsWindow(candidate).as_bool() } {
                let current_pid = unsafe { GetCurrentProcessId() };
                let mut target_pid = 0u32;
                unsafe { GetWindowThreadProcessId(candidate, Some(&mut target_pid)); }
                if target_pid == current_pid {
                    return "Ignored minimize for Rift window".to_string();
                }

                unsafe {
                    let _ = ShowWindow(candidate, SW_MINIMIZE);
                }
                "Minimized active window".to_string()
            } else {
                "No active window to minimize".to_string()
            }
        }

        QuickAction::MaximizeWindow => {
            let candidate = target_hwnd.filter(|h| !h.0.is_null()).unwrap_or_else(|| unsafe { GetForegroundWindow() });
            if !candidate.0.is_null() && unsafe { IsWindow(candidate).as_bool() } {
                unsafe {
                    let _ = ShowWindow(candidate, SW_MAXIMIZE);
                }
                "Maximized active window".to_string()
            } else {
                "No active window to maximize".to_string()
            }
        }

        QuickAction::LockPC => {
            unsafe {
                let _ = LockWorkStation();
            }
            "Locked PC".to_string()
        }

        QuickAction::TakeScreenshot => {
            unsafe {
                keybd_event(VK_SNAPSHOT, 0, KEYBD_EVENT_FLAGS(0), 0);
                thread::sleep(Duration::from_millis(20));
                keybd_event(VK_SNAPSHOT, 0, KEYEVENTF_KEYUP, 0);
            }
            "Screenshot copied to clipboard".to_string()
        }

        QuickAction::MuteAudio => {
            unsafe {
                keybd_event(VK_VOLUME_MUTE, 0, KEYBD_EVENT_FLAGS(0), 0);
                thread::sleep(Duration::from_millis(15));
                keybd_event(VK_VOLUME_MUTE, 0, KEYEVENTF_KEYUP, 0);
            }
            "Toggled audio mute".to_string()
        }

        QuickAction::UnmuteAudio => {
            unsafe {
                keybd_event(VK_VOLUME_MUTE, 0, KEYBD_EVENT_FLAGS(0), 0);
                thread::sleep(Duration::from_millis(15));
                keybd_event(VK_VOLUME_MUTE, 0, KEYEVENTF_KEYUP, 0);
            }
            "Toggled audio mute".to_string()
        }

        QuickAction::VolumeUp => {
            unsafe {
                for _ in 0..3 {
                    keybd_event(VK_VOLUME_UP, 0, KEYBD_EVENT_FLAGS(0), 0);
                    keybd_event(VK_VOLUME_UP, 0, KEYEVENTF_KEYUP, 0);
                    thread::sleep(Duration::from_millis(15));
                }
            }
            "Volume increased".to_string()
        }

        QuickAction::VolumeDown => {
            unsafe {
                for _ in 0..3 {
                    keybd_event(VK_VOLUME_DOWN, 0, KEYBD_EVENT_FLAGS(0), 0);
                    keybd_event(VK_VOLUME_DOWN, 0, KEYEVENTF_KEYUP, 0);
                    thread::sleep(Duration::from_millis(15));
                }
            }
            "Volume decreased".to_string()
        }

        QuickAction::RiftOpenSettings => {
            if let Some(app_handle) = app {
                if let Some(main_win) = app_handle.get_webview_window("main") {
                    let _ = main_win.eval("window.location.href = 'settings.html';");
                    crate::show_and_focus_window(&main_win);
                    "Opened Rift Settings".to_string()
                } else {
                    "Rift Settings window not available".to_string()
                }
            } else {
                "Settings handle unavailable".to_string()
            }
        }

        QuickAction::RiftOpenScratchpad => {
            if let Some(app_handle) = app {
                match crate::window::auxiliary::get_or_create_scratchpad(app_handle) {
                    Ok(_) => "Opened Rift Scratchpad".to_string(),
                    Err(e) => format!("Failed to open Scratchpad: {}", e),
                }
            } else {
                "Scratchpad handle unavailable".to_string()
            }
        }

        QuickAction::RiftOpenSpotlight => {
            if let Some(app_handle) = app {
                match crate::window::auxiliary::get_or_create_spotlight(app_handle) {
                    Ok(_) => "Opened Rift Spotlight".to_string(),
                    Err(e) => format!("Failed to open Spotlight: {}", e),
                }
            } else {
                "Spotlight handle unavailable".to_string()
            }
        }

        QuickAction::None => String::new(),
    }
}

#[cfg(not(windows))]
pub fn execute_quick_action(
    action: &QuickAction,
    app: Option<&tauri::AppHandle>,
    _target_hwnd: Option<WindowTarget>,
) -> String {
    match action {
        QuickAction::OpenApp { executable, label, spoken } => {
            let app_name = if !label.is_empty() { label.as_str() } else { spoken.as_str() };
            let status = std::process::Command::new("open")
                .arg("-a")
                .arg(app_name)
                .status();
            if status.map(|s| s.success()).unwrap_or(false) {
                format!("Opened {}", app_name)
            } else {
                let _ = std::process::Command::new("open")
                    .arg("-a")
                    .arg(executable)
                    .status();
                format!("Attempted to open {}", app_name)
            }
        }
        QuickAction::SwitchToApp { spoken } => {
            let script = format!("tell application \"{}\" to activate", spoken);
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .status();
            format!("Switched to {}", spoken)
        }
        QuickAction::CloseWindow => {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to keystroke \"w\" using command down")
                .status();
            "Closed active window".to_string()
        }
        QuickAction::MinimizeWindow => {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to keystroke \"m\" using command down")
                .status();
            "Minimized active window".to_string()
        }
        QuickAction::MaximizeWindow => {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to keystroke \"f\" using {control down, command down}")
                .status();
            "Maximized active window".to_string()
        }
        QuickAction::LockPC => {
            let _ = std::process::Command::new("/System/Library/CoreServices/Menu Extras/User.menu/Contents/Resources/CGSession")
                .arg("-suspend")
                .spawn();
            "Locked Mac".to_string()
        }
        QuickAction::TakeScreenshot => {
            let _ = std::process::Command::new("screencapture")
                .arg("-c")
                .spawn();
            "Screenshot copied to clipboard".to_string()
        }
        QuickAction::MuteAudio => {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg("set volume output muted (not (output muted of (get volume settings)))")
                .status();
            "Toggled audio mute".to_string()
        }
        QuickAction::UnmuteAudio => {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg("set volume output muted false")
                .status();
            "Unmuted audio".to_string()
        }
        QuickAction::VolumeUp => {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg("set volume output volume ((output volume of (get volume settings)) + 10)")
                .status();
            "Increased volume".to_string()
        }
        QuickAction::VolumeDown => {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg("set volume output volume ((output volume of (get volume settings)) - 10)")
                .status();
            "Decreased volume".to_string()
        }
        QuickAction::RiftOpenSettings => {
            if let Some(app_handle) = app {
                if let Some(w) = app_handle.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "Opened Rift Settings".to_string()
        }
        QuickAction::RiftOpenScratchpad => {
            if let Some(app_handle) = app {
                let _ = crate::window::auxiliary::get_or_create_scratchpad(app_handle);
            }
            "Opened Scratchpad".to_string()
        }
        QuickAction::RiftOpenSpotlight => {
            if let Some(app_handle) = app {
                let _ = crate::window::auxiliary::get_or_create_spotlight(app_handle);
            }
            "Opened Ask Rift Spotlight".to_string()
        }
        QuickAction::None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_system_actions() {
        assert_eq!(detect_quick_action("Lock my PC."), QuickAction::LockPC);
        assert_eq!(detect_quick_action("lock computer"), QuickAction::LockPC);
        assert_eq!(detect_quick_action("Take a screenshot!"), QuickAction::TakeScreenshot);
        assert_eq!(detect_quick_action("screenshot this"), QuickAction::TakeScreenshot);
        assert_eq!(detect_quick_action("Mute"), QuickAction::MuteAudio);
        assert_eq!(detect_quick_action("Unmute audio"), QuickAction::UnmuteAudio);
        assert_eq!(detect_quick_action("Volume up!"), QuickAction::VolumeUp);
        assert_eq!(detect_quick_action("volume down"), QuickAction::VolumeDown);
    }

    #[test]
    fn test_detect_window_management() {
        assert_eq!(detect_quick_action("Close this window."), QuickAction::CloseWindow);
        assert_eq!(detect_quick_action("minimize this"), QuickAction::MinimizeWindow);
        assert_eq!(detect_quick_action("Maximize window"), QuickAction::MaximizeWindow);
    }

    #[test]
    fn test_detect_app_launch() {
        assert_eq!(
            detect_quick_action("Open Chrome."),
            QuickAction::OpenApp {
                spoken: "chrome".to_string(),
                executable: "chrome".to_string(),
                label: "Google Chrome".to_string()
            }
        );
        assert_eq!(
            detect_quick_action("Launch VS Code"),
            QuickAction::OpenApp {
                spoken: "vs code".to_string(),
                executable: "code".to_string(),
                label: "VS Code".to_string()
            }
        );
        assert_eq!(
            detect_quick_action("Open calculator!"),
            QuickAction::OpenApp {
                spoken: "calculator".to_string(),
                executable: "calc".to_string(),
                label: "Calculator".to_string()
            }
        );
    }

    #[test]
    fn test_detect_switch_app() {
        assert_eq!(
            detect_quick_action("Switch to Chrome"),
            QuickAction::SwitchToApp {
                spoken: "chrome".to_string()
            }
        );
    }

    #[test]
    fn test_detect_rift_internal() {
        assert_eq!(detect_quick_action("Open Rift settings"), QuickAction::RiftOpenSettings);
        assert_eq!(detect_quick_action("Open scratchpad"), QuickAction::RiftOpenScratchpad);
        assert_eq!(detect_quick_action("Ask Rift"), QuickAction::RiftOpenSpotlight);
    }

    #[test]
    fn test_regular_dictation_falls_through() {
        assert_eq!(detect_quick_action("Open the door for me please"), QuickAction::None);
        assert_eq!(detect_quick_action("I want to discuss our goals"), QuickAction::None);
        assert_eq!(detect_quick_action("Hello world this is a test"), QuickAction::None);
    }
}
