use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub push_to_talk_key: String,
    pub push_to_talk_mode: String, // "single" or "combo"
    pub push_to_talk_combo: String,
    pub toggle_recording_key: Option<String>,
    pub microphone_device_id: Option<String>,
    pub widget_visibility: String, // "always", "recording", "hidden"
    pub widget_style: String,      // "active_waveform"
    pub follow_cursor_across_monitors: bool,
    pub lock_widget_position: bool,
    pub widget_custom_position: Option<WidgetPosition>,
    pub copy_instead_of_type: bool,
    pub launch_on_startup: bool,
    pub launch_at_login: bool,
    pub transcription_mode: String, // "cloud"
    pub stt_provider: String,       // "openrouter", "groq"
    pub inference_model: String,    // "elevenlabs/scribe-v2"
    pub local_inference_profile: String,
    pub local_whisper_unload_on_idle: bool,
    pub recording_sounds: bool,
    pub sound_volume: f32,
    pub pause_other_audio_while_talking: bool,
    pub auto_enhance_prompt: bool,
    pub context_aware_formatting: bool,
    pub formatting_instructions: String,
    pub theme: String,
    pub onboarding_completed: bool,
    pub mic_boost: String, // "off", "2x", "4x", "6x", "8x"
    pub voice_commands_enabled: bool,
    pub voice_snippets_enabled: bool,
    pub quick_actions_enabled: bool,
    pub clipboard_history_enabled: bool,
    pub clipboard_max_items: usize,
    pub tenglish_mode_enabled: bool,
    pub tenglish_stt_tier: String,
    pub tenglish_translit_engine: String,
    pub noise_cancellation_enabled: bool,
    pub ambient_memory_enabled: bool,
    pub whisper_mode_enabled: bool,
    pub intent_router_enabled: bool,
    pub wake_word_enabled: bool,
    pub widget_transcribing_style: String, // "orbit_pure", "orbit_badge", "harmonic_wave", "infinity_pulse"
}


impl AppSettings {
    pub fn mic_boost_multiplier(&self) -> f32 {
        let clean = self.mic_boost.trim().to_lowercase();
        let num_part = clean.trim_end_matches('x').trim();
        if let Ok(val) = num_part.parse::<f32>() {
            if val >= 1.0 && val <= 16.0 {
                return val;
            }
        }
        match clean.as_str() {
            "2x" => 2.0,
            "4x" => 4.0,
            "6x" => 6.0,
            "8x" => 8.0,
            _ => 1.0,
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            push_to_talk_key: "right alt".to_string(),
            push_to_talk_mode: "single".to_string(),
            push_to_talk_combo: "ctrl+win".to_string(),
            toggle_recording_key: None,
            microphone_device_id: None,
            widget_visibility: "always".to_string(),
            widget_style: "pill_text".to_string(),
            follow_cursor_across_monitors: true,
            lock_widget_position: false,
            widget_custom_position: None,
            copy_instead_of_type: false,
            launch_on_startup: true,
            launch_at_login: true,
            transcription_mode: "cloud".to_string(),
            stt_provider: "groq".to_string(),
            inference_model: "whisper-large-v3-turbo".to_string(),
            local_inference_profile: "balanced".to_string(),
            local_whisper_unload_on_idle: true,
            recording_sounds: true,
            sound_volume: 0.6,
            pause_other_audio_while_talking: true,
            auto_enhance_prompt: false,
            context_aware_formatting: true,
            formatting_instructions: "".to_string(),
            theme: "dark".to_string(),
            onboarding_completed: false,
            mic_boost: "off".to_string(),
            voice_commands_enabled: true,
            voice_snippets_enabled: true,
            quick_actions_enabled: true,
            clipboard_history_enabled: true,
            clipboard_max_items: 50,
            tenglish_mode_enabled: false,
            tenglish_stt_tier: "auto".to_string(),
            tenglish_translit_engine: "auto".to_string(),
            noise_cancellation_enabled: true,
            ambient_memory_enabled: false,
            whisper_mode_enabled: false,
            intent_router_enabled: true,
            wake_word_enabled: false,
            widget_transcribing_style: "orbit_pure".to_string(),
        }
    }
}



pub fn get_app_data_dir() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));

    #[cfg(target_os = "macos")]
    let base = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
        .unwrap_or_else(|_| PathBuf::from("."));

    #[cfg(all(not(windows), not(target_os = "macos")))]
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .unwrap_or_else(|_| PathBuf::from("."));

    let path = base.join("Rift");
    let _ = fs::create_dir_all(&path);
    path
}

pub fn get_settings_path() -> PathBuf {
    get_app_data_dir().join("settings.json")
}

static CACHED_SETTINGS: std::sync::RwLock<Option<AppSettings>> = std::sync::RwLock::new(None);
static SETTINGS_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn load_settings() -> AppSettings {
    if let Ok(guard) = CACHED_SETTINGS.read() {
        if let Some(ref cached) = *guard {
            return cached.clone();
        }
    }

    let settings = load_settings_from_disk();
    if let Ok(mut guard) = CACHED_SETTINGS.write() {
        *guard = Some(settings.clone());
    }
    settings
}

fn load_settings_from_disk() -> AppSettings {
    let path = get_settings_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(mut settings) = serde_json::from_str::<AppSettings>(&content) {
                let mut modified = false;
                if settings.widget_style == "active_waveform"
                    || settings.widget_style == "minimal_dot"
                    || settings.widget_style == "key_dock"
                    || settings.widget_style.is_empty()
                {
                    settings.widget_style = "pill_text".to_string();
                    modified = true;
                }
                if settings.stt_provider.is_empty() {
                    settings.stt_provider = "groq".to_string();
                    modified = true;
                }
                if settings.inference_model.is_empty() || settings.inference_model.contains("mai-transcribe") {
                    if settings.stt_provider == "groq" {
                        settings.inference_model = "whisper-large-v3-turbo".to_string();
                    } else {
                        settings.inference_model = "elevenlabs/scribe-v2".to_string();
                    }
                    modified = true;
                }
                if settings.tenglish_stt_tier.contains("mai") {
                    settings.tenglish_stt_tier = "auto".to_string();
                    modified = true;
                }
                if settings.widget_transcribing_style.is_empty() {
                    settings.widget_transcribing_style = "orbit_pure".to_string();
                    modified = true;
                }
                if modified {
                    save_settings(&settings);
                }
                return settings;
            }
        }
    }

    let default_settings = AppSettings::default();
    save_settings(&default_settings);
    default_settings
}

pub fn save_settings(settings: &AppSettings) {
    if let Err(error) = try_save_settings(settings) {
        log::error!("Failed to persist Rift settings: {error}");
    }
}

/// Persist settings before publishing them to the in-memory cache.
/// A unique temporary file avoids collisions when multiple settings updates happen close together.
pub fn try_save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_path();
    let sequence = SETTINGS_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = path.with_file_name(format!(
        "settings.json.tmp-{}-{sequence}",
        std::process::id()
    ));
    let json = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Could not serialize settings: {error}"))?;

    fs::write(&temp_path, json)
        .map_err(|error| format!("Could not write temporary settings file: {error}"))?;
    if let Err(error) = fs::rename(&temp_path, &path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("Could not replace settings file: {error}"));
    }

    if let Ok(mut guard) = CACHED_SETTINGS.write() {
        *guard = Some(settings.clone());
    }
    Ok(())
}

pub fn append_scratchpad_inbox_note(note: &str) {
    let inbox_path = get_app_data_dir().join("scratchpad_inbox.json");
    let mut notes: Vec<String> = if inbox_path.exists() {
        fs::read_to_string(&inbox_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    notes.push(note.to_string());
    if let Ok(json) = serde_json::to_string_pretty(&notes) {
        let _ = fs::write(&inbox_path, json);
    }
}

pub fn drain_scratchpad_inbox_notes() -> Vec<String> {
    let inbox_path = get_app_data_dir().join("scratchpad_inbox.json");
    if !inbox_path.exists() {
        return Vec::new();
    }
    let notes: Vec<String> = fs::read_to_string(&inbox_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let _ = fs::remove_file(&inbox_path);
    notes
}
