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
fn rdev_key_to_vk(key: rdev::Key) -> Option<u32> {
    match key {
        rdev::Key::Alt => Some(0xA4),       // Left Option
        rdev::Key::AltGr => Some(0xA5),     // Right Option
        rdev::Key::ControlLeft => Some(0xA2),
        rdev::Key::ControlRight => Some(0xA3),
        rdev::Key::ShiftLeft => Some(0xA0),
        rdev::Key::ShiftRight => Some(0xA1),
        rdev::Key::MetaLeft => Some(0x5B),  // Left Command
        rdev::Key::MetaRight => Some(0x5C), // Right Command
        rdev::Key::Space => Some(0x20),
        rdev::Key::Tab => Some(0x09),
        rdev::Key::Return | rdev::Key::KpReturn => Some(0x0D),
        rdev::Key::Backspace => Some(0x08),
        rdev::Key::Escape => Some(0x1B),
        rdev::Key::CapsLock => Some(0x14),
        rdev::Key::Delete => Some(0x2E),
        rdev::Key::Home => Some(0x24),
        rdev::Key::End => Some(0x23),
        rdev::Key::PageUp => Some(0x21),
        rdev::Key::PageDown => Some(0x22),
        rdev::Key::UpArrow => Some(0x26),
        rdev::Key::DownArrow => Some(0x28),
        rdev::Key::LeftArrow => Some(0x25),
        rdev::Key::RightArrow => Some(0x27),
        rdev::Key::Function => Some(0xFF),
        rdev::Key::F1 => Some(0x70),
        rdev::Key::F2 => Some(0x71),
        rdev::Key::F3 => Some(0x72),
        rdev::Key::F4 => Some(0x73),
        rdev::Key::F5 => Some(0x74),
        rdev::Key::F6 => Some(0x75),
        rdev::Key::F7 => Some(0x76),
        rdev::Key::F8 => Some(0x77),
        rdev::Key::F9 => Some(0x78),
        rdev::Key::F10 => Some(0x79),
        rdev::Key::F11 => Some(0x7A),
        rdev::Key::F12 => Some(0x7B),
        rdev::Key::KeyA => Some(0x41),
        rdev::Key::KeyB => Some(0x42),
        rdev::Key::KeyC => Some(0x43),
        rdev::Key::KeyD => Some(0x44),
        rdev::Key::KeyE => Some(0x45),
        rdev::Key::KeyF => Some(0x46),
        rdev::Key::KeyG => Some(0x47),
        rdev::Key::KeyH => Some(0x48),
        rdev::Key::KeyI => Some(0x49),
        rdev::Key::KeyJ => Some(0x4A),
        rdev::Key::KeyK => Some(0x4B),
        rdev::Key::KeyL => Some(0x4C),
        rdev::Key::KeyM => Some(0x4D),
        rdev::Key::KeyN => Some(0x4E),
        rdev::Key::KeyO => Some(0x4F),
        rdev::Key::KeyP => Some(0x50),
        rdev::Key::KeyQ => Some(0x51),
        rdev::Key::KeyR => Some(0x52),
        rdev::Key::KeyS => Some(0x53),
        rdev::Key::KeyT => Some(0x54),
        rdev::Key::KeyU => Some(0x55),
        rdev::Key::KeyV => Some(0x56),
        rdev::Key::KeyW => Some(0x57),
        rdev::Key::KeyX => Some(0x58),
        rdev::Key::KeyY => Some(0x59),
        rdev::Key::KeyZ => Some(0x5A),
        rdev::Key::Num0 | rdev::Key::Kp0 => Some(0x30),
        rdev::Key::Num1 | rdev::Key::Kp1 => Some(0x31),
        rdev::Key::Num2 | rdev::Key::Kp2 => Some(0x32),
        rdev::Key::Num3 | rdev::Key::Kp3 => Some(0x33),
        rdev::Key::Num4 | rdev::Key::Kp4 => Some(0x34),
        rdev::Key::Num5 | rdev::Key::Kp5 => Some(0x35),
        rdev::Key::Num6 | rdev::Key::Kp6 => Some(0x36),
        rdev::Key::Num7 | rdev::Key::Kp7 => Some(0x37),
        rdev::Key::Num8 | rdev::Key::Kp8 => Some(0x38),
        rdev::Key::Num9 | rdev::Key::Kp9 => Some(0x39),
        rdev::Key::BackQuote => Some(0xC0),
        rdev::Key::Minus | rdev::Key::KpMinus => Some(0xBD),
        rdev::Key::Equal => Some(0xBB),
        rdev::Key::LeftBracket => Some(0xDB),
        rdev::Key::RightBracket => Some(0xDD),
        rdev::Key::BackSlash | rdev::Key::IntlBackslash => Some(0xDC),
        rdev::Key::SemiColon => Some(0xBA),
        rdev::Key::Quote => Some(0xDE),
        rdev::Key::Comma => Some(0xBC),
        rdev::Key::Dot => Some(0xBE),
        rdev::Key::Slash | rdev::Key::KpDivide => Some(0xBF),
        rdev::Key::KpPlus => Some(0x6B),
        rdev::Key::KpMultiply => Some(0x6A),
        rdev::Key::KpDelete => Some(0x2E),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn handle_macos_event(event: rdev::Event) {
    let (is_press, key) = match event.event_type {
        rdev::EventType::KeyPress(k) => (true, k),
        rdev::EventType::KeyRelease(k) => (false, k),
        _ => return,
    };

    let effective_vk = match rdev_key_to_vk(key) {
        Some(vk) => vk,
        None => return,
    };

    // Update active keys set
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
                if has_alt { parts.push("Alt".to_string()); }
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
            crate::log_status("Starting macOS rdev hotkey listener...");
            std::thread::spawn(move || {
                if let Err(error) = rdev::listen(move |event| {
                    if !running.load(Ordering::Relaxed) {
                        return;
                    }
                    handle_macos_event(event);
                }) {
                    crate::log_status(&format!("rdev::listen error (check macOS Accessibility permissions): {:?}", error));
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
    }
}
