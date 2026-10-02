#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::ptr::null_mut;
use std::thread;
use std::time::Duration;
#[cfg(windows)]
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND, LPARAM, WPARAM};
#[cfg(windows)]
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData};
#[cfg(windows)]
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND};
#[cfg(windows)]
use windows::Win32::System::Threading::GetCurrentThreadId;
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    VK_C, VK_CONTROL, VK_MENU, VK_SHIFT, VK_V,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowTextW,
    GetWindowThreadProcessId, IsWindow, PostMessageW, SetForegroundWindow, WM_CANCELMODE,
};

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: i32) -> i32;
}

#[cfg(windows)]
pub type WindowTarget = windows::Win32::Foundation::HWND;

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowTarget(pub usize);

pub struct TextInjector;

impl TextInjector {
    pub fn target_from_raw(raw: Option<usize>) -> Option<WindowTarget> {
        #[cfg(windows)]
        {
            raw.map(|val| windows::Win32::Foundation::HWND(val as *mut _))
        }
        #[cfg(not(windows))]
        {
            raw.map(WindowTarget)
        }
    }
}

#[cfg(windows)]
impl TextInjector {
    pub fn get_foreground_window() -> HWND {
        unsafe { GetForegroundWindow() }
    }

    pub fn is_valid_window(hwnd: HWND) -> bool {
        if hwnd.0.is_null() {
            return false;
        }
        unsafe { IsWindow(hwnd).as_bool() }
    }

    pub fn get_window_title(hwnd: HWND) -> Option<String> {
        if hwnd.0.is_null() {
            return None;
        }
        unsafe {
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut buf);
            if len > 0 {
                let s = String::from_utf16_lossy(&buf[..len as usize]).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
            None
        }
    }

    fn open_clipboard_with_retry(max_retries: usize) -> bool {
        for attempt in 0..max_retries {
            unsafe {
                if OpenClipboard(HWND(null_mut())).is_ok() {
                    return true;
                }
            }
            if attempt + 1 < max_retries {
                thread::sleep(Duration::from_millis(5 * (attempt as u64 + 1)));
            }
        }
        false
    }

    pub fn set_clipboard_text(text: &str) -> bool {
        let wide: Vec<u16> = OsStr::new(text).encode_wide().chain(Some(0)).collect();
        let bytes = wide.len() * std::mem::size_of::<u16>();

        unsafe {
            if !Self::open_clipboard_with_retry(5) {
                return false;
            }
            let _ = EmptyClipboard();

            let h_mem = match GlobalAlloc(GHND, bytes) {
                Ok(h) if !h.is_invalid() => h,
                _ => {
                    let _ = CloseClipboard();
                    return false;
                }
            };

            let p_mem = GlobalLock(h_mem);
            if p_mem.is_null() {
                let _ = CloseClipboard();
                return false;
            }

            std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, p_mem as *mut u8, bytes);
            let _ = GlobalUnlock(h_mem);

            const CF_UNICODETEXT: u32 = 13;
            let res = SetClipboardData(CF_UNICODETEXT, HANDLE(h_mem.0));
            let _ = CloseClipboard();

            res.is_ok()
        }
    }

    pub fn get_clipboard_text() -> Option<String> {
        unsafe {
            if !Self::open_clipboard_with_retry(5) {
                return None;
            }
            const CF_UNICODETEXT: u32 = 13;
            let h_data = match GetClipboardData(CF_UNICODETEXT) {
                Ok(h) if !h.is_invalid() => h,
                _ => {
                    let _ = CloseClipboard();
                    return None;
                }
            };
            let h_mem = HGLOBAL(h_data.0);
            let p_mem = GlobalLock(h_mem);
            if p_mem.is_null() {
                let _ = CloseClipboard();
                return None;
            }
            let ptr = p_mem as *const u16;
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            let result = String::from_utf16_lossy(slice);
            let _ = GlobalUnlock(h_mem);
            let _ = CloseClipboard();
            Some(result)
        }
    }

    pub fn send_ctrl_c() {
        unsafe {
            // Ctrl down
            let mut ctrl_down: INPUT = std::mem::zeroed();
            ctrl_down.r#type = INPUT_KEYBOARD;
            ctrl_down.Anonymous.ki = KEYBDINPUT {
                wVk: VK_CONTROL,
                wScan: 0,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            };
            SendInput(&[ctrl_down], std::mem::size_of::<INPUT>() as i32);
            thread::sleep(Duration::from_millis(4));

            // C down
            let mut c_down: INPUT = std::mem::zeroed();
            c_down.r#type = INPUT_KEYBOARD;
            c_down.Anonymous.ki = KEYBDINPUT {
                wVk: VK_C,
                wScan: 0,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            };
            SendInput(&[c_down], std::mem::size_of::<INPUT>() as i32);
            thread::sleep(Duration::from_millis(5));

            // C up
            let mut c_up: INPUT = std::mem::zeroed();
            c_up.r#type = INPUT_KEYBOARD;
            c_up.Anonymous.ki = KEYBDINPUT {
                wVk: VK_C,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };
            SendInput(&[c_up], std::mem::size_of::<INPUT>() as i32);
            thread::sleep(Duration::from_millis(4));

            // Ctrl up
            let mut ctrl_up: INPUT = std::mem::zeroed();
            ctrl_up.r#type = INPUT_KEYBOARD;
            ctrl_up.Anonymous.ki = KEYBDINPUT {
                wVk: VK_CONTROL,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };
            SendInput(&[ctrl_up], std::mem::size_of::<INPUT>() as i32);
        }
    }

    pub fn get_selected_text_from_target(target_hwnd: Option<HWND>) -> Option<String> {
        let prev_clipboard = Self::get_clipboard_text();

        // If target window was passed, ensure it is in foreground
        let current_fg = unsafe { GetForegroundWindow() };
        if let Some(hwnd) = target_hwnd {
            if Self::is_valid_window(hwnd) && current_fg != hwnd {
                unsafe {
                    let _ = SetForegroundWindow(hwnd);
                }
                thread::sleep(Duration::from_millis(25));
            }
        }

        // Empty clipboard so we can definitively detect if Ctrl+C produced new text
        unsafe {
            if OpenClipboard(HWND(null_mut())).is_ok() {
                let _ = EmptyClipboard();
                let _ = CloseClipboard();
            }
        }

        Self::release_modifier_keys();
        Self::send_ctrl_c();
        thread::sleep(Duration::from_millis(55));

        let selected = Self::get_clipboard_text();

        if let Some(text) = selected {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        // If no text was selected, restore previous clipboard text
        if let Some(prev) = prev_clipboard {
            Self::set_clipboard_text(&prev);
        }

        None
    }

    pub fn release_modifier_keys() {
        unsafe {
            let mut release_inputs = Vec::new();
            let mut released_menu = false;
            for &vk in &[VK_CONTROL, VK_SHIFT, VK_MENU] {
                if (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 {
                    if vk == VK_MENU {
                        released_menu = true;
                    }
                    let mut input: INPUT = std::mem::zeroed();
                    input.r#type = INPUT_KEYBOARD;
                    input.Anonymous.ki = KEYBDINPUT {
                        wVk: vk,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                    release_inputs.push(input);
                }
            }

            if !release_inputs.is_empty() {
                SendInput(&release_inputs, std::mem::size_of::<INPUT>() as i32);
                thread::sleep(Duration::from_millis(4));
            }

            // If VK_MENU (Alt) was released, send a dummy key event to prevent Windows
            // from activating the application's menu bar (SC_KEYMENU).
            if released_menu {
                const VK_NONAME: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY =
                    windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0xFC);
                let mut dummy: [INPUT; 2] = std::mem::zeroed();
                dummy[0].r#type = INPUT_KEYBOARD;
                dummy[0].Anonymous.ki.wVk = VK_NONAME;
                dummy[1].r#type = INPUT_KEYBOARD;
                dummy[1].Anonymous.ki.wVk = VK_NONAME;
                dummy[1].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
                SendInput(&dummy, std::mem::size_of::<INPUT>() as i32);
            }
        }
    }

    pub fn send_ctrl_v() {
        unsafe {
            let mut inputs: [INPUT; 4] = std::mem::zeroed();

            // 1. Ctrl down
            inputs[0].r#type = INPUT_KEYBOARD;
            inputs[0].Anonymous.ki = KEYBDINPUT {
                wVk: VK_CONTROL,
                wScan: 0,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            };

            // 2. V down
            inputs[1].r#type = INPUT_KEYBOARD;
            inputs[1].Anonymous.ki = KEYBDINPUT {
                wVk: VK_V,
                wScan: 0,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            };

            // 3. V up
            inputs[2].r#type = INPUT_KEYBOARD;
            inputs[2].Anonymous.ki = KEYBDINPUT {
                wVk: VK_V,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };

            // 4. Ctrl up
            inputs[3].r#type = INPUT_KEYBOARD;
            inputs[3].Anonymous.ki = KEYBDINPUT {
                wVk: VK_CONTROL,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            };

            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    pub fn paste_text(text: &str, target_hwnd: Option<HWND>) -> (bool, Option<HWND>) {
        if text.is_empty() {
            return (false, None);
        }

        let prev_clipboard = Self::get_clipboard_text();

        if !Self::set_clipboard_text(text) {
            return (false, None);
        }

        Self::release_modifier_keys();

        let my_pid = std::process::id();
        let current_fg = unsafe { GetForegroundWindow() };
        let mut current_pid = 0u32;
        if !current_fg.0.is_null() {
            unsafe {
                GetWindowThreadProcessId(current_fg, Some(&mut current_pid));
            }
        }

        // Determine the actual destination window:
        // If current foreground is a valid window and NOT our own Rift application,
        // that is the user's latest focus! Paste directly into it.
        let dest_hwnd = if !current_fg.0.is_null()
            && current_pid != 0
            && current_pid != my_pid
            && Self::is_valid_window(current_fg)
        {
            current_fg
        } else if let Some(hwnd) = target_hwnd {
            if Self::is_valid_window(hwnd) {
                hwnd
            } else {
                current_fg
            }
        } else {
            current_fg
        };

        if Self::is_valid_window(dest_hwnd) {
            // Only bring window to top if current foreground is NOT already the destination
            // (e.g. if the user clicked the Rift UI or floating widget)
            if current_fg != dest_hwnd {
                let cur_thread = unsafe { GetCurrentThreadId() };
                let mut dest_pid = 0u32;
                let dest_thread = unsafe { GetWindowThreadProcessId(dest_hwnd, Some(&mut dest_pid)) };

                if cur_thread != dest_thread && dest_thread != 0 {
                    unsafe {
                        let _ = AttachThreadInput(cur_thread, dest_thread, 1);
                        let _ = SetForegroundWindow(dest_hwnd);
                        let _ = BringWindowToTop(dest_hwnd);
                        let _ = AttachThreadInput(cur_thread, dest_thread, 0);
                    }
                } else {
                    unsafe {
                        let _ = SetForegroundWindow(dest_hwnd);
                    }
                }
                thread::sleep(Duration::from_millis(20));
            }

            // Cancel any accidental menu bar or popup mode on the destination window
            // to ensure keyboard focus remains directly in the edit control
            unsafe {
                let _ = PostMessageW(
                    dest_hwnd,
                    WM_CANCELMODE,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }

        Self::send_ctrl_v();

        // Restore prior clipboard content so user's clipboard is not permanently clobbered.
        // 350ms provides sufficient time for Electron apps (VS Code, Slack), browsers, and Office
        // to process the WM_KEYDOWN/WM_KEYUP/WM_PASTE dispatch before the original text is restored.
        if let Some(prev) = prev_clipboard {
            if !prev.is_empty() && prev != text {
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(350));
                    Self::set_clipboard_text(&prev);
                });
            }
        }

        (true, if dest_hwnd.0.is_null() { None } else { Some(dest_hwnd) })
    }

    /// Directly types text character by character into the target window using Windows SendInput
    /// with KEYEVENTF_UNICODE. Useful for terminals, remote desktop, and environments
    /// where clipboard paste (Ctrl+V) is restricted or produces raw escape sequences.
    pub fn type_unicode_text(text: &str, target_hwnd: Option<HWND>) -> (bool, Option<HWND>) {
        if text.is_empty() {
            return (false, None);
        }

        Self::release_modifier_keys();

        let my_pid = std::process::id();
        let current_fg = unsafe { GetForegroundWindow() };
        let mut current_pid = 0u32;
        if !current_fg.0.is_null() {
            unsafe {
                GetWindowThreadProcessId(current_fg, Some(&mut current_pid));
            }
        }

        let dest_hwnd = if !current_fg.0.is_null()
            && current_pid != 0
            && current_pid != my_pid
            && Self::is_valid_window(current_fg)
        {
            current_fg
        } else if let Some(hwnd) = target_hwnd {
            if Self::is_valid_window(hwnd) {
                hwnd
            } else {
                current_fg
            }
        } else {
            current_fg
        };

        if Self::is_valid_window(dest_hwnd) && current_fg != dest_hwnd {
            let cur_thread = unsafe { GetCurrentThreadId() };
            let mut dest_pid = 0u32;
            let dest_thread = unsafe { GetWindowThreadProcessId(dest_hwnd, Some(&mut dest_pid)) };

            if cur_thread != dest_thread && dest_thread != 0 {
                unsafe {
                    let _ = AttachThreadInput(cur_thread, dest_thread, 1);
                    let _ = SetForegroundWindow(dest_hwnd);
                    let _ = BringWindowToTop(dest_hwnd);
                    let _ = AttachThreadInput(cur_thread, dest_thread, 0);
                }
            } else {
                unsafe {
                    let _ = SetForegroundWindow(dest_hwnd);
                }
            }
            thread::sleep(Duration::from_millis(20));
        }

        const KEYEVENTF_UNICODE: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS =
            windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0x0004);

        let wide: Vec<u16> = OsStr::new(text).encode_wide().collect();
        let mut inputs = Vec::with_capacity(wide.len() * 2);

        for &ch in &wide {
            if ch == '\n' as u16 {
                let mut down: INPUT = unsafe { std::mem::zeroed() };
                down.r#type = INPUT_KEYBOARD;
                down.Anonymous.ki.wVk = windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;
                inputs.push(down);

                let mut up: INPUT = unsafe { std::mem::zeroed() };
                up.r#type = INPUT_KEYBOARD;
                up.Anonymous.ki.wVk = windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;
                up.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
                inputs.push(up);
            } else if ch == '\r' as u16 {
                continue;
            } else {
                let mut down: INPUT = unsafe { std::mem::zeroed() };
                down.r#type = INPUT_KEYBOARD;
                down.Anonymous.ki.wScan = ch;
                down.Anonymous.ki.dwFlags = KEYEVENTF_UNICODE;
                inputs.push(down);

                let mut up: INPUT = unsafe { std::mem::zeroed() };
                up.r#type = INPUT_KEYBOARD;
                up.Anonymous.ki.wScan = ch;
                up.Anonymous.ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;
                inputs.push(up);
            }
        }

        if !inputs.is_empty() {
            for chunk in inputs.chunks(64) {
                unsafe {
                    SendInput(chunk, std::mem::size_of::<INPUT>() as i32);
                }
                thread::sleep(Duration::from_millis(1));
            }
        }

        (true, if dest_hwnd.0.is_null() { None } else { Some(dest_hwnd) })
    }

    pub fn simulate_undo(target_hwnd: Option<HWND>) -> bool {
        let current_fg = unsafe { GetForegroundWindow() };
        if let Some(hwnd) = target_hwnd {
            if Self::is_valid_window(hwnd) && current_fg != hwnd {
                unsafe {
                    let _ = SetForegroundWindow(hwnd);
                }
                thread::sleep(Duration::from_millis(25));
            }
        }
        Self::release_modifier_keys();
        unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::VK_Z;
            let mut ctrl_down: INPUT = std::mem::zeroed();
            ctrl_down.r#type = INPUT_KEYBOARD;
            ctrl_down.Anonymous.ki.wVk = VK_CONTROL;
            SendInput(&[ctrl_down], std::mem::size_of::<INPUT>() as i32);
            thread::sleep(Duration::from_millis(4));

            let mut z_down: INPUT = std::mem::zeroed();
            z_down.r#type = INPUT_KEYBOARD;
            z_down.Anonymous.ki.wVk = VK_Z;
            SendInput(&[z_down], std::mem::size_of::<INPUT>() as i32);
            thread::sleep(Duration::from_millis(15));

            let mut z_up: INPUT = std::mem::zeroed();
            z_up.r#type = INPUT_KEYBOARD;
            z_up.Anonymous.ki.wVk = VK_Z;
            z_up.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            SendInput(&[z_up], std::mem::size_of::<INPUT>() as i32);
            thread::sleep(Duration::from_millis(4));

            let mut ctrl_up: INPUT = std::mem::zeroed();
            ctrl_up.r#type = INPUT_KEYBOARD;
            ctrl_up.Anonymous.ki.wVk = VK_CONTROL;
            ctrl_up.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            SendInput(&[ctrl_up], std::mem::size_of::<INPUT>() as i32);
        }
        true
    }

    pub fn get_foreground_window_pid_and_raw() -> Option<(u32, isize)> {
        let fg = unsafe { GetForegroundWindow() };
        if !fg.0.is_null() {
            let mut fg_pid = 0u32;
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(fg, Some(&mut fg_pid));
            }
            Some((fg_pid, fg.0 as isize))
        } else {
            None
        }
    }
}

#[cfg(target_os = "macos")]
mod macos_inject {
    use std::ffi::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CGEventCreateKeyboardEvent(
            source: *const c_void,
            virtual_key: u16,
            key_down: bool,
        ) -> *mut c_void;
        pub fn CGEventSetFlags(event: *mut c_void, flags: u64);
        pub fn CGEventPost(tap: u32, event: *mut c_void);
        pub fn CFRelease(cf: *const c_void);
    }

    pub const K_CG_SESSION_EVENT_TAP: u32 = 1;
    pub const K_CG_HID_EVENT_TAP: u32 = 0;
    pub const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x00100000;
    pub const K_VK_V: u16 = 0x09;
    pub const K_VK_C: u16 = 0x08;
    pub const K_VK_Z: u16 = 0x06;

    pub fn send_key_with_command(vk: u16) -> bool {
        unsafe {
            let event_down = CGEventCreateKeyboardEvent(std::ptr::null(), vk, true);
            if event_down.is_null() {
                return false;
            }
            CGEventSetFlags(event_down, K_CG_EVENT_FLAG_MASK_COMMAND);
            CGEventPost(K_CG_SESSION_EVENT_TAP, event_down);
            CGEventPost(K_CG_HID_EVENT_TAP, event_down);
            CFRelease(event_down);

            std::thread::sleep(std::time::Duration::from_millis(15));

            let event_up = CGEventCreateKeyboardEvent(std::ptr::null(), vk, false);
            if event_up.is_null() {
                return false;
            }
            CGEventSetFlags(event_up, K_CG_EVENT_FLAG_MASK_COMMAND);
            CGEventPost(K_CG_SESSION_EVENT_TAP, event_up);
            CGEventPost(K_CG_HID_EVENT_TAP, event_up);
            CFRelease(event_up);
        }
        true
    }
}

#[cfg(not(windows))]
impl TextInjector {
    pub fn get_foreground_window() -> WindowTarget {
        #[cfg(target_os = "macos")]
        {
            if let Some((pid, _)) = Self::get_foreground_window_pid_and_raw() {
                return WindowTarget(pid as usize);
            }
        }
        WindowTarget(0)
    }

    pub fn is_valid_window(_w: WindowTarget) -> bool {
        true
    }

    pub fn get_window_title(_w: WindowTarget) -> Option<String> {
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                let title = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
        None
    }

    pub fn set_clipboard_text(text: &str) -> bool {
        use std::io::Write;
        let mut child = match std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return false,
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        child.wait().map(|s| s.success()).unwrap_or(false)
    }

    pub fn get_clipboard_text() -> Option<String> {
        let output = std::process::Command::new("pbpaste").output().ok()?;
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            None
        }
    }

    pub fn paste_text(text: &str, target_hwnd: Option<WindowTarget>) -> (bool, Option<WindowTarget>) {
        if text.is_empty() {
            return (false, target_hwnd);
        }

        let prev_clipboard = Self::get_clipboard_text();

        if !Self::set_clipboard_text(text) {
            return (false, target_hwnd);
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(target) = target_hwnd {
                if target.0 != 0 {
                    let script = format!(
                        "tell application \"System Events\" to set frontmost of (first process whose unix id is {}) to true",
                        target.0
                    );
                    let _ = std::process::Command::new("osascript")
                        .arg("-e")
                        .arg(&script)
                        .status();
                    thread::sleep(Duration::from_millis(25));
                }
            }
        }

        thread::sleep(Duration::from_millis(25));

        #[cfg(target_os = "macos")]
        let mut pasted = macos_inject::send_key_with_command(macos_inject::K_VK_V);

        #[cfg(not(target_os = "macos"))]
        let mut pasted = false;

        if !pasted {
            let status = std::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to keystroke \"v\" using command down")
                .status();
            pasted = status.map(|s| s.success()).unwrap_or(false);
        }

        // Restore prior clipboard content after buffer delay
        if let Some(prev) = prev_clipboard {
            if !prev.is_empty() && prev != text {
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(350));
                    Self::set_clipboard_text(&prev);
                });
            }
        }

        (pasted, target_hwnd)
    }

    pub fn type_unicode_text(text: &str, target_hwnd: Option<WindowTarget>) -> (bool, Option<WindowTarget>) {
        Self::paste_text(text, target_hwnd)
    }

    pub fn simulate_undo(_target_hwnd: Option<WindowTarget>) -> bool {
        #[cfg(target_os = "macos")]
        {
            if macos_inject::send_key_with_command(macos_inject::K_VK_Z) {
                return true;
            }
        }
        let status = std::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to keystroke \"z\" using command down")
            .status();
        status.map(|s| s.success()).unwrap_or(false)
    }

    pub fn get_selected_text_from_target(_target_hwnd: Option<WindowTarget>) -> Option<String> {
        let prev_clip = Self::get_clipboard_text();
        #[cfg(target_os = "macos")]
        macos_inject::send_key_with_command(macos_inject::K_VK_C);
        #[cfg(not(target_os = "macos"))]
        {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to keystroke \"c\" using command down")
                .status();
        }
        thread::sleep(Duration::from_millis(50));
        let selected = Self::get_clipboard_text();
        if let Some(ref prev) = prev_clip {
            let _ = Self::set_clipboard_text(prev);
        }
        selected
    }

    pub fn get_foreground_window_pid_and_raw() -> Option<(u32, isize)> {
        #[cfg(target_os = "macos")]
        {
            let my_pid = std::process::id();
            let output = std::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to get unix id of first application process whose frontmost is true")
                .output()
                .ok()?;
            if output.status.success() {
                let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if pid != 0 && pid != my_pid {
                        return Some((pid, pid as isize));
                    }
                }
            }
        }
        None
    }
}

