#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextW, GetWindowThreadProcessId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppDomain {
    Coding,
    Terminal,
    Messaging,
    Document,
    General,
}

#[derive(Debug, Clone)]
pub struct WindowContext {
    pub process_name: String,
    pub window_title: String,
    pub domain: AppDomain,
}

impl Default for WindowContext {
    fn default() -> Self {
        Self {
            process_name: String::new(),
            window_title: String::new(),
            domain: AppDomain::General,
        }
    }
}

impl WindowContext {
    pub fn get_system_prompt_addon(&self) -> Option<String> {
        match self.domain {
            AppDomain::Coding => {
                Some(format!(
                    "CONTEXT: The user is dictating into a code editor ({}, active context: '{}'). \
Format technical identifiers, variables, types, and functions in standard programming casing (camelCase, snake_case, or PascalCase depending on context). \
Convert spoken programming operators and symbols directly into code syntax (e.g. 'arrow' -> '=>', 'double equals' -> '==', 'not equals' -> '!=', 'dot' -> '.', 'curly braces' -> '{{}}', 'parentheses' -> '()', 'colon colon' -> '::'). \
Keep keywords concise and omit conversational filler.",
                    self.process_name, self.window_title
                ))
            }
            AppDomain::Terminal => {
                Some(format!(
                    "CONTEXT: The user is dictating into a terminal/command prompt ({}, window: '{}'). \
Format output as exact CLI commands, arguments, shell flags (e.g. '--help', '-rf'), file paths, or git syntax. \
Do NOT capitalize command names and do NOT add trailing periods to commands.",
                    self.process_name, self.window_title
                ))
            }
            AppDomain::Messaging => {
                Some(format!(
                    "CONTEXT: The user is dictating in a chat/messaging application ({}, channel/chat: '{}'). \
Keep the tone natural, clear, conversational, and concise with light, friendly punctuation.",
                    self.process_name, self.window_title
                ))
            }
            AppDomain::Document => {
                Some(format!(
                    "CONTEXT: The user is dictating into a document/email ({}, title: '{}'). \
Format into polished, professional prose with clean paragraph breaks and appropriate business punctuation.",
                    self.process_name, self.window_title
                ))
            }
            AppDomain::General => None,
        }
    }

    pub fn get_domain_vocabulary_hints(&self) -> Vec<String> {
        match self.domain {
            AppDomain::Coding => vec![
                "async".into(), "await".into(), "fn".into(), "function".into(),
                "const".into(), "let".into(), "var".into(), "struct".into(),
                "enum".into(), "impl".into(), "return".into(), "import".into(),
                "export".into(), "string".into(), "boolean".into(), "usize".into(),
                "null".into(), "None".into(), "Some".into(), "Result".into(),
                "Option".into(), "JSON".into(), "API".into(), "HTTP".into(),
                "Tauri".into(), "Rust".into(), "TypeScript".into(), "Python".into(),
            ],
            AppDomain::Terminal => vec![
                "git".into(), "commit".into(), "push".into(), "pull".into(),
                "checkout".into(), "status".into(), "branch".into(), "merge".into(),
                "cargo".into(), "run".into(), "build".into(), "test".into(),
                "npm".into(), "pnpm".into(), "yarn".into(), "install".into(),
                "docker".into(), "kubectl".into(), "cd".into(), "ls".into(),
                "grep".into(), "cat".into(), "curl".into(), "ssh".into(),
            ],
            _ => Vec::new(),
        }
    }
}

#[cfg(windows)]
fn get_process_name(pid: u32) -> String {
    // 1. Ultra-fast direct Win32 process query (<0.05ms single syscall)
    unsafe {
        use windows::Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        use windows::Win32::Foundation::CloseHandle;

        if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut buf = [0u16; 1024];
            let mut size = buf.len() as u32;
            let res = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_FORMAT(0),
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut size,
            );
            let _ = CloseHandle(handle);
            if res.is_ok() && size > 0 {
                let full_path = String::from_utf16_lossy(&buf[..size as usize]);
                if let Some(file_name) = std::path::Path::new(&full_path).file_name().and_then(|f| f.to_str()) {
                    let clean = file_name.trim().to_lowercase();
                    if !clean.is_empty() {
                        return clean;
                    }
                }
            }
        }
    }

    // 2. Resilient fallback: ToolHelp32 process snapshot if OpenProcess permission is denied
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    unsafe {
        if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    if entry.th32ProcessID == pid {
                        let name = String::from_utf16_lossy(&entry.szExeFile);
                        let clean = name.trim_matches('\0').trim().to_lowercase();
                        let _ = windows::Win32::Foundation::CloseHandle(snap);
                        return clean;
                    }
                    if Process32NextW(snap, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = windows::Win32::Foundation::CloseHandle(snap);
        }
    }
    String::new()
}

fn classify_domain(proc_name: &str, title: &str) -> AppDomain {
    let p = proc_name.to_lowercase();
    let t = title.to_lowercase();

    // Coding editors & IDEs
    if p == "code.exe"
        || p == "cursor.exe"
        || p == "devenv.exe"
        || p == "idea64.exe"
        || p == "pycharm64.exe"
        || p == "webstorm64.exe"
        || p == "clion64.exe"
        || p == "goland64.exe"
        || p == "rider64.exe"
        || p == "rustrover64.exe"
        || p == "sublime_text.exe"
        || p == "notepad++.exe"
        || p == "zed.exe"
        || p == "neovide.exe"
        || t.contains("visual studio code")
        || t.contains(" - visual studio")
    {
        return AppDomain::Coding;
    }

    // Terminals & Shells
    if p == "windowsterminal.exe"
        || p == "powershell.exe"
        || p == "pwsh.exe"
        || p == "cmd.exe"
        || p == "conhost.exe"
        || p == "mintty.exe"
        || p == "wezterm-gui.exe"
        || p == "alacritty.exe"
        || p == "wt.exe"
        || p == "bash.exe"
        || t.contains("powershell")
        || t.contains("command prompt")
    {
        return AppDomain::Terminal;
    }

    // Messaging & Collaboration
    if p == "slack.exe"
        || p == "teams.exe"
        || p == "ms-teams.exe"
        || p == "discord.exe"
        || p == "telegram.exe"
        || p == "whatsapp.exe"
        || p == "signal.exe"
        || p == "skype.exe"
        || t.contains("slack")
        || t.contains("discord")
        || t.contains("whatsapp")
        || t.contains("teams.microsoft.com")
    {
        return AppDomain::Messaging;
    }

    // Documents & Email
    if p == "outlook.exe"
        || p == "winword.exe"
        || p == "excel.exe"
        || p == "powerpnt.exe"
        || p == "notion.exe"
        || p == "onenote.exe"
        || p == "obsidian.exe"
        || t.contains("outlook")
        || t.contains("notion")
        || t.contains("google docs")
        || t.contains("word |")
        || t.contains(" - word")
        || t.contains("microsoft word")
    {
        return AppDomain::Document;
    }

    AppDomain::General
}

#[cfg(windows)]
pub fn get_window_context(hwnd_val: Option<isize>) -> WindowContext {
    let Some(raw_hwnd) = hwnd_val else {
        return WindowContext::default();
    };
    if raw_hwnd == 0 {
        return WindowContext::default();
    }

    let hwnd = HWND(raw_hwnd as *mut _);
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        return WindowContext::default();
    }

    // 1. Window title
    let mut title_buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut title_buf) };
    let window_title = if len > 0 {
        String::from_utf16_lossy(&title_buf[..len as usize]).trim().to_string()
    } else {
        String::new()
    };

    // 2. Process name
    let proc_name = get_process_name(pid);

    let domain = classify_domain(&proc_name, &window_title);
    WindowContext {
        process_name: proc_name,
        window_title,
        domain,
    }
}

#[cfg(not(windows))]
pub fn get_window_context(_hwnd_val: Option<isize>) -> WindowContext {
    WindowContext::default()
}
