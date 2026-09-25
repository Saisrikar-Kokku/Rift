use tauri::WebviewWindow;

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongW, SetWindowLongW, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOPMOST,
    SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

#[cfg(windows)]
pub fn setup_widget_window(window: &WebviewWindow) {
    if let Ok(hwnd_ptr) = window.hwnd() {
        let hwnd = HWND(hwnd_ptr.0 as *mut _);

        unsafe {
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            // Ensure WS_EX_NOACTIVATE is stripped so Windows drag loops and input work seamlessly.
            // Apply WS_EX_TOOLWINDOW (prevent taskbar/alt-tab) and WS_EX_TOPMOST (stay on top).
            let new_style = (ex_style & !(WS_EX_NOACTIVATE.0 as i32))
                | (WS_EX_TOOLWINDOW.0 as i32)
                | (WS_EX_TOPMOST.0 as i32);

            SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);

            // Ensure HWND_TOPMOST, apply frame styles, and show window without stealing focus
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
            );

            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
    }
}

#[cfg(not(windows))]
pub fn setup_widget_window(window: &WebviewWindow) {
    let _ = window.set_always_on_top(true);
    let _ = window.show();
}

