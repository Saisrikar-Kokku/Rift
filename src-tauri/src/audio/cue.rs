use std::f32::consts::PI;
use std::sync::Mutex;

#[cfg(windows)]
#[link(name = "winmm")]
extern "system" {
    fn PlaySoundA(pszSound: *const u8, hmod: *mut std::ffi::c_void, fdwSound: u32) -> i32;
}

#[cfg(windows)]
const SND_ASYNC: u32 = 0x0001;
#[cfg(windows)]
const SND_NODEFAULT: u32 = 0x0002;
#[cfg(windows)]
const SND_MEMORY: u32 = 0x0004;
#[cfg(windows)]
const PLAY_FLAGS: u32 = SND_ASYNC | SND_NODEFAULT | SND_MEMORY;

struct PrecomputedCues {
    start: Vec<u8>,
    success: Vec<u8>,
    cancel: Vec<u8>,
    volume: f32,
}

static CUE_CACHE: Mutex<Option<PrecomputedCues>> = Mutex::new(None);

fn make_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 2;
    let block_align: u16 = 2;
    let bits_per_sample: u16 = 16;
    let subchunk2_size = num_samples * 2;
    let chunk_size = 36 + subchunk2_size;

    let mut buf = Vec::with_capacity((44 + subchunk2_size) as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&chunk_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&1u16.to_le_bytes()); // Mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&subchunk2_size.to_le_bytes());

    for &s in samples {
        let clamped = s.max(-1.0).min(1.0);
        let val = (clamped * 32767.0) as i16;
        buf.extend_from_slice(&val.to_le_bytes());
    }

    buf
}

fn synth_start_tone(volume: f32) -> Vec<u8> {
    let sr = 44100u32;
    let duration = 0.120f32; // 120ms - warm, sleek, rising above keyclick and DAC buffer latency
    let num_samples = (sr as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(num_samples);
    
    // Elegant musical ascent: C5 (523.25 Hz) -> G5 (783.99 Hz) (Pure harmonic 5th)
    let f_start = 523.25f32;
    let f_end = 783.99f32;
    let mut phase1 = 0.0f32;
    let mut phase2 = 0.0f32;

    for i in 0..num_samples {
        let t = i as f32 / sr as f32;
        let progress = t / duration;
        // Smooth S-curve pitch transition
        let freq = f_start + (f_end - f_start) * (1.0 - (progress * PI).cos()) * 0.5;
        phase1 += 2.0 * PI * freq / sr as f32;
        phase2 += 2.0 * PI * (freq * 2.0) / sr as f32;

        // Smooth 12ms cosine attack to completely eliminate any digital clicks/pops
        let attack_time = 0.012f32;
        let env = if t < attack_time {
            0.5 * (1.0 - (PI * t / attack_time).cos())
        } else {
            let decay_progress = (t - attack_time) / (duration - attack_time);
            (-decay_progress * 2.5).exp()
        };

        // 80% fundamental + 20% 2nd harmonic for rich glass-bell warmth
        let sample = (phase1.sin() * 0.80 + phase2.sin() * 0.20) * env * volume * 0.65;
        samples.push(sample);
    }

    make_wav(&samples, sr)
}

fn synth_success_tone(volume: f32) -> Vec<u8> {
    let sr = 44100u32;
    let duration = 0.130f32; // 130ms - satisfying double-chime resolution
    let num_samples = (sr as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / sr as f32;
        let mut s = 0.0f32;

        // Bell 1: E5 (659.25 Hz)
        if t < 0.080 {
            let att1 = (t / 0.006).min(1.0);
            let env1 = att1 * (-t * 30.0).exp();
            s += ((2.0 * PI * 659.25 * t).sin() * 0.82 + (2.0 * PI * 1318.5 * t).sin() * 0.18) * env1 * 0.55;
        }

        // Bell 2: B5 (987.77 Hz) entering at 32ms
        if t >= 0.032 {
            let t2 = t - 0.032;
            let att2 = (t2 / 0.006).min(1.0);
            let env2 = att2 * (-t2 * 26.0).exp();
            s += ((2.0 * PI * 987.77 * t2).sin() * 0.85 + (2.0 * PI * 1975.54 * t2).sin() * 0.15) * env2 * 0.70;
        }

        let sample = s * volume * 0.62;
        samples.push(sample);
    }

    make_wav(&samples, sr)
}

fn synth_cancel_tone(volume: f32) -> Vec<u8> {
    let sr = 44100u32;
    let duration = 0.050f32; // 50ms
    let num_samples = (sr as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / sr as f32;
        let progress = t / duration;
        let freq = 340.0 - 140.0 * progress;
        let att = (t / 0.005).min(1.0);
        let env = att * (1.0 - progress).max(0.0);
        let sample = (2.0 * PI * freq * t).sin() * env * volume * 0.32;
        samples.push(sample);
    }

    make_wav(&samples, sr)
}

pub fn prewarm(volume: f32) {
    ensure_cache(volume);
}

fn ensure_cache(volume: f32) {
    let mut lock = match CUE_CACHE.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };

    let needs_update = match &*lock {
        Some(c) => (c.volume - volume).abs() > 0.02,
        None => true,
    };

    if needs_update {
        let clamped_vol = volume.max(0.0).min(1.0);
        let start = synth_start_tone(clamped_vol);
        let success = synth_success_tone(clamped_vol);
        let cancel = synth_cancel_tone(clamped_vol);

        #[cfg(target_os = "macos")]
        {
            let _ = std::fs::write("/tmp/rift_start.wav", &start);
            let _ = std::fs::write("/tmp/rift_success.wav", &success);
            let _ = std::fs::write("/tmp/rift_cancel.wav", &cancel);
        }

        *lock = Some(PrecomputedCues {
            start,
            success,
            cancel,
            volume: clamped_vol,
        });
    }
}

pub fn play_start(volume: f32) {
    if volume <= 0.01 {
        return;
    }
    ensure_cache(volume);
    #[cfg(windows)]
    if let Ok(lock) = CUE_CACHE.lock() {
        if let Some(cues) = &*lock {
            unsafe {
                let res = PlaySoundA(cues.start.as_ptr(), std::ptr::null_mut(), PLAY_FLAGS);
                crate::log_status(&format!("cue::play_start executed (vol={}, res={})", volume, res));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("afplay")
            .arg("/tmp/rift_start.wav")
            .spawn();
    }
}

pub fn play_success(volume: f32) {
    if volume <= 0.01 {
        return;
    }
    ensure_cache(volume);
    #[cfg(windows)]
    if let Ok(lock) = CUE_CACHE.lock() {
        if let Some(cues) = &*lock {
            unsafe {
                let res = PlaySoundA(cues.success.as_ptr(), std::ptr::null_mut(), PLAY_FLAGS);
                crate::log_status(&format!("cue::play_success executed (vol={}, res={})", volume, res));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("afplay")
            .arg("/tmp/rift_success.wav")
            .spawn();
    }
}

pub fn play_cancel(volume: f32) {
    if volume <= 0.01 {
        return;
    }
    ensure_cache(volume);
    #[cfg(windows)]
    if let Ok(lock) = CUE_CACHE.lock() {
        if let Some(cues) = &*lock {
            unsafe {
                let res = PlaySoundA(cues.cancel.as_ptr(), std::ptr::null_mut(), PLAY_FLAGS);
                crate::log_status(&format!("cue::play_cancel executed (vol={}, res={})", volume, res));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("afplay")
            .arg("/tmp/rift_cancel.wav")
            .spawn();
    }
}
