use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, RwLock};
#[cfg(windows)]
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

#[derive(Debug, Clone)]
enum HookEvent {
    PttPress,
    PttRelease,
    TogglePress,
    ShowWidget,
    Captured {
        key_name: String,
        combo_name: String,
        mode: String,
    },
}

#[cfg(windows)]
static mut HOOK_HANDLE: Option<HHOOK> = None;
#[cfg(target_os = "macos")]
static mut MACOS_RUN_LOOP: Option<macos_tap::CFRunLoopRef> = None;
static mut KEY_CALLBACK_PRESS: Option<Box<dyn Fn() + Send + Sync>> = None;
static mut KEY_CALLBACK_RELEASE: Option<Box<dyn Fn() + Send + Sync>> = None;
static mut KEY_CALLBACK_TOGGLE: Option<Box<dyn Fn() + Send + Sync>> = None;
static mut KEY_CALLBACK_SHOW_WIDGET: Option<Box<dyn Fn() + Send + Sync>> = None;
static mut KEY_CALLBACK_CAPTURED: Option<Box<dyn Fn(String, String, String) + Send + Sync>> = None;

static EVENT_SENDER: Mutex<Option<Sender<HookEvent>>> = Mutex::new(None);

static PTT_KEYS: RwLock<Vec<u32>> = RwLock::new(Vec::new());
static TOGGLE_KEYS: RwLock<Vec<u32>> = RwLock::new(Vec::new());
static SHOW_WIDGET_KEYS: RwLock<Vec<u32>> = RwLock::new(Vec::new());

static IS_PTT_PRESSED: AtomicBool = AtomicBool::new(false);
static IS_TOGGLE_KEY_DOWN: AtomicBool = AtomicBool::new(false);
static LAST_TOGGLE_TIME: Mutex<Option<std::time::Instant>> = Mutex::new(None);

static IS_WIDGET_KEY_DOWN: AtomicBool = AtomicBool::new(false);
static LAST_WIDGET_TIME: Mutex<Option<std::time::Instant>> = Mutex::new(None);

static IS_CAPTURING: AtomicBool = AtomicBool::new(false);
static CAPTURE_MODE: Mutex<Option<String>> = Mutex::new(None);
static CAPTURED_KEYS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

static ACTIVE_KEYS_MACOS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

fn send_event(event: HookEvent) {
    if let Ok(guard) = EVENT_SENDER.try_lock() {
        if let Some(tx) = &*guard {
            let _ = tx.send(event);
        }
    }
}

pub fn is_modifier_vk(vk: u32) -> bool {
    matches!(
        vk,
        0x10 | 0xA0 | 0xA1 // Shift
        | 0x11 | 0xA2 | 0xA3 // Ctrl
        | 0x12 | 0xA4 | 0xA5 // Alt
        | 0x5B | 0x5C // Win
    )
}

pub fn name_to_vk(key_name: &str) -> u32 {
    let lower = key_name.to_lowercase().trim().to_string();
    match lower.as_str() {
        "fn" | "fnlock" => 0xFF,
        "mute" | "volumemute" | "audiovolumemute" => 0xAD,
        "volume down" | "volumedown" | "audiovolumedown" => 0xAE,
        "volume up" | "volumeup" | "audiovolumeup" => 0xAF,
        "next track" | "nexttrack" | "mediatracknext" => 0xB0,
        "prev track" | "previous track" | "prevtrack" | "mediatrackprevious" => 0xB1,
        "stop" | "mediastop" => 0xB2,
        "play/pause" | "playpause" | "mediaplaypause" => 0xB3,
        "right alt" | "ralt" | "alt_r" | "alt gr" | "altgr" | "right option" | "roption" => 0xA5,
        "left alt" | "lalt" | "alt_l" | "left option" | "loption" => 0xA4,
        "alt" | "option" => 0x12,
        "right ctrl" | "rctrl" => 0xA3,
        "left ctrl" | "lctrl" => 0xA2,
        "ctrl" | "control" => 0x11,
        "shift" => 0x10,
        "right shift" | "rshift" => 0xA1,
        "left shift" | "lshift" => 0xA0,
        "windows" | "win" | "lwin" | "rwin" | "meta" | "cmd" | "command" => 0x5B,
        "space" => 0x20,
        "tab" => 0x09,
        "enter" | "return" => 0x0D,
        "backspace" | "back" => 0x08,
        "escape" | "esc" => 0x1B,
        "caps lock" | "capslock" => 0x14,
        "delete" | "del" => 0x2E,
        "insert" | "ins" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "page up" | "pageup" | "pgup" => 0x21,
        "page down" | "pagedown" | "pgdn" => 0x22,
        "up" | "arrow up" => 0x26,
        "down" | "arrow down" => 0x28,
        "left" | "arrow left" => 0x25,
        "right" | "arrow right" => 0x27,
        "pause" | "break" => 0x13,
        "print screen" | "printscreen" | "prtsc" => 0x2C,
        "scroll lock" | "scrolllock" => 0x91,
        "num lock" | "numlock" => 0x90,
        "tilde" | "`" | "grave" => 0xC0,
        "-" | "minus" => 0xBD,
        "=" | "equals" | "equal" => 0xBB,
        "[" | "bracketleft" => 0xDB,
        "]" | "bracketright" => 0xDD,
        "\\" | "backslash" => 0xDC,
        ";" | "semicolon" => 0xBA,
        "'" | "quote" => 0xDE,
        "," | "comma" => 0xBC,
        "." | "period" => 0xBE,
        "/" | "slash" => 0xBF,
        "f1" => 0x70,
        "f2" => 0x71,
        "f3" => 0x72,
        "f4" => 0x73,
        "f5" => 0x74,
        "f6" => 0x75,
        "f7" => 0x76,
        "f8" => 0x77,
        "f9" => 0x78,
        "f10" => 0x79,
        "f11" => 0x7A,
        "f12" => 0x7B,
        "f13" => 0x7C,
        "f14" => 0x7D,
        "f15" => 0x7E,
        "f16" => 0x7F,
        "f17" => 0x80,
        "f18" => 0x81,
        "f19" => 0x82,
        "f20" => 0x83,
        "f21" => 0x84,
        "f22" => 0x85,
        "f23" => 0x86,
        "f24" => 0x87,
        _ => {
            if lower.starts_with('f') {
                if let Ok(num) = lower[1..].parse::<u32>() {
                    if (1..=24).contains(&num) {
                        return 0x70 + (num - 1);
                    }
                }
            }
            if lower.len() == 1 && lower.chars().next().unwrap().is_ascii_alphabetic() {
                return lower.chars().next().unwrap().to_ascii_uppercase() as u32;
            }
            if lower.len() == 1 && lower.chars().next().unwrap().is_ascii_digit() {
                return lower.chars().next().unwrap() as u32;
            }
            0xA5
        }
    }
}

pub fn parse_key_combination(combo_str: &str) -> Vec<u32> {
    let parts: Vec<&str> = combo_str
        .split('+')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return vec![0xA5]; // Default Right Alt
    }
    parts.iter().map(|p| name_to_vk(p)).collect()
}

pub fn vk_to_name(vk: u32) -> String {
    match vk {
        0xA5 => "Right Alt".to_string(),
        0xA4 => "Left Alt".to_string(),
        0x12 => "Alt".to_string(),
        0xA3 => "Right Ctrl".to_string(),
        0xA2 => "Left Ctrl".to_string(),
        0x11 => "Ctrl".to_string(),
        0xA1 => "Right Shift".to_string(),
        0xA0 => "Left Shift".to_string(),
        0x10 => "Shift".to_string(),
        0x5B | 0x5C => "Win".to_string(),
        0x20 => "Space".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Enter".to_string(),
        0x08 => "Backspace".to_string(),
        0x1B => "Esc".to_string(),
        0x14 => "Caps Lock".to_string(),
        0x2E => "Delete".to_string(),
        0x2D => "Insert".to_string(),
        0x24 => "Home".to_string(),
        0x23 => "End".to_string(),
        0x21 => "Page Up".to_string(),
        0x22 => "Page Down".to_string(),
        0x26 => "Up".to_string(),
        0x28 => "Down".to_string(),
        0x25 => "Left".to_string(),
        0x27 => "Right".to_string(),
        0x13 => "Pause".to_string(),
        0x2C => "Print Screen".to_string(),
        0x91 => "Scroll Lock".to_string(),
        0x90 => "Num Lock".to_string(),
        0xC0 => "`".to_string(),
        0xBD => "-".to_string(),
        0xBB => "=".to_string(),
        0xDB => "[".to_string(),
        0xDD => "]".to_string(),
        0xDC => "\\".to_string(),
        0xBA => ";".to_string(),
        0xDE => "'".to_string(),
        0xBC => ",".to_string(),
        0xBE => ".".to_string(),
        0xBF => "/".to_string(),
        0xFF | 0xEB | 0xE7 | 0x88 => "Fn".to_string(),
        0xAD => "Mute".to_string(),
        0xAE => "Volume Down".to_string(),
        0xAF => "Volume Up".to_string(),
        0xB0 => "Next Track".to_string(),
        0xB1 => "Prev Track".to_string(),
        0xB2 => "Media Stop".to_string(),
        0xB3 => "Play/Pause".to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x70 + 1),
        0x30..=0x39 => ((vk as u8) as char).to_string(),
        0x41..=0x5A => ((vk as u8) as char).to_string(),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0x6A => "Num *".to_string(),
        0x6B => "Num +".to_string(),
        0x6D => "Num -".to_string(),
        0x6E => "Num .".to_string(),
        0x6F => "Num /".to_string(),
        _ => format!("Key-0x{:X}", vk),
    }
}

pub fn is_vk_down(vk: u32) -> bool {
    #[cfg(windows)]
    unsafe {
        match vk {
            0x11 | 0xA2 | 0xA3 => {
                (GetAsyncKeyState(0x11) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(0xA2) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(0xA3) as u16 & 0x8000 != 0)
            }
            0x10 | 0xA0 | 0xA1 => {
                (GetAsyncKeyState(0x10) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(0xA0) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(0xA1) as u16 & 0x8000 != 0)
            }
            0x12 | 0xA4 | 0xA5 => {
                (GetAsyncKeyState(0x12) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(0xA4) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(0xA5) as u16 & 0x8000 != 0)
            }
            0x5B | 0x5C => {
                (GetAsyncKeyState(0x5B) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(0x5C) as u16 & 0x8000 != 0)
            }
            0xFF => {
                (GetAsyncKeyState(0xFF) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(0xEB) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(0xE7) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(0x88) as u16 & 0x8000 != 0)
            }
            _ => GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0,
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(active) = ACTIVE_KEYS_MACOS.lock() {
            active.iter().any(|&k| vk_matches(k, vk))
        } else {
            false
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = vk;
        false
    }
}

pub fn vk_matches(current_vk: u32, target_vk: u32) -> bool {
    if current_vk == target_vk {
        return true;
    }
    match target_vk {
        0x11 | 0xA2 | 0xA3 => current_vk == 0xA2 || current_vk == 0xA3 || current_vk == 0x11,
        0x12 | 0xA4 | 0xA5 => current_vk == 0xA4 || current_vk == 0xA5 || current_vk == 0x12,
        0x10 | 0xA0 | 0xA1 => current_vk == 0xA0 || current_vk == 0xA1 || current_vk == 0x10,
        0x5B | 0x5C => current_vk == 0x5B || current_vk == 0x5C,
        0xFF => current_vk == 0xFF || current_vk == 0xEB || current_vk == 0xE7 || current_vk == 0x88,
        _ => false,
    }
}

#[cfg(windows)]
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kbd = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let msg = wparam.0 as u32;

        // 0. Ignore synthetic/injected keystrokes (e.g. from SendInput/TextInjector)
        // LLKHF_INJECTED = 0x00000001
        if (kbd.flags.0 & 1) != 0 {
            return CallNextHookEx(HOOK_HANDLE.unwrap_or_default(), code, wparam, lparam);
        }

        let effective_vk = if (kbd.vkCode == 0 || kbd.vkCode == 0xFF || kbd.vkCode == 0xEB || kbd.vkCode == 0xE7 || kbd.vkCode == 0x88)
            || kbd.scanCode == 0x73 || kbd.scanCode == 0x63
        {
            0xFF
        } else {
            kbd.vkCode
        };

        // 1. Interactive Capture Mode
        if IS_CAPTURING.load(Ordering::Relaxed) {
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                if effective_vk == 0x1B && !is_vk_down(0x11) && !is_vk_down(0x12) && !is_vk_down(0x10) && !is_vk_down(0x5B) && !is_vk_down(0x5C) {
                    // Escape alone cancels capture
                    if let Ok(mut keys) = CAPTURED_KEYS.try_lock() {
                        keys.clear();
                    }
                    IS_CAPTURING.store(false, Ordering::SeqCst);
                    return CallNextHookEx(HOOK_HANDLE.unwrap_or_default(), code, wparam, lparam);
                }

                if let Ok(mut keys) = CAPTURED_KEYS.try_lock() {
                    if !keys.contains(&effective_vk) {
                        keys.push(effective_vk);
                    }
                }
            } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                let captured_vks = if let Ok(mut keys) = CAPTURED_KEYS.try_lock() {
                    let res = keys.clone();
                    keys.clear();
                    res
                } else {
                    Vec::new()
                };

                IS_CAPTURING.store(false, Ordering::SeqCst);

                if !captured_vks.is_empty() {
                    let mut has_ctrl = false;
                    let mut has_alt = false;
                    let mut has_shift = false;
                    let mut has_win = false;
                    let mut others = Vec::new();

                    for &k in &captured_vks {
                        match k {
                            0x11 | 0xA2 | 0xA3 => has_ctrl = true,
                            0x12 | 0xA4 | 0xA5 => {
                                if captured_vks.len() == 1 && (k == 0xA5 || k == 0x12) {
                                    others.push(vk_to_name(k));
                                } else {
                                    has_alt = true;
                                }
                            }
                            0x10 | 0xA0 | 0xA1 => has_shift = true,
                            0x5B | 0x5C => has_win = true,
                            other_vk => {
                                let name = vk_to_name(other_vk);
                                if !others.contains(&name) {
                                    others.push(name);
                                }
                            }
                        }
                    }

                    let mut parts = Vec::new();
                    if has_ctrl { parts.push("Ctrl".to_string()); }
                    if has_alt { parts.push("Alt".to_string()); }
                    if has_shift { parts.push("Shift".to_string()); }
                    if has_win { parts.push("Win".to_string()); }
                    for name in others {
                        if !parts.contains(&name) {
                            parts.push(name);
                        }
                    }

                    if parts.is_empty() {
                        parts.push(vk_to_name(effective_vk));
                    }

                    let combo_name = parts.join("+");
                    let key_name = parts.last().cloned().unwrap_or_else(|| vk_to_name(effective_vk));
                    let mode = if parts.len() > 1 { "combo" } else { "single" };

                    send_event(HookEvent::Captured {
                        key_name,
                        combo_name,
                        mode: mode.to_string(),
                    });
                }
            }
            return CallNextHookEx(HOOK_HANDLE.unwrap_or_default(), code, wparam, lparam);
        }

        // 2. Push-to-Talk Handling (supports single key and combos)
        let ptt_keys = if let Ok(guard) = PTT_KEYS.try_read() {
            guard.clone()
        } else {
            Vec::new()
        };

        if !ptt_keys.is_empty() {
            let is_match = ptt_keys.iter().any(|&k| vk_matches(effective_vk, k));
            if is_match {
                if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                    let all_down = ptt_keys.iter().all(|&k| {
                        if vk_matches(effective_vk, k) {
                            true
                        } else {
                            is_vk_down(k)
                        }
                    });

                    if all_down {
                        if !IS_PTT_PRESSED.swap(true, Ordering::SeqCst) {
                            send_event(HookEvent::PttPress);
                        }
                        // Consume the trigger key down so it does not trigger Alt menu bar or leak into active text field
                        return LRESULT(1);
                    }
                } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                    let was_pressed = IS_PTT_PRESSED.swap(false, Ordering::SeqCst);
                    if was_pressed {
                        send_event(HookEvent::PttRelease);
                        // Consume key release ONLY for single dedicated key (e.g. Right Alt) to prevent SC_KEYMENU
                        // Never consume modifier release for combo keys so modifiers never get stuck!
                        if ptt_keys.len() == 1 {
                            return LRESULT(1);
                        }
                    }
                }
            }
        }

        // 3. Toggle Recording Handling (supports single key and combos with repeat debounce)
        let toggle_keys = if let Ok(guard) = TOGGLE_KEYS.try_read() {
            guard.clone()
        } else {
            Vec::new()
        };

        if !toggle_keys.is_empty() {
            let is_match = toggle_keys.iter().any(|&k| vk_matches(effective_vk, k));
            if is_match {
                if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                    let all_down = toggle_keys.iter().all(|&k| {
                        if vk_matches(effective_vk, k) {
                            true
                        } else {
                            is_vk_down(k)
                        }
                    });

                    if all_down {
                        let now = std::time::Instant::now();
                        let mut can_toggle = false;
                        if !IS_TOGGLE_KEY_DOWN.swap(true, Ordering::SeqCst) {
                            if let Ok(mut last_t) = LAST_TOGGLE_TIME.try_lock() {
                                if last_t.map(|t| now.duration_since(t).as_millis() > 300).unwrap_or(true) {
                                    *last_t = Some(now);
                                    can_toggle = true;
                                }
                            }
                        }
                        if can_toggle {
                            send_event(HookEvent::TogglePress);
                        }
                        // Consume trigger keydown so it does not type into the active field
                        return LRESULT(1);
                    }
                } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                    let any_down = toggle_keys.iter().any(|&k| {
                        if vk_matches(effective_vk, k) {
                            false
                        } else {
                            is_vk_down(k)
                        }
                    });
                    if !any_down {
                        IS_TOGGLE_KEY_DOWN.store(false, Ordering::SeqCst);
                    }
                    // NEVER swallow keyups for modifiers on toggle keys!
                    // Let all keyup events pass freely to Windows.
                }
            }
        }

        // 4. Show Widget Handling (Ctrl+Shift+Space default)
        let widget_keys = if let Ok(guard) = SHOW_WIDGET_KEYS.try_read() {
            guard.clone()
        } else {
            Vec::new()
        };

        if !widget_keys.is_empty() {
            let is_match = widget_keys.iter().any(|&k| vk_matches(effective_vk, k));
            if is_match {
                if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                    let all_down = widget_keys.iter().all(|&k| {
                        if vk_matches(effective_vk, k) {
                            true
                        } else {
                            is_vk_down(k)
                        }
                    });

                    if all_down {
                        let now = std::time::Instant::now();
                        let mut can_show = false;
                        if !IS_WIDGET_KEY_DOWN.swap(true, Ordering::SeqCst) {
                            if let Ok(mut last_t) = LAST_WIDGET_TIME.try_lock() {
                                if last_t.map(|t| now.duration_since(t).as_millis() > 300).unwrap_or(true) {
                                    *last_t = Some(now);
                                    can_show = true;
                                }
                            }
                        }
                        if can_show {
                            send_event(HookEvent::ShowWidget);
                        }
                        // Consume the trigger keydown (e.g. Space) so it does not type a space
                        return LRESULT(1);
                    }
                } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                    let any_down = widget_keys.iter().any(|&k| {
                        if vk_matches(effective_vk, k) {
                            false
                        } else {
                            is_vk_down(k)
                        }
                    });
                    if !any_down {
                        IS_WIDGET_KEY_DOWN.store(false, Ordering::SeqCst);
                    }
                    // CRITICAL: NEVER consume modifier keyups! All modifier releases MUST reach Windows.
                }
            }
        }
    }
    CallNextHookEx(HOOK_HANDLE.unwrap_or_default(), code, wparam, lparam)
}

#[cfg(target_os = "macos")]
mod macos_tap {
    use std::ffi::c_void;

    pub type CGEventRef = *mut c_void;
    pub type CGEventTapProxy = *mut c_void;
    pub type CFMachPortRef = *mut c_void;
    pub type CFRunLoopSourceRef = *mut c_void;
    pub type CFRunLoopRef = *mut c_void;
    pub type CFStringRef = *const c_void;

    pub type CGEventTapCallBack = unsafe extern "C" fn(
        proxy: CGEventTapProxy,
        event_type: u32,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef;

    #[link(name = "CoreGraphics", kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: CGEventTapCallBack,
            user_info: *mut c_void,
        ) -> CFMachPortRef;

        pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
        pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
        pub fn CGEventGetFlags(event: CGEventRef) -> u64;

        pub fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: CFMachPortRef,
            order: isize,
        ) -> CFRunLoopSourceRef;

        pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
        pub fn CFRunLoopRun();
        pub fn CFRunLoopStop(rl: CFRunLoopRef);
        pub fn CFRelease(cf: *const c_void);

        pub static kCFRunLoopCommonModes: CFStringRef;
    }

    pub const K_CG_SESSION_EVENT_TAP: u32 = 1;
    pub const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
    pub const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;

    pub const K_CG_EVENT_KEY_DOWN: u32 = 10;
    pub const K_CG_EVENT_KEY_UP: u32 = 11;
    pub const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;

    pub const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 14;

    pub const K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT: u64 = 0x00010000;
    pub const K_CG_EVENT_FLAG_MASK_SHIFT: u64       = 0x00020000;
    pub const K_CG_EVENT_FLAG_MASK_CONTROL: u64     = 0x00040000;
    pub const K_CG_EVENT_FLAG_MASK_ALTERNATE: u64   = 0x00080000;
    pub const K_CG_EVENT_FLAG_MASK_COMMAND: u64     = 0x00100000;
    pub const K_CG_EVENT_FLAG_MASK_SECONDARY_FN: u64= 0x00800000;
}

#[cfg(target_os = "macos")]
fn macos_keycode_to_vk(keycode: u32) -> u32 {
    match keycode {
        0x3D => 0xA5, // Right Option
        0x3A => 0xA4, // Left Option
        0x37 => 0x5B, // Left Command
        0x36 => 0x5C, // Right Command
        0x3B => 0xA2, // Left Control
        0x3E => 0xA3, // Right Control
        0x38 => 0xA0, // Left Shift
        0x3C => 0xA1, // Right Shift
        0x39 => 0x14, // Caps Lock
        0x3F => 0xFF, // Function (fn)
        0x31 => 0x20, // Space
        0x24 => 0x0D, // Return
        0x30 => 0x09, // Tab
        0x33 => 0x08, // Delete / Backspace
        0x35 => 0x1B, // Escape
        0x7A => 0x70, // F1
        0x78 => 0x71, // F2
        0x63 => 0x72, // F3
        0x76 => 0x73, // F4
        0x60 => 0x74, // F5
        0x61 => 0x75, // F6
        0x62 => 0x76, // F7
        0x64 => 0x77, // F8
        0x65 => 0x78, // F9
        0x6D => 0x79, // F10
        0x67 => 0x7A, // F11
        0x6F => 0x7B, // F12
        0x7E => 0x26, // Up
        0x7D => 0x28, // Down
        0x7B => 0x25, // Left
        0x7C => 0x27, // Right
        0x73 => 0x24, // Home
        0x77 => 0x23, // End
        0x74 => 0x21, // Page Up
        0x79 => 0x22, // Page Down
        0x75 => 0x2E, // Forward Delete
        // Letters
        0x00 => 0x41, // A
        0x0B => 0x42, // B
        0x08 => 0x43, // C
        0x02 => 0x44, // D
        0x0E => 0x45, // E
        0x03 => 0x46, // F
        0x05 => 0x47, // G
        0x04 => 0x48, // H
        0x22 => 0x49, // I
        0x26 => 0x4A, // J
        0x28 => 0x4B, // K
        0x25 => 0x4C, // L
        0x2E => 0x4D, // M
        0x2D => 0x4E, // N
        0x1F => 0x4F, // O
        0x23 => 0x50, // P
        0x0C => 0x51, // Q
        0x0F => 0x52, // R
        0x01 => 0x53, // S
        0x11 => 0x54, // T
        0x20 => 0x55, // U
        0x09 => 0x56, // V
        0x0D => 0x57, // W
        0x07 => 0x58, // X
        0x10 => 0x59, // Y
        0x06 => 0x5A, // Z
        // Numbers
        0x1D => 0x30, // 0
        0x12 => 0x31, // 1
        0x13 => 0x32, // 2
        0x14 => 0x33, // 3
        0x15 => 0x34, // 4
        0x17 => 0x35, // 5
        0x16 => 0x36, // 6
        0x1A => 0x37, // 7
        0x1C => 0x38, // 8
        0x19 => 0x39, // 9
        // Symbols
        0x32 => 0xC0, // `
        0x1B => 0xBD, // -
        0x18 => 0xBB, // =
        0x21 => 0xDB, // [
        0x1E => 0xDD, // ]
        0x2A => 0xDC, // \
        0x29 => 0xBA, // ;
        0x27 => 0xDE, // '
        0x2B => 0xBC, // ,
        0x2F => 0xBE, // .
        0x2C => 0xBF, // /
        other => other,
    }
}

#[cfg(target_os = "macos")]
fn dispatch_macos_key(effective_vk: u32, is_press: bool) {
    if let Ok(mut active) = ACTIVE_KEYS_MACOS.lock() {
        if is_press {
            if !active.contains(&effective_vk) {
                active.push(effective_vk);
            }
        } else {
            active.retain(|&k| k != effective_vk);
        }
    }

    let is_key_down = |vk: u32| -> bool {
        if let Ok(active) = ACTIVE_KEYS_MACOS.lock() {
            active.iter().any(|&k| vk_matches(k, vk))
        } else {
            false
        }
    };

    // 1. Interactive Capture Mode
    if IS_CAPTURING.load(Ordering::Relaxed) {
        if is_press {
            if effective_vk == 0x1B && !is_key_down(0x11) && !is_key_down(0x12) && !is_key_down(0x10) && !is_key_down(0x5B) && !is_key_down(0x5C) {
                if let Ok(mut keys) = CAPTURED_KEYS.try_lock() {
                    keys.clear();
                }
                IS_CAPTURING.store(false, Ordering::SeqCst);
                return;
            }
            if let Ok(mut keys) = CAPTURED_KEYS.try_lock() {
                if !keys.contains(&effective_vk) {
                    keys.push(effective_vk);
                }
            }
        } else {
            let captured_vks = if let Ok(mut keys) = CAPTURED_KEYS.try_lock() {
                let res = keys.clone();
                keys.clear();
                res
            } else {
                Vec::new()
            };
            IS_CAPTURING.store(false, Ordering::SeqCst);

            if !captured_vks.is_empty() {
                let mut has_ctrl = false;
                let mut has_alt = false;
                let mut has_shift = false;
                let mut has_win = false;
                let mut others = Vec::new();

                for &k in &captured_vks {
                    match k {
                        0x11 | 0xA2 | 0xA3 => has_ctrl = true,
                        0x12 | 0xA4 | 0xA5 => {
                            if captured_vks.len() == 1 && (k == 0xA5 || k == 0x12) {
                                others.push(vk_to_name(k));
                            } else {
                                has_alt = true;
                            }
                        }
                        0x10 | 0xA0 | 0xA1 => has_shift = true,
                        0x5B | 0x5C => has_win = true,
                        other_vk => {
                            let name = vk_to_name(other_vk);
                            if !others.contains(&name) {
                                others.push(name);
                            }
                        }
                    }
                }

                let mut parts = Vec::new();
                if has_ctrl { parts.push("Ctrl".to_string()); }
                if has_alt { parts.push("Option".to_string()); }
                if has_shift { parts.push("Shift".to_string()); }
                if has_win { parts.push("Cmd".to_string()); }
                for name in others {
                    if !parts.contains(&name) {
                        parts.push(name);
                    }
                }

                if parts.is_empty() {
                    parts.push(vk_to_name(effective_vk));
                }

                let combo_name = parts.join("+");
                let key_name = parts.last().cloned().unwrap_or_else(|| vk_to_name(effective_vk));
                let mode = if parts.len() > 1 { "combo" } else { "single" };

                send_event(HookEvent::Captured {
                    key_name,
                    combo_name,
                    mode: mode.to_string(),
                });
            }
        }
        return;
    }

    // 2. Push-to-Talk Handling
    let ptt_keys = if let Ok(guard) = PTT_KEYS.try_read() {
        guard.clone()
    } else {
        Vec::new()
    };

    if !ptt_keys.is_empty() {
        let is_match = ptt_keys.iter().any(|&k| vk_matches(effective_vk, k));
        if is_match {
            if is_press {
                let all_down = ptt_keys.iter().all(|&k| {
                    if vk_matches(effective_vk, k) {
                        true
                    } else {
                        is_key_down(k)
                    }
                });
                if all_down {
                    if !IS_PTT_PRESSED.swap(true, Ordering::SeqCst) {
                        crate::log_status("macOS PTT Press -> sending HookEvent::PttPress");
                        send_event(HookEvent::PttPress);
                    }
                }
            } else {
                let was_pressed = IS_PTT_PRESSED.swap(false, Ordering::SeqCst);
                if was_pressed {
                    crate::log_status("macOS PTT Release -> sending HookEvent::PttRelease");
                    send_event(HookEvent::PttRelease);
                }
            }
        }
    }

    // 3. Toggle Recording Handling
    let toggle_keys = if let Ok(guard) = TOGGLE_KEYS.try_read() {
        guard.clone()
    } else {
        Vec::new()
    };

    if !toggle_keys.is_empty() {
        let is_match = toggle_keys.iter().any(|&k| vk_matches(effective_vk, k));
        if is_match {
            if is_press {
                let all_down = toggle_keys.iter().all(|&k| {
                    if vk_matches(effective_vk, k) {
                        true
                    } else {
                        is_key_down(k)
                    }
                });
                if all_down {
                    let now = std::time::Instant::now();
                    let mut can_toggle = false;
                    if !IS_TOGGLE_KEY_DOWN.swap(true, Ordering::SeqCst) {
                        if let Ok(mut last_t) = LAST_TOGGLE_TIME.try_lock() {
                            if last_t.map(|t| now.duration_since(t).as_millis() > 300).unwrap_or(true) {
                                *last_t = Some(now);
                                can_toggle = true;
                            }
                        }
                    }
                    if can_toggle {
                        crate::log_status("macOS Toggle Press -> sending HookEvent::TogglePress");
                        send_event(HookEvent::TogglePress);
                    }
                }
            } else {
                let any_down = toggle_keys.iter().any(|&k| {
                    if vk_matches(effective_vk, k) {
                        false
                    } else {
                        is_key_down(k)
                    }
                });
                if !any_down {
                    IS_TOGGLE_KEY_DOWN.store(false, Ordering::SeqCst);
                }
            }
        }
    }

    // 4. Show Widget Handling
    let widget_keys = if let Ok(guard) = SHOW_WIDGET_KEYS.try_read() {
        guard.clone()
    } else {
        Vec::new()
    };

    if !widget_keys.is_empty() {
        let is_match = widget_keys.iter().any(|&k| vk_matches(effective_vk, k));
        if is_match {
            if is_press {
                let all_down = widget_keys.iter().all(|&k| {
                    if vk_matches(effective_vk, k) {
                        true
                    } else {
                        is_key_down(k)
                    }
                });
                if all_down {
                    let now = std::time::Instant::now();
                    let mut can_show = false;
                    if !IS_WIDGET_KEY_DOWN.swap(true, Ordering::SeqCst) {
                        if let Ok(mut last_t) = LAST_WIDGET_TIME.try_lock() {
                            if last_t.map(|t| now.duration_since(t).as_millis() > 300).unwrap_or(true) {
                                *last_t = Some(now);
                                can_show = true;
                            }
                        }
                    }
                    if can_show {
                        crate::log_status("macOS Show Widget -> sending HookEvent::ShowWidget");
                        send_event(HookEvent::ShowWidget);
                    }
                }
            } else {
                let any_down = widget_keys.iter().any(|&k| {
                    if vk_matches(effective_vk, k) {
                        false
                    } else {
                        is_key_down(k)
                    }
                });
                if !any_down {
                    IS_WIDGET_KEY_DOWN.store(false, Ordering::SeqCst);
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_event_tap_callback(
    _proxy: macos_tap::CGEventTapProxy,
    event_type: u32,
    event: macos_tap::CGEventRef,
    _user_info: *mut std::ffi::c_void,
) -> macos_tap::CGEventRef {
    if event.is_null() {
        return event;
    }

    let raw_keycode = macos_tap::CGEventGetIntegerValueField(event, macos_tap::K_CG_KEYBOARD_EVENT_KEYCODE) as u32;
    let effective_vk = macos_keycode_to_vk(raw_keycode);

    match event_type {
        macos_tap::K_CG_EVENT_KEY_DOWN => {
            dispatch_macos_key(effective_vk, true);
        }
        macos_tap::K_CG_EVENT_KEY_UP => {
            dispatch_macos_key(effective_vk, false);
        }
        macos_tap::K_CG_EVENT_FLAGS_CHANGED => {
            let flags = macos_tap::CGEventGetFlags(event);
            let is_down = match effective_vk {
                0xA4 | 0xA5 | 0x12 => (flags & macos_tap::K_CG_EVENT_FLAG_MASK_ALTERNATE) != 0,
                0x5B | 0x5C => (flags & macos_tap::K_CG_EVENT_FLAG_MASK_COMMAND) != 0,
                0xA2 | 0xA3 | 0x11 => (flags & macos_tap::K_CG_EVENT_FLAG_MASK_CONTROL) != 0,
                0xA0 | 0xA1 | 0x10 => (flags & macos_tap::K_CG_EVENT_FLAG_MASK_SHIFT) != 0,
                0x14 => (flags & macos_tap::K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT) != 0,
                0xFF => (flags & macos_tap::K_CG_EVENT_FLAG_MASK_SECONDARY_FN) != 0,
                _ => false,
            };
            dispatch_macos_key(effective_vk, is_down);
        }
        _ => {}
    }

    event
}

pub struct HotkeyListener {
    running: Arc<AtomicBool>,
}

impl HotkeyListener {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_ptt_key(&self, key_name: &str) {
        let keys = parse_key_combination(key_name);
        if let Ok(mut guard) = PTT_KEYS.write() {
            *guard = keys;
        }
    }

    pub fn set_toggle_key(&self, key_name: Option<&str>) {
        let keys = key_name.map(parse_key_combination).unwrap_or_default();
        if let Ok(mut guard) = TOGGLE_KEYS.write() {
            *guard = keys;
        }
    }

    pub fn set_show_widget_key(&self, key_name: &str) {
        let keys = parse_key_combination(key_name);
        if let Ok(mut guard) = SHOW_WIDGET_KEYS.write() {
            *guard = keys;
        }
    }

    pub fn start_capture<F>(&self, mode: &str, on_captured: F)
    where
        F: Fn(String, String, String) + Send + Sync + 'static,
    {
        {
            let mut m = CAPTURE_MODE.lock().unwrap();
            *m = Some(mode.to_string());
        }
        if let Ok(mut keys) = CAPTURED_KEYS.lock() {
            keys.clear();
        }
        unsafe {
            KEY_CALLBACK_CAPTURED = Some(Box::new(on_captured));
        }
        IS_CAPTURING.store(true, Ordering::SeqCst);
    }

    pub fn cancel_capture(&self) {
        if let Ok(mut keys) = CAPTURED_KEYS.lock() {
            keys.clear();
        }
        IS_CAPTURING.store(false, Ordering::SeqCst);
        unsafe {
            KEY_CALLBACK_CAPTURED = None;
        }
    }

    pub fn start<FP, FR, FT, FW>(
        &self,
        target_key: &str,
        toggle_key: Option<&str>,
        show_widget_key: Option<&str>,
        on_press: FP,
        on_release: FR,
        on_toggle: FT,
        on_show_widget: FW,
    ) where
        FP: Fn() + Send + Sync + 'static,
        FR: Fn() + Send + Sync + 'static,
        FT: Fn() + Send + Sync + 'static,
        FW: Fn() + Send + Sync + 'static,
    {
        self.set_ptt_key(target_key);
        self.set_toggle_key(toggle_key);
        self.set_show_widget_key(show_widget_key.unwrap_or("Ctrl+Shift+Space"));

        unsafe {
            KEY_CALLBACK_PRESS = Some(Box::new(on_press));
            KEY_CALLBACK_RELEASE = Some(Box::new(on_release));
            KEY_CALLBACK_TOGGLE = Some(Box::new(on_toggle));
            KEY_CALLBACK_SHOW_WIDGET = Some(Box::new(on_show_widget));
        }

        // Set up communication channel for off-hook execution
        let (tx, rx) = std::sync::mpsc::channel::<HookEvent>();
        if let Ok(mut guard) = EVENT_SENDER.lock() {
            *guard = Some(tx);
        }

        // Spawn dedicated worker thread to process hotkey actions off the OS hook thread
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                match event {
                    HookEvent::PttPress => {
                        unsafe {
                            if let Some(cb) = &*std::ptr::addr_of!(KEY_CALLBACK_PRESS) {
                                cb();
                            }
                        }
                    }
                    HookEvent::PttRelease => {
                        unsafe {
                            if let Some(cb) = &*std::ptr::addr_of!(KEY_CALLBACK_RELEASE) {
                                cb();
                            }
                        }
                    }
                    HookEvent::TogglePress => {
                        unsafe {
                            if let Some(cb) = &*std::ptr::addr_of!(KEY_CALLBACK_TOGGLE) {
                                cb();
                            }
                        }
                    }
                    HookEvent::ShowWidget => {
                        unsafe {
                            if let Some(cb) = &*std::ptr::addr_of!(KEY_CALLBACK_SHOW_WIDGET) {
                                cb();
                            }
                        }
                    }
                    HookEvent::Captured { key_name, combo_name, mode } => {
                        unsafe {
                            if let Some(cb) = &*std::ptr::addr_of!(KEY_CALLBACK_CAPTURED) {
                                cb(key_name, combo_name, mode);
                            }
                        }
                    }
                }
            }
        });

        let running = Arc::clone(&self.running);
        running.store(true, Ordering::SeqCst);

        #[cfg(windows)]
        std::thread::spawn(move || unsafe {
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(hook_proc),
                HINSTANCE(std::ptr::null_mut()),
                0,
            );
            match hook {
                Ok(h) => {
                    crate::log_status(&format!("SetWindowsHookExW succeeded (handle={:?})", h.0));
                    HOOK_HANDLE = Some(h);
                    let mut msg = MSG::default();
                    while running.load(Ordering::Relaxed) && GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() {
                        DispatchMessageW(&msg);
                    }
                    if let Some(h) = HOOK_HANDLE.take() {
                        let _ = UnhookWindowsHookEx(h);
                    }
                    crate::log_status("Keyboard hook message loop terminated");
                }
                Err(e) => {
                    crate::log_status(&format!("SetWindowsHookExW FAILED with error: {:?}", e));
                }
            }
        });

        #[cfg(target_os = "macos")]
        {
            crate::log_status("Starting macOS native CGEventTap hotkey listener...");
            let running_thread = Arc::clone(&running);
            std::thread::spawn(move || unsafe {
                while running_thread.load(Ordering::Relaxed) {
                    let tap = macos_tap::CGEventTapCreate(
                        macos_tap::K_CG_SESSION_EVENT_TAP,
                        macos_tap::K_CG_HEAD_INSERT_EVENT_TAP,
                        macos_tap::K_CG_EVENT_TAP_OPTION_LISTEN_ONLY,
                        (1u64 << macos_tap::K_CG_EVENT_KEY_DOWN)
                            | (1u64 << macos_tap::K_CG_EVENT_KEY_UP)
                            | (1u64 << macos_tap::K_CG_EVENT_FLAGS_CHANGED),
                        macos_event_tap_callback,
                        std::ptr::null_mut(),
                    );

                    if tap.is_null() {
                        crate::log_status("CGEventTap waiting for macOS Accessibility permission...");
                        std::thread::sleep(std::time::Duration::from_millis(1500));
                        continue;
                    }

                    crate::log_status("CGEventTap created successfully! Attaching to RunLoop...");
                    let source = macos_tap::CFMachPortCreateRunLoopSource(
                        std::ptr::null(),
                        tap,
                        0,
                    );
                    if source.is_null() {
                        crate::log_status("CFMachPortCreateRunLoopSource failed");
                        macos_tap::CFRelease(tap);
                        std::thread::sleep(std::time::Duration::from_millis(1500));
                        continue;
                    }

                    let rl = macos_tap::CFRunLoopGetCurrent();
                    MACOS_RUN_LOOP = Some(rl);
                    macos_tap::CFRunLoopAddSource(rl, source, macos_tap::kCFRunLoopCommonModes);
                    macos_tap::CGEventTapEnable(tap, true);
                    crate::log_status("macOS native CGEventTap event loop running.");
                    macos_tap::CFRunLoopRun();

                    macos_tap::CFRelease(source);
                    macos_tap::CFRelease(tap);
                    MACOS_RUN_LOOP = None;
                    break;
                }
            });
        }

        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = running;
            crate::log_status("Hotkey listener stub for other non-windows OS");
        }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        #[cfg(windows)]
        unsafe {
            if let Some(h) = HOOK_HANDLE.take() {
                let _ = UnhookWindowsHookEx(h);
            }
        }
        #[cfg(target_os = "macos")]
        unsafe {
            if let Some(rl) = MACOS_RUN_LOOP.take() {
                macos_tap::CFRunLoopStop(rl);
            }
        }
    }
}
