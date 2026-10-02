use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex};
use crate::audio::capture::AudioRecorder;
use crate::hotkey::listener::HotkeyListener;
use crate::stt::groq::GroqClient;
use crate::stt::openrouter::OpenRouterClient;
use crate::storage::clipboard::ClipboardStore;
use crate::storage::history::HistoryStore;
use crate::storage::dictionary::DictionaryStore;
use crate::storage::snippets::SnippetStore;

pub struct AppState {
    pub recorder: Mutex<AudioRecorder>,
    pub hotkey: HotkeyListener,
    pub groq: GroqClient,
    pub openrouter: OpenRouterClient,
    pub history: HistoryStore,
    pub dictionary: DictionaryStore,
    pub snippets: SnippetStore,
    pub clipboard: Arc<ClipboardStore>,
    pub is_internal_pasting: Arc<AtomicBool>,
    pub target_window: Mutex<Option<isize>>,
    pub whisper_requests_today: AtomicU32,
    pub is_toggle_recording: AtomicBool,
    pub last_recording: Mutex<Option<(Vec<u8>, f64)>>,
    pub ambient_recorder: Mutex<crate::audio::ambient_memory::AmbientRecorder>,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let history = HistoryStore::new().map_err(|e| e.to_string())?;
        let dictionary = DictionaryStore::new().map_err(|e| e.to_string())?;
        let snippets = SnippetStore::new().map_err(|e| e.to_string())?;
        let clipboard = Arc::new(ClipboardStore::new().map_err(|e| e.to_string())?);
        let initial_today = history.get_dashboard_stats().map(|s| s.today_requests as u32).unwrap_or(0);
        Ok(Self {
            recorder: Mutex::new(AudioRecorder::new()),
            hotkey: HotkeyListener::new(),
            groq: GroqClient::new(),
            openrouter: OpenRouterClient::new(),
            history,
            dictionary,
            snippets,
            clipboard,
            is_internal_pasting: Arc::new(AtomicBool::new(false)),
            target_window: Mutex::new(None),
            whisper_requests_today: AtomicU32::new(initial_today),
            is_toggle_recording: AtomicBool::new(false),
            last_recording: Mutex::new(None),
            ambient_recorder: Mutex::new(crate::audio::ambient_memory::AmbientRecorder::new(3 * 60)), // 3 minutes buffer
        })
    }
}



