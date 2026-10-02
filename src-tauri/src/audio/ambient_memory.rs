use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;

pub const AMBIENT_SAMPLE_RATE: u32 = 16000;

pub struct RollingBuffer {
    buffer: VecDeque<i16>,
    capacity: usize,
}

impl RollingBuffer {
    pub fn new(capacity_samples: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            capacity: capacity_samples,
        }
    }

    pub fn push(&mut self, sample: i16) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(sample);
    }

    pub fn get_samples(&self) -> Vec<i16> {
        let (slice1, slice2) = self.buffer.as_slices();
        let mut res = Vec::with_capacity(slice1.len() + slice2.len());
        res.extend_from_slice(slice1);
        res.extend_from_slice(slice2);
        res
    }
}

pub struct AmbientRecorder {
    is_running: Arc<AtomicBool>,
    mic_buffer: Arc<Mutex<RollingBuffer>>,
    sys_buffer: Arc<Mutex<RollingBuffer>>,
    mic_stream: Option<Stream>,
    sys_stream: Option<Stream>,
}

unsafe impl Send for AmbientRecorder {}
unsafe impl Sync for AmbientRecorder {}

impl AmbientRecorder {
    pub fn new(duration_seconds: u32) -> Self {
        let capacity = (AMBIENT_SAMPLE_RATE * duration_seconds) as usize;
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            mic_buffer: Arc::new(Mutex::new(RollingBuffer::new(capacity))),
            sys_buffer: Arc::new(Mutex::new(RollingBuffer::new(capacity))),
            mic_stream: None,
            sys_stream: None,
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.is_running.load(Ordering::SeqCst) {
            return Ok(());
        }
        
        self.is_running.store(true, Ordering::SeqCst);
        let host = cpal::default_host();
        
        let err_fn = |err| eprintln!("Ambient Audio stream error: {}", err);

        // 1. Setup Microphone Capture
        let mic_stream = if let Some(in_dev) = host.default_input_device() {
            if let Ok(config) = in_dev.default_input_config() {
                let channels = config.channels() as usize;
                let buf_clone = Arc::clone(&self.mic_buffer);
                let is_running_clone = Arc::clone(&self.is_running);
                
                match config.sample_format() {
                    cpal::SampleFormat::I16 => {
                        in_dev.build_input_stream(
                            &config.into(),
                            move |data: &[i16], _| {
                                if !is_running_clone.load(Ordering::Relaxed) { return; }
                                if let Ok(mut b) = buf_clone.lock() {
                                    for chunk in data.chunks(channels) {
                                        let avg = chunk.iter().map(|&s| s as f32).sum::<f32>() / channels as f32;
                                        b.push(avg as i16);
                                    }
                                }
                            },
                            err_fn,
                            None
                        ).ok()
                    },
                    cpal::SampleFormat::F32 => {
                        in_dev.build_input_stream(
                            &config.into(),
                            move |data: &[f32], _| {
                                if !is_running_clone.load(Ordering::Relaxed) { return; }
                                if let Ok(mut b) = buf_clone.lock() {
                                    for chunk in data.chunks(channels) {
                                        let avg = chunk.iter().copied().sum::<f32>() / channels as f32;
                                        b.push((avg * 32767.0) as i16);
                                    }
                                }
                            },
                            err_fn,
                            None
                        ).ok()
                    },
                    _ => None,
                }
            } else { None }
        } else { None };

        // 2. Setup System Loopback Capture
        // On Windows WASAPI, calling build_input_stream on an output device activates loopback mode automatically.
        let sys_stream = if let Some(out_dev) = host.default_output_device() {
            if let Ok(config) = out_dev.default_output_config() {
                let channels = config.channels() as usize;
                let buf_clone = Arc::clone(&self.sys_buffer);
                let is_running_clone = Arc::clone(&self.is_running);
                
                match config.sample_format() {
                    cpal::SampleFormat::I16 => {
                        out_dev.build_input_stream(
                            &config.into(),
                            move |data: &[i16], _| {
                                if !is_running_clone.load(Ordering::Relaxed) { return; }
                                if let Ok(mut b) = buf_clone.lock() {
                                    for chunk in data.chunks(channels) {
                                        let avg = chunk.iter().map(|&s| s as f32).sum::<f32>() / channels as f32;
                                        b.push(avg as i16);
                                    }
                                }
                            },
                            err_fn,
                            None
                        ).ok()
                    },
                    cpal::SampleFormat::F32 => {
                        out_dev.build_input_stream(
                            &config.into(),
                            move |data: &[f32], _| {
                                if !is_running_clone.load(Ordering::Relaxed) { return; }
                                if let Ok(mut b) = buf_clone.lock() {
                                    for chunk in data.chunks(channels) {
                                        let avg = chunk.iter().copied().sum::<f32>() / channels as f32;
                                        b.push((avg * 32767.0) as i16);
                                    }
                                }
                            },
                            err_fn,
                            None
                        ).ok()
                    },
                    _ => None,
                }
            } else { None }
        } else { None };

        if let Some(stream) = &mic_stream {
            let _ = stream.play();
        }
        if let Some(stream) = &sys_stream {
            let _ = stream.play();
        }
        
        self.mic_stream = mic_stream;
        self.sys_stream = sys_stream;

        Ok(())
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        let _ = self.mic_stream.take();
        let _ = self.sys_stream.take();
    }
    
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn extract_wav_bytes(&self) -> Vec<u8> {
        let mic_samples = if let Ok(b) = self.mic_buffer.lock() { b.get_samples() } else { Vec::new() };
        let sys_samples = if let Ok(b) = self.sys_buffer.lock() { b.get_samples() } else { Vec::new() };
        
        let max_len = mic_samples.len().max(sys_samples.len());
        if max_len == 0 {
            return Vec::new();
        }

        // Mix the two buffers by summing and soft-clipping
        let mut mixed = Vec::with_capacity(max_len);
        let mic_offset = max_len.saturating_sub(mic_samples.len());
        let sys_offset = max_len.saturating_sub(sys_samples.len());

        for i in 0..max_len {
            let m = if i >= mic_offset { mic_samples[i - mic_offset] as f32 / 32768.0 } else { 0.0 };
            let s = if i >= sys_offset { sys_samples[i - sys_offset] as f32 / 32768.0 } else { 0.0 };
            
            let mut sum = m + s;
            if sum > 1.0 { sum = 1.0; }
            if sum < -1.0 { sum = -1.0; }
            
            mixed.push((sum * 32767.0) as i16);
        }

        let header_len = 44;
        let data_len = mixed.len() * 2;
        let mut wav = Vec::with_capacity(header_len + data_len);
        
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // Mono
        wav.extend_from_slice(&AMBIENT_SAMPLE_RATE.to_le_bytes());
        wav.extend_from_slice(&(AMBIENT_SAMPLE_RATE * 2).to_le_bytes()); // byte rate
        wav.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data_len as u32).to_le_bytes());
        
        for &s in &mixed {
            wav.extend_from_slice(&s.to_le_bytes());
        }
        
        wav
    }
}
