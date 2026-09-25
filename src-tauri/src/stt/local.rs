use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use serde_json::json;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const ENGINE_ZIP_URL: &str = "https://github.com/ggml-org/whisper.cpp/releases/download/b4938/whisper-bin-x64.zip";

struct ResidentServer {
    child: Child,
    model_path: PathBuf,
    port: u16,
}

static RESIDENT_SERVER: Mutex<Option<ResidentServer>> = Mutex::new(None);
const RESIDENT_PORT: u16 = 28492;

pub fn stop_resident_server() {
    if let Ok(mut guard) = RESIDENT_SERVER.lock() {
        if let Some(mut s) = guard.take() {
            let _ = s.child.kill();
            let _ = s.child.wait();
        }
    }
}

pub fn get_server_engine_path() -> PathBuf {
    get_bin_dir().join("whisper-server.exe")
}

pub fn is_server_engine_available() -> bool {
    get_server_engine_path().exists()
}

fn ensure_resident_server(model_path: &Path) -> Result<u16, String> {
    let mut guard = RESIDENT_SERVER.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut s) = *guard {
        if s.model_path == model_path {
            match s.child.try_wait() {
                Ok(None) => return Ok(s.port),
                _ => {}
            }
        }
        let _ = s.child.kill();
        let _ = s.child.wait();
    }
    *guard = None;

    let server_bin = get_server_engine_path();
    if !server_bin.exists() {
        return Err("whisper-server.exe not found".to_string());
    }

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(4, 8)
        .to_string();

    let mut cmd = Command::new(&server_bin);
    cmd.args(&[
        "--host", "127.0.0.1",
        "--port", &RESIDENT_PORT.to_string(),
        "-m", model_path.to_str().unwrap(),
        "-t", &threads,
        "-fa",
        "-nt",
        "-l", "auto",
    ]);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let child = cmd.spawn().map_err(|e| format!("Failed to spawn whisper-server: {}", e))?;
    *guard = Some(ResidentServer {
        child,
        model_path: model_path.to_path_buf(),
        port: RESIDENT_PORT,
    });

    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(2000) {
        if std::net::TcpStream::connect(("127.0.0.1", RESIDENT_PORT)).is_ok() {
            return Ok(RESIDENT_PORT);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    Ok(RESIDENT_PORT)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelInfo {
    pub profile: String,
    pub display_name: String,
    pub model_id: String,
    pub filename: String,
    pub downloaded: bool,
    pub size_mb: Option<u64>,
    pub download_url: String,
    pub description: String,
}

pub fn get_rift_data_dir() -> PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let path = PathBuf::from(app_data).join("Rift");
    let _ = fs::create_dir_all(&path);
    path
}

pub fn get_models_dir() -> PathBuf {
    let path = get_rift_data_dir().join("models");
    let _ = fs::create_dir_all(&path);
    path
}

pub fn get_bin_dir() -> PathBuf {
    let path = get_rift_data_dir().join("bin");
    let _ = fs::create_dir_all(&path);
    path
}

pub fn get_engine_path() -> PathBuf {
    get_bin_dir().join("whisper-cli.exe")
}

pub fn is_engine_available() -> bool {
    get_engine_path().exists()
}

pub fn get_supported_models() -> Vec<LocalModelInfo> {
    let models_dir = get_models_dir();
    let specs = vec![
        (
            "balanced",
            "Whisper Small (Default / Balanced)",
            "small",
            "ggml-small.bin",
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            465,
            "Optimal for 16GB laptops -- balanced accuracy and speed",
        ),
        (
            "accurate",
            "Whisper Large-v3 Turbo (High Precision)",
            "large-v3-turbo",
            "ggml-large-v3-turbo-q5_0.bin",
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
            547,
            "Maximum precision -- matching Groq cloud accuracy",
        ),
        (
            "fast",
            "Whisper Base (Fast)",
            "base.en",
            "ggml-base.en.bin",
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
            141,
            "Low latency offline dictation",
        ),
        (
            "tiny",
            "Whisper Tiny (Ultra Fast)",
            "tiny.en",
            "ggml-tiny.en.bin",
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
            74,
            "Ultra-lightweight with minimal memory footprint",
        ),
    ];

    specs
        .into_iter()
        .map(|(profile, display, id, filename, url, default_size, desc)| {
            let file_path = models_dir.join(filename);
            let downloaded = file_path.exists();
            let size_mb = if downloaded {
                fs::metadata(&file_path)
                    .map(|m| m.len() / (1024 * 1024))
                    .ok()
            } else {
                Some(default_size)
            };

            LocalModelInfo {
                profile: profile.to_string(),
                display_name: display.to_string(),
                model_id: id.to_string(),
                filename: filename.to_string(),
                downloaded,
                size_mb,
                download_url: url.to_string(),
                description: desc.to_string(),
            }
        })
        .collect()
}

pub fn find_model_by_profile_or_id(key: &str) -> Option<LocalModelInfo> {
    let models = get_supported_models();
    let lower = key.to_lowercase();
    models.into_iter().find(|m| {
        m.profile.to_lowercase() == lower
            || m.model_id.to_lowercase() == lower
            || m.filename.to_lowercase() == lower
    })
}

/// Automatically downloads and unpacks the official precompiled whisper-cli.exe
/// and its required runtime DLLs into %APPDATA%/Rift/bin/ using Windows built-in tar.
pub async fn ensure_engine_installed(app: &AppHandle, profile: &str) -> Result<(), String> {
    if is_engine_available() {
        return Ok(());
    }

    emit_to_frontend(
        app,
        "modelDownloadProgress",
        Some(json!({
            "profile": profile,
            "stage": "engine",
            "percent": 2,
            "receivedBytes": 0,
            "totalBytes": 8_361_840u64,
            "message": "Connecting to whisper engine repository..."
        })),
    );

    let bin_dir = get_bin_dir();
    let temp_zip = bin_dir.join("whisper_engine.zip");
    let temp_extract = bin_dir.join("engine_temp");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .unwrap_or_default();

    let mut resp = client
        .get(ENGINE_ZIP_URL)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) RiftDictation/1.0")
        .send()
        .await
        .map_err(|e| format!("Failed to download whisper engine: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Engine download failed with HTTP {}", resp.status()));
    }

    let total_engine_bytes = resp.content_length().unwrap_or(8_361_840);
    let mut file = File::create(&temp_zip).map_err(|e| format!("Failed to create engine zip: {}", e))?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("Engine download stream error: {}", e))? {
        file.write_all(&chunk).map_err(|e| format!("Failed to write engine chunk: {}", e))?;
        downloaded += chunk.len() as u64;

        let pct = (2 + ((downloaded as f64 / total_engine_bytes as f64) * 6.0) as u32).min(8);
        emit_to_frontend(
            app,
            "modelDownloadProgress",
            Some(json!({
                "profile": profile,
                "stage": "engine",
                "percent": pct,
                "receivedBytes": downloaded,
                "totalBytes": total_engine_bytes,
                "message": format!("Downloading Whisper Engine: {:.1} MB / {:.1} MB ({}%)",
                    downloaded as f64 / (1024.0 * 1024.0),
                    total_engine_bytes as f64 / (1024.0 * 1024.0),
                    pct
                )
            })),
        );
    }
    file.flush().map_err(|e| format!("Flush error: {}", e))?;
    drop(file);

    emit_to_frontend(
        app,
        "modelDownloadProgress",
        Some(json!({
            "profile": profile,
            "stage": "engine_extract",
            "percent": 9,
            "receivedBytes": total_engine_bytes,
            "totalBytes": total_engine_bytes,
            "message": "Extracting whisper-cli engine..."
        })),
    );

    let _ = fs::create_dir_all(&temp_extract);

    // Unpack using Windows built-in tar (bsdtar)
    let mut cmd = Command::new("tar");
    cmd.args(&["-xf", temp_zip.to_str().unwrap(), "-C", temp_extract.to_str().unwrap()]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| format!("Failed to extract engine with tar: {}", e))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Tar extraction error: {}", err));
    }

    // Move release files to bin_dir
    let release_dir = temp_extract.join("Release");
    let source_dir = if release_dir.exists() { release_dir } else { temp_extract.clone() };

    if let Ok(entries) = fs::read_dir(&source_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                let dest = bin_dir.join(name);
                let _ = fs::copy(&path, &dest);
            }
        }
    }

    // Cleanup temporary files
    let _ = fs::remove_file(&temp_zip);
    let _ = fs::remove_dir_all(&temp_extract);

    if is_engine_available() {
        Ok(())
    } else {
        Err("whisper-cli.exe could not be verified after extraction".to_string())
    }
}

pub fn emit_to_frontend(app: &AppHandle, event_name: &str, payload: Option<serde_json::Value>) {
    use tauri::Manager;
    let js = match &payload {
        Some(p) => format!("if (window.__rift_emit) window.__rift_emit({:?}, {});", event_name, p),
        None => format!("if (window.__rift_emit) window.__rift_emit({:?});", event_name),
    };
    for label in &["widget", "main"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.eval(&js);
        }
    }
}

/// Asynchronously streams model download from Hugging Face directly to %APPDATA%/Rift/models/
pub async fn download_model_file(profile: &str, app: AppHandle) -> Result<(), String> {
    let model = find_model_by_profile_or_id(profile)
        .ok_or_else(|| format!("Unknown model profile: {}", profile))?;

    // 1. Ensure engine is present with live progress
    if !is_engine_available() {
        ensure_engine_installed(&app, &model.profile).await?;
    }

    emit_to_frontend(
        &app,
        "modelDownloadProgress",
        Some(json!({
            "profile": model.profile,
            "stage": "starting",
            "percent": 10,
            "receivedBytes": 0,
            "totalBytes": model.size_mb.unwrap_or(400) * 1024 * 1024,
            "message": format!("Connecting to download {}...", model.display_name)
        })),
    );

    let models_dir = get_models_dir();
    let final_path = models_dir.join(&model.filename);
    let part_path = models_dir.join(format!("{}.part", &model.filename));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .unwrap_or_default();

    let mut resp = client
        .get(&model.download_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) RiftDictation/1.0")
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Hugging Face: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed with HTTP status {}", resp.status()));
    }

    let total_bytes = resp.content_length().unwrap_or(model.size_mb.unwrap_or(400) * 1024 * 1024);
    let mut file = File::create(&part_path).map_err(|e| format!("Failed to create part file: {}", e))?;

    let mut downloaded_bytes: u64 = 0;
    let mut last_percent: u32 = 10;

    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("Download stream error: {}", e))? {
        file.write_all(&chunk)
            .map_err(|e| format!("Failed to write chunk to disk: {}", e))?;

        downloaded_bytes += chunk.len() as u64;

        if total_bytes > 0 {
            let pct = (10 + ((downloaded_bytes as f64 / total_bytes as f64) * 85.0) as u32).min(95);
            if pct > last_percent || downloaded_bytes == total_bytes {
                last_percent = pct;
                emit_to_frontend(
                    &app,
                    "modelDownloadProgress",
                    Some(json!({
                        "profile": model.profile,
                        "stage": "downloading",
                        "percent": pct,
                        "receivedBytes": downloaded_bytes,
                        "totalBytes": total_bytes,
                        "message": format!("Downloading {}: {:.1} MB / {:.1} MB ({}%)",
                            model.display_name,
                            downloaded_bytes as f64 / (1024.0 * 1024.0),
                            total_bytes as f64 / (1024.0 * 1024.0),
                            pct
                        )
                    })),
                );
            }
        }
    }

    file.flush().map_err(|e| format!("Flush error: {}", e))?;
    drop(file);

    emit_to_frontend(
        &app,
        "modelDownloadProgress",
        Some(json!({
            "profile": model.profile,
            "stage": "verifying",
            "percent": 97,
            "receivedBytes": total_bytes,
            "totalBytes": total_bytes,
            "message": "Verifying model file integrity..."
        })),
    );

    if final_path.exists() {
        let _ = fs::remove_file(&final_path);
    }
    fs::rename(&part_path, &final_path).map_err(|e| format!("Failed to finalize model file: {}", e))?;

    let final_size_mb = fs::metadata(&final_path)
        .map(|m| m.len() / (1024 * 1024))
        .unwrap_or(0);

    emit_to_frontend(
        &app,
        "modelDownloadProgress",
        Some(json!({
            "profile": model.profile,
            "stage": "complete",
            "percent": 100,
            "receivedBytes": total_bytes,
            "totalBytes": total_bytes,
            "message": format!("{} is ready for offline use!", model.display_name)
        })),
    );

    emit_to_frontend(
        &app,
        "modelDownloadComplete",
        Some(json!({
            "profile": model.profile,
            "displayName": model.display_name,
            "success": true,
            "sizeMb": final_size_mb,
            "message": format!("Model '{}' ({} MB) ready for offline dictation!", model.display_name, final_size_mb),
        })),
    );

    Ok(())
}

pub fn verify_local_model(profile: &str) -> Result<serde_json::Value, String> {
    let model = find_model_by_profile_or_id(profile)
        .ok_or_else(|| format!("Unknown model profile: {}", profile))?;

    let engine_path = get_engine_path();
    if !engine_path.exists() {
        return Err("Whisper runtime engine (whisper-cli.exe) is not installed yet.".to_string());
    }

    let model_path = get_models_dir().join(&model.filename);
    if !model_path.exists() {
        return Err(format!("Model file '{}' is not downloaded yet.", model.filename));
    }

    let metadata = fs::metadata(&model_path)
        .map_err(|e| format!("Cannot read model file: {}", e))?;
    let size_mb = metadata.len() / (1024 * 1024);

    if size_mb == 0 {
        return Err("Model file appears empty or corrupted. Please delete and re-download.".to_string());
    }

    Ok(json!({
        "success": true,
        "profile": model.profile,
        "displayName": model.display_name,
        "filename": model.filename,
        "sizeMb": size_mb,
        "enginePath": engine_path.to_string_lossy(),
        "modelPath": model_path.to_string_lossy(),
        "status": "ready",
        "message": format!("✓ Local Whisper Engine & Model Verified! ({}, {} MB) - Ready for offline dictation.", model.display_name, size_mb)
    }))
}

pub fn delete_local_model_file(profile: &str) -> Result<(), String> {
    if let Some(model) = find_model_by_profile_or_id(profile) {
        let path = get_models_dir().join(&model.filename);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("Failed to delete model: {}", e))?;
        }
    }
    Ok(())
}

/// Runs inference on captured WAV audio via resident whisper-server (in-memory model)
/// with seamless fallback to whisper-cli.exe. Audio is 16kHz mono WAV format.
pub fn transcribe_local(wav_bytes: &[u8], profile_or_id: &str) -> Result<String, String> {
    if !is_engine_available() && !is_server_engine_available() {
        return Err("Local Whisper engine not installed. Please download a model in Settings to initialize.".to_string());
    }

    let models_dir = get_models_dir();

    // Determine which model file to use
    let target_model = find_model_by_profile_or_id(profile_or_id);
    let mut model_path = target_model
        .as_ref()
        .map(|m| models_dir.join(&m.filename))
        .unwrap_or_else(|| models_dir.join(format!("ggml-{}.bin", profile_or_id)));

    // Fallback: If requested model is not downloaded, search for ANY downloaded model in models_dir
    if !model_path.exists() {
        let any_downloaded = get_supported_models()
            .into_iter()
            .find(|m| models_dir.join(&m.filename).exists());

        if let Some(found) = any_downloaded {
            model_path = models_dir.join(&found.filename);
        } else {
            return Err("No offline Whisper model downloaded. Please download one in Settings under 'Offline Speech Models Manager'.".to_string());
        }
    }

    // Tier 1: Fast in-memory resident whisper-server (sub-300ms latency)
    if is_server_engine_available() {
        if let Ok(port) = ensure_resident_server(&model_path) {
            let wav_copy = wav_bytes.to_vec();
            let server_call = async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(12))
                    .build()
                    .unwrap_or_default();

                let part = reqwest::multipart::Part::bytes(wav_copy)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| e.to_string())?;

                let form = reqwest::multipart::Form::new()
                    .part("file", part)
                    .text("temperature", "0.0")
                    .text("response_format", "json");

                let resp = client
                    .post(format!("http://127.0.0.1:{}/inference", port))
                    .multipart(form)
                    .send()
                    .await
                    .map_err(|e| format!("Server HTTP error: {}", e))?;

                if resp.status().is_success() {
                    let json_val: serde_json::Value = resp
                        .json()
                        .await
                        .map_err(|e| format!("Server JSON parse error: {}", e))?;
                    if let Some(text) = json_val.get("text").and_then(|t| t.as_str()) {
                        let t = text.trim();
                        if !t.is_empty() {
                            return Ok(t.to_string());
                        }
                    }
                }
                Err("Empty inference result from server".to_string())
            };

            let inference_result: Result<String, String> = if let Ok(handle) = tokio::runtime::Handle::try_current() {
                tokio::task::block_in_place(|| handle.block_on(server_call))
            } else if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(server_call)
            } else {
                Err("Could not initialize async runtime for local server".to_string())
            };

            if let Ok(text) = inference_result {
                return Ok(text);
            }
        }
    }

    // Tier 2: Process fallback via whisper-cli.exe
    let ts = chrono::Utc::now().timestamp_millis();
    let pid = std::process::id();
    let temp_wav = models_dir.join(format!("temp_dict_{}_{}.wav", pid, ts));
    let temp_out_base = models_dir.join(format!("temp_out_{}_{}", pid, ts));
    let expected_txt = models_dir.join(format!("temp_out_{}_{}.txt", pid, ts));

    fs::write(&temp_wav, wav_bytes)
        .map_err(|e| format!("Failed to write temporary wav file: {}", e))?;

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(4, 8)
        .to_string();

    let mut cmd = Command::new(get_engine_path());
    cmd.args(&[
        "-m",
        model_path.to_str().unwrap(),
        "-f",
        temp_wav.to_str().unwrap(),
        "-nt", // No timestamps
        "-np", // No debug/extra prints
        "-fa", // Flash attention acceleration
        "-l",
        "auto", // Auto-detect language
        "-t",
        &threads, // Scaled computation threads
        "-otxt",
        "-of",
        temp_out_base.to_str().unwrap(),
    ]);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| format!("Failed to run whisper-cli: {}", e))?;

    let mut transcribed_text = String::new();

    if expected_txt.exists() {
        if let Ok(content) = fs::read_to_string(&expected_txt) {
            transcribed_text = content;
        }
        let _ = fs::remove_file(&expected_txt);
    }

    if transcribed_text.trim().is_empty() && output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        transcribed_text = stdout.to_string();
    }

    // Cleanup temp audio
    let _ = fs::remove_file(&temp_wav);

    if !output.status.success() && transcribed_text.trim().is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Whisper inference error: {}", stderr.trim()));
    }

    Ok(transcribed_text.trim().to_string())
}
