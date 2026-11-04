use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::Stream;
use parking_lot::Mutex;
use serde_json::Value;

use super::capture::{encode_wav, AudioRecorder, SAMPLE_RATE};
use super::level::calculate_level;
use crate::storage::credentials::CredentialsProvider;

const PRE_ROLL_SAMPLES: usize = (SAMPLE_RATE as usize * 520) / 1000; // 520ms pre-roll at 16kHz (8,320 samples)
const SILENCE_COMPLETION_MS: u64 = 1050; // 1.05s silence after speech completes dictation snappily
const POST_WAKE_WAIT_TIMEOUT_MS: u64 = 5500; // 5.5s max wait if user pauses after saying "Hey Rift"
const MAX_DICTATION_DURATION_SECS: u64 = 45;

#[derive(Debug, Clone)]
struct WakeProbeOutcome {
    matched: bool,
    has_trailing_words: bool,
    probe_samples_len: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum WakeState {
    Standby {
        onset_frames: u32,
        cooldown_until: Option<Instant>,
    },
    CapturingCandidate {
        started_at: Instant,
        last_voice_at: Instant,
        probes_sent: u8,
        probe_in_flight: bool,
        last_probe_len: usize,
        peak_rms: f64,
    },
    ActiveDictation {
        activated_at: Instant,
        last_voice_at: Instant,
        has_post_wake_speech: bool,
        peak_rms: f64,
    },
}

fn get_wake_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .tcp_nodelay(true)
            .pool_max_idle_per_host(4)
            .pool_idle_timeout(Duration::from_secs(90))
            .connect_timeout(Duration::from_millis(1500))
            .timeout(Duration::from_millis(3200))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// Pre-warms the DNS + TCP + TLS 1.3 connection to Groq so the first "Hey Rift" probe has 0ms handshake overhead.
fn prewarm_wake_http_client() {
    if let Some(groq_key) = CredentialsProvider::get_api_key() {
        let gkey = groq_key.trim().to_string();
        if !gkey.is_empty() {
            tauri::async_runtime::spawn(async move {
                let client = get_wake_http_client();
                let _ = client
                    .get("https://api.groq.com/openai/v1/models")
                    .bearer_auth(&gkey)
                    .send()
                    .await;
            });
        }
    }
}

pub struct WakeWordEngine {
    is_enabled: Arc<AtomicBool>,
    stream: Option<Stream>,
}

unsafe impl Send for WakeWordEngine {}
unsafe impl Sync for WakeWordEngine {}

impl WakeWordEngine {
    pub fn new() -> Self {
        Self {
            is_enabled: Arc::new(AtomicBool::new(false)),
            stream: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_enabled.load(Ordering::SeqCst)
    }

    pub fn stop(&mut self) {
        self.is_enabled.store(false, Ordering::SeqCst);
        let _ = self.stream.take();
    }

    pub fn start<FLevel, FActivated, FCancelled, FComplete>(
        &mut self,
        device_name: Option<&str>,
        manual_recording_flag: Arc<AtomicBool>,
        on_level: FLevel,
        on_activated: FActivated,
        on_cancelled: FCancelled,
        on_complete: FComplete,
    ) -> Result<(), String>
    where
        FLevel: Fn(u32) + Send + Sync + 'static,
        FActivated: Fn() + Send + Sync + 'static,
        FCancelled: Fn() + Send + Sync + 'static,
        FComplete: Fn(Vec<i16>) + Send + Sync + 'static,
    {
        self.stop();

        let (device, config) = AudioRecorder::resolve_device_and_config(device_name)?;
        let channels = config.channels() as usize;
        let native_rate = config.sample_rate().0;

        let is_enabled = Arc::clone(&self.is_enabled);
        is_enabled.store(true, Ordering::SeqCst);

        // Pre-warm TLS connection to Groq API immediately
        prewarm_wake_http_client();

        let incoming_16k: Arc<Mutex<VecDeque<i16>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(SAMPLE_RATE as usize * 4)));
        let incoming_writer = Arc::clone(&incoming_16k);
        let enabled_for_stream = Arc::clone(&is_enabled);
        let manual_for_stream = Arc::clone(&manual_recording_flag);

        let step = (native_rate as f64 / SAMPLE_RATE as f64).max(1.0);
        let err_fn = |err| eprintln!("[WakeWord] Audio stream error: {}", err);

        let stream = match config.sample_format() {
            cpal::SampleFormat::I16 => {
                let mut phase = 0.0f64;
                device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &_| {
                        if !enabled_for_stream.load(Ordering::Relaxed)
                            || manual_for_stream.load(Ordering::Relaxed)
                        {
                            return;
                        }
                        let mut q = incoming_writer.lock();
                        for chunk in data.chunks(channels) {
                            phase += 1.0;
                            if phase >= step {
                                phase -= step;
                                let avg = chunk.iter().map(|&s| s as i32).sum::<i32>()
                                    / channels as i32;
                                q.push_back(avg.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
                            }
                        }
                        if q.len() > SAMPLE_RATE as usize * 8 {
                            let excess = q.len() - SAMPLE_RATE as usize * 8;
                            q.drain(0..excess);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::F32 => {
                let mut phase = 0.0f64;
                device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &_| {
                        if !enabled_for_stream.load(Ordering::Relaxed)
                            || manual_for_stream.load(Ordering::Relaxed)
                        {
                            return;
                        }
                        let mut q = incoming_writer.lock();
                        for chunk in data.chunks(channels) {
                            phase += 1.0;
                            if phase >= step {
                                phase -= step;
                                let avg =
                                    chunk.iter().copied().sum::<f32>() / channels as f32;
                                let sample = (avg * 32767.0)
                                    .round()
                                    .clamp(i16::MIN as f32, i16::MAX as f32)
                                    as i16;
                                q.push_back(sample);
                            }
                        }
                        if q.len() > SAMPLE_RATE as usize * 8 {
                            let excess = q.len() - SAMPLE_RATE as usize * 8;
                            q.drain(0..excess);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            _ => return Err("Unsupported sample format for wake word listener".to_string()),
        }
        .map_err(|e| format!("Failed to build wake word stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start wake word stream: {}", e))?;

        self.stream = Some(stream);

        // Spawn the high-responsiveness (25ms) VAD + Wake-Word state machine worker thread
        std::thread::spawn(move || {
            let mut pre_roll: VecDeque<i16> = VecDeque::with_capacity(PRE_ROLL_SAMPLES + 1600);
            let mut utterance: Vec<i16> = Vec::with_capacity(SAMPLE_RATE as usize * 15);
            let mut noise_floor_rms: f64 = 85.0;
            let mut state = WakeState::Standby {
                onset_frames: 0,
                cooldown_until: None,
            };

            let probe_result: Arc<Mutex<Option<WakeProbeOutcome>>> = Arc::new(Mutex::new(None));

            while is_enabled.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(25));

                if manual_recording_flag.load(Ordering::Relaxed) {
                    incoming_16k.lock().clear();
                    pre_roll.clear();
                    utterance.clear();
                    *probe_result.lock() = None;
                    state = WakeState::Standby {
                        onset_frames: 0,
                        cooldown_until: Some(Instant::now() + Duration::from_millis(450)),
                    };
                    continue;
                }

                let new_samples: Vec<i16> = {
                    let mut q = incoming_16k.lock();
                    if q.is_empty() {
                        continue;
                    }
                    q.drain(..).collect()
                };

                let settings = crate::storage::settings::load_settings();
                if !settings.wake_word_enabled {
                    is_enabled.store(false, Ordering::SeqCst);
                    break;
                }

                let whisper_mode = settings.whisper_mode_enabled;
                let min_onset = if whisper_mode { 80.0 } else { 165.0 };
                let min_active = if whisper_mode { 62.0 } else { 125.0 };

                let frame_rms = compute_rms(&new_samples);
                let onset_rms_threshold = (noise_floor_rms * 2.35).max(min_onset);
                let base_active_threshold = (noise_floor_rms * 1.75).max(min_active);
                let now = Instant::now();

                match &mut state {
                    WakeState::Standby {
                        onset_frames,
                        cooldown_until,
                    } => {
                        for &s in &new_samples {
                            if pre_roll.len() >= PRE_ROLL_SAMPLES {
                                pre_roll.pop_front();
                            }
                            pre_roll.push_back(s);
                        }

                        // Continuously adapt ambient noise floor when below speech onset
                        if frame_rms < onset_rms_threshold {
                            if frame_rms <= noise_floor_rms * 1.25 {
                                noise_floor_rms =
                                    (0.90 * noise_floor_rms + 0.10 * frame_rms).clamp(40.0, 420.0);
                            } else {
                                noise_floor_rms =
                                    (0.985 * noise_floor_rms + 0.015 * frame_rms).clamp(40.0, 420.0);
                            }
                        }

                        if let Some(until) = cooldown_until {
                            if now < *until {
                                continue;
                            } else {
                                *cooldown_until = None;
                            }
                        }

                        if frame_rms >= onset_rms_threshold {
                            *onset_frames += 1;
                            if *onset_frames >= 2 {
                                utterance.clear();
                                utterance.extend(pre_roll.iter().copied());
                                utterance.extend_from_slice(&new_samples);
                                *probe_result.lock() = None;
                                state = WakeState::CapturingCandidate {
                                    started_at: now,
                                    last_voice_at: now,
                                    probes_sent: 0,
                                    probe_in_flight: false,
                                    last_probe_len: 0,
                                    peak_rms: frame_rms.max(onset_rms_threshold),
                                };
                            }
                        } else {
                            *onset_frames = 0;
                        }
                    }

                    WakeState::CapturingCandidate {
                        started_at,
                        last_voice_at,
                        probes_sent,
                        probe_in_flight,
                        last_probe_len,
                        peak_rms,
                    } => {
                        utterance.extend_from_slice(&new_samples);
                        if frame_rms > *peak_rms {
                            *peak_rms = frame_rms;
                        } else {
                            *peak_rms = (*peak_rms * 0.992).max(onset_rms_threshold);
                        }

                        let dynamic_active_threshold =
                            base_active_threshold.max(*peak_rms * 0.18);
                        if frame_rms >= dynamic_active_threshold {
                            *last_voice_at = now;
                        }

                        // Check if an async probe returned
                        let maybe_outcome = probe_result.lock().take();
                        if let Some(outcome) = maybe_outcome {
                            *probe_in_flight = false;
                            if outcome.matched {
                                // "Hey Rift" confirmed! Invoke immediately!
                                on_activated();

                                let post_wake_samples =
                                    utterance.len().saturating_sub(outcome.probe_samples_len);
                                let spoke_after_probe = post_wake_samples
                                    >= (SAMPLE_RATE as usize * 28) / 100
                                    && now.duration_since(*last_voice_at)
                                        < Duration::from_millis(450);

                                // If the probe only contained "Hey Rift" (no trailing command words inside the probe slice),
                                // trim the "Hey Rift" prefix audio from `utterance` so downstream STT never hears "Hey Rift"!
                                if !outcome.has_trailing_words && outcome.probe_samples_len > 2400 {
                                    let drain_len = outcome
                                        .probe_samples_len
                                        .saturating_sub(1600)
                                        .min(utterance.len());
                                    utterance.drain(0..drain_len);
                                }

                                let has_post = outcome.has_trailing_words || spoke_after_probe;
                                state = WakeState::ActiveDictation {
                                    activated_at: now,
                                    last_voice_at: now,
                                    has_post_wake_speech: has_post,
                                    peak_rms: (*peak_rms * 0.75).max(onset_rms_threshold),
                                };
                                continue;
                            } else {
                                // Probe did not match "Hey Rift".
                                // If this was only Stage 1 probe AND the user is still speaking (e.g. had a breath before "Hey Rift"),
                                // allow Stage 2 probe instead of discarding!
                                let silence_ms =
                                    now.duration_since(*last_voice_at).as_millis() as u64;
                                if *probes_sent < 2 && silence_ms < 450 {
                                    // Keep capturing for Stage 2 probe
                                } else {
                                    utterance.clear();
                                    state = WakeState::Standby {
                                        onset_frames: 0,
                                        cooldown_until: Some(now + Duration::from_millis(180)),
                                    };
                                    continue;
                                }
                            }
                        }

                        let elapsed_ms = now.duration_since(*started_at).as_millis() as u64;
                        let silence_ms = now.duration_since(*last_voice_at).as_millis() as u64;

                        if !*probe_in_flight {
                            if *probes_sent == 0 {
                                // Stage 1 Fast Probe:
                                // 1) User spoke for >= 240ms and paused for >= 170ms ("Hey Rift" [pause])
                                // 2) Or user reached >= 660ms of speech ("Hey Rift, ...")
                                let paused_after_wake = elapsed_ms >= 240
                                    && silence_ms >= 170
                                    && utterance.len() >= (SAMPLE_RATE as usize * 62) / 100;
                                let continuous_speech_ready = elapsed_ms >= 660
                                    && utterance.len() >= (SAMPLE_RATE as usize * 105) / 100;

                                if paused_after_wake || continuous_speech_ready {
                                    *probes_sent = 1;
                                    *probe_in_flight = true;
                                    let probe_slice_len =
                                        utterance.len().min((SAMPLE_RATE as usize * 18) / 10);
                                    *last_probe_len = probe_slice_len;
                                    let probe_samples = utterance[..probe_slice_len].to_vec();
                                    let probe_slot = Arc::clone(&probe_result);

                                    tauri::async_runtime::spawn(async move {
                                        let outcome =
                                            verify_wake_word_audio(&probe_samples, probe_slice_len)
                                                .await;
                                        *probe_slot.lock() = Some(outcome);
                                    });
                                } else if silence_ms > 520 && elapsed_ms < 240 {
                                    // Brief click/noise (<240ms) followed by silence — discard without API call
                                    utterance.clear();
                                    state = WakeState::Standby {
                                        onset_frames: 0,
                                        cooldown_until: None,
                                    };
                                }
                            } else if *probes_sent == 1 {
                                // Stage 2 Continuation Probe:
                                // Fires if user continued speaking after Stage 1 and either paused (>=170ms) or reached 1350ms
                                let new_audio_since_stage1 =
                                    utterance.len().saturating_sub(*last_probe_len);
                                let stage2_ready = new_audio_since_stage1
                                    >= (SAMPLE_RATE as usize * 35) / 100
                                    && (silence_ms >= 170 || elapsed_ms >= 1350);

                                if stage2_ready {
                                    *probes_sent = 2;
                                    *probe_in_flight = true;
                                    let probe_slice_len =
                                        utterance.len().min((SAMPLE_RATE as usize * 26) / 10);
                                    *last_probe_len = probe_slice_len;
                                    let probe_samples = utterance[..probe_slice_len].to_vec();
                                    let probe_slot = Arc::clone(&probe_result);

                                    tauri::async_runtime::spawn(async move {
                                        let outcome =
                                            verify_wake_word_audio(&probe_samples, probe_slice_len)
                                                .await;
                                        *probe_slot.lock() = Some(outcome);
                                    });
                                } else if silence_ms > 550 {
                                    utterance.clear();
                                    state = WakeState::Standby {
                                        onset_frames: 0,
                                        cooldown_until: Some(now + Duration::from_millis(150)),
                                    };
                                }
                            }
                        } else if elapsed_ms > 5500 {
                            // Safety timeout if network probe hung
                            utterance.clear();
                            state = WakeState::Standby {
                                onset_frames: 0,
                                cooldown_until: Some(now + Duration::from_millis(200)),
                            };
                        }
                    }

                    WakeState::ActiveDictation {
                        activated_at,
                        last_voice_at,
                        has_post_wake_speech,
                        peak_rms,
                    } => {
                        utterance.extend_from_slice(&new_samples);

                        let level_scale = if whisper_mode { 3 } else { 1 };
                        let level = (calculate_level(&new_samples) * level_scale).min(100);
                        on_level(level);

                        if frame_rms > *peak_rms {
                            *peak_rms = frame_rms;
                        } else {
                            *peak_rms = (*peak_rms * 0.994).max(onset_rms_threshold);
                        }

                        // Peak-relative + noise-floor-aware voice detection prevents background fan/Bluetooth hiss
                        // from resetting the silence completion timer after the user stops speaking!
                        let dynamic_voice_threshold =
                            base_active_threshold.max(*peak_rms * 0.18);

                        if frame_rms >= dynamic_voice_threshold {
                            *last_voice_at = now;
                            if now.duration_since(*activated_at) >= Duration::from_millis(120) {
                                *has_post_wake_speech = true;
                            }
                        }

                        let silence_ms = now.duration_since(*last_voice_at).as_millis() as u64;
                        let total_active_secs = now.duration_since(*activated_at).as_secs();

                        if *has_post_wake_speech && silence_ms >= SILENCE_COMPLETION_MS {
                            // User finished speaking! Complete and transcribe immediately.
                            let completed_samples = std::mem::take(&mut utterance);
                            pre_roll.clear();
                            state = WakeState::Standby {
                                onset_frames: 0,
                                cooldown_until: Some(now + Duration::from_millis(300)),
                            };
                            on_complete(completed_samples);
                        } else if !*has_post_wake_speech && silence_ms >= POST_WAKE_WAIT_TIMEOUT_MS {
                            // User only said "Hey Rift" and stayed silent for 5.5 seconds -> cancel cleanly
                            utterance.clear();
                            pre_roll.clear();
                            state = WakeState::Standby {
                                onset_frames: 0,
                                cooldown_until: None,
                            };
                            on_cancelled();
                        } else if total_active_secs >= MAX_DICTATION_DURATION_SECS {
                            let completed_samples = std::mem::take(&mut utterance);
                            pre_roll.clear();
                            state = WakeState::Standby {
                                onset_frames: 0,
                                cooldown_until: Some(now + Duration::from_millis(300)),
                            };
                            on_complete(completed_samples);
                        }
                    }
                }
            }
        });

        Ok(())
    }
}

fn compute_rms(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples
        .iter()
        .map(|&s| {
            let v = s as f64;
            v * v
        })
        .sum();
    (sum_sq / samples.len() as f64).sqrt()
}

/// Verifies whether a short 16kHz audio slice starts with "Hey Rift" using a warm pooled
/// Groq whisper-large-v3-turbo connection (or local offline whisper fallback).
async fn verify_wake_word_audio(samples_16k: &[i16], probe_samples_len: usize) -> WakeProbeOutcome {
    let t0 = Instant::now();
    let mut normalized = samples_16k.to_vec();
    super::capture::apply_high_pass_filter(&mut normalized, SAMPLE_RATE, 95.0);
    super::capture::normalize_speech_agc_with_mode(&mut normalized, true);
    let wav_bytes = encode_wav(&normalized, SAMPLE_RATE);

    // Tier 1: Fast Groq Whisper-Large-v3-Turbo probe over warm pooled HTTP client
    if let Some(groq_key) = CredentialsProvider::get_api_key() {
        let gkey = groq_key.trim();
        if !gkey.is_empty() {
            match probe_groq_whisper(gkey, wav_bytes.clone()).await {
                Ok(text) => {
                    let matched = contains_wake_word(&text);
                    let remainder = crate::ai::intent_router::strip_wake_word_prefix(&text);
                    let has_trailing_words =
                        matched && remainder.split_whitespace().count() >= 1;
                    crate::log_status(&format!(
                        "[WakeWord] Probe ({}ms, {} samples): '{}' -> matched={}, trailing={}",
                        t0.elapsed().as_millis(),
                        probe_samples_len,
                        text.trim(),
                        matched,
                        has_trailing_words
                    ));
                    return WakeProbeOutcome {
                        matched,
                        has_trailing_words,
                        probe_samples_len,
                    };
                }
                Err(e) => {
                    crate::log_status(&format!(
                        "[WakeWord] Groq probe error ({}ms): {}",
                        t0.elapsed().as_millis(),
                        e
                    ));
                }
            }
        }
    }

    // Tier 2: Offline Local Whisper fallback if no Groq key or Groq probe errored
    let settings = crate::storage::settings::load_settings();
    let profile = settings.local_inference_profile.clone();
    if let Ok(Ok(text)) =
        tokio::task::spawn_blocking(move || crate::stt::local::transcribe_local(&wav_bytes, &profile))
            .await
    {
        let matched = contains_wake_word(&text);
        let remainder = crate::ai::intent_router::strip_wake_word_prefix(&text);
        let has_trailing_words = matched && remainder.split_whitespace().count() >= 1;
        return WakeProbeOutcome {
            matched,
            has_trailing_words,
            probe_samples_len,
        };
    }

    WakeProbeOutcome {
        matched: false,
        has_trailing_words: false,
        probe_samples_len,
    }
}

async fn probe_groq_whisper(api_key: &str, wav_bytes: Vec<u8>) -> Result<String, String> {
    let client = get_wake_http_client();

    let part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("wake_probe.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-large-v3-turbo")
        .text("language", "en")
        .text("prompt", "Hey Rift. Hey Rift, Hi Rift, Okay Rift.")
        .text("response_format", "json")
        .text("temperature", "0.0");

    let resp = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("Groq probe HTTP {}", resp.status()));
    }

    let data: Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(data["text"].as_str().unwrap_or("").to_string())
}

/// Checks if a short probe transcript begins with or contains "Hey Rift" (including common
/// Bluetooth narrowband HFP & Indian-English phonetic near-matches for "Hey Rift").
pub fn contains_wake_word(transcript: &str) -> bool {
    let cleaned: String = transcript
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    let norm = cleaned.split_whitespace().collect::<Vec<&str>>().join(" ");

    let patterns = [
        "hey rift",
        "hi rift",
        "okay rift",
        "ok rift",
        "yo rift",
        "hey rithvik",
        "hey ritvik",
        "hey rith",
        "hey rit",
        "hey ref",
        "hey lift",
        "hey drift",
        "hey wrist",
        "hey ript",
        "hey rif",
        "hey rick",
        "hey rich",
        "hey gift",
        "hey swift",
        "hey roof",
        "hair rift",
        "a rift",
    ];

    for pat in &patterns {
        if norm.starts_with(pat) || norm.contains(pat) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_wake_word() {
        assert!(contains_wake_word("Hey Rift, send a message to John."));
        assert!(contains_wake_word("Hey, Rift!"));
        assert!(contains_wake_word("Okay Rift write a python script"));
        assert!(contains_wake_word("Hey drift make this professional"));
        assert!(contains_wake_word("Hey Rithvik!"));
        assert!(!contains_wake_word("Hello everyone let us start the meeting"));
    }
}
