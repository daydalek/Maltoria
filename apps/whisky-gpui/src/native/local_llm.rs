//! Local translation engine management for Windows: downloads the pinned
//! llama.cpp server binary and catalog GGUF models, runs `llama-server` on
//! loopback, and translates through its OpenAI-compatible endpoint. Mirrors
//! the Swift LocalTranslationRuntime/Store design so the frontend wire shapes
//! stay identical across platforms.

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};

use crate::native::store::Store;

const PINNED_VERSION: &str = "b12540";
const RUNTIME_SIZE: u64 = 24_812_775;
// SHA-256 of llama-b12540-bin-win-cpu-x64.zip, pinned after first-party verification.
const RUNTIME_SHA256: &str = "a99b3f8d90956610186232815f43414967d3ded08dd509b0e5f26c9459742647";
const IDLE_TIMEOUT_SECS: u64 = 300;
const HEALTH_TIMEOUT_SECS: u64 = 120;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

struct ModelEntry {
    id: &'static str,
    display_name: &'static str,
    subtitle: &'static str,
    recommended: bool,
    repo_id: &'static str,
    filename: &'static str,
    size_bytes: u64,
    sha256: &'static str,
    context_window: u64,
    recommended_ram_gb: u64,
    license_name: &'static str,
    license_url: &'static str,
    server_context_window: u64,
}

const MODELS: &[ModelEntry] = &[
    ModelEntry {
        id: "plamo-3-translate-2b-q4_k_m",
        display_name: "PLaMo 3 Translate 2B (Q4_K_M)",
        subtitle: "推荐主力 · 日中准确度、人称指代与语境理解更强（运行约占 2.2~2.7 GB 内存）",
        recommended: true,
        repo_id: "pfnet/plamo-3-translate-2b-GGUF",
        filename: "plamo-3-translate-2b-Q4_K_M.gguf",
        size_bytes: 1_807_203_040,
        sha256: "1ffd9073afcc70e35ccc942428aa2b73d7aa2b9ea779bf504b775369e0276c13",
        context_window: 32_768,
        recommended_ram_gb: 8,
        license_name: "PLaMo Community License",
        license_url: "https://plamo.preferredai.jp/info/plamo-community-license-en",
        server_context_window: 4_096,
    },
    ModelEntry {
        id: "plamo-3-translate-0.8b-q4_k_m",
        display_name: "PLaMo 3 Translate 0.8B (Q4_K_M)",
        subtitle: "轻量极速 · 下载更快、延迟更低，适合短句或低内存设备（运行约占 1.0 GB 内存）",
        recommended: false,
        repo_id: "pfnet/plamo-3-translate-0.8b-GGUF",
        filename: "plamo-3-translate-0.8b-Q4_K_M.gguf",
        size_bytes: 573_126_848,
        sha256: "70891ec70dc7ab4205745a38307867c2881b99239f20327927633abfe4bc9ec6",
        context_window: 32_768,
        recommended_ram_gb: 4,
        license_name: "PLaMo Community License",
        license_url: "https://plamo.preferredai.jp/info/plamo-community-license-en",
        server_context_window: 4_096,
    },
    ModelEntry {
        id: "hy-mt3-1.8b-q4_k_m",
        display_name: "Hy-MT3 1.8B (Q4_K_M)",
        subtitle: "腾讯混元 MT3 · 覆盖 102 种语言，指令遵循与上下文理解更强（运行约占 1.8~2.3 GB 内存）",
        recommended: false,
        repo_id: "tencent/Hy-MT3-1.8B-GGUF",
        filename: "Hy-MT3-1.8B-Q4_K_M.gguf",
        size_bytes: 1_348_432_160,
        sha256: "bae6e591103654ff43ef0de7d1492f6e6df1f955505f6bb2cc52e86fc399a942",
        context_window: 8_192,
        recommended_ram_gb: 8,
        license_name: "Apache-2.0",
        license_url: "https://huggingface.co/tencent/Hy-MT3-1.8B-GGUF/blob/main/LICENSE.txt",
        server_context_window: 8_192,
    },
];

fn model_by_id(id: &str) -> Option<&'static ModelEntry> {
    MODELS.iter().find(|model| model.id == id)
}

pub(crate) struct LocalState {
    pub(crate) use_china_mirror: bool,
    pub(crate) download: Option<DownloadState>,
    pub(crate) server: Option<ServerState>,
    pub(crate) last_error: Option<String>,
}

pub(crate) struct DownloadState {
    pub(crate) model_id: String,
    pub(crate) stage: String,
    pub(crate) downloaded_bytes: u64,
    pub(crate) total_bytes: u64,
    pub(crate) bytes_per_second: u64,
    pub(crate) cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub(crate) struct ServerState {
    pub(crate) child: std::process::Child,
    pub(crate) port: u16,
    pub(crate) api_key: String,
    pub(crate) model_id: String,
    pub(crate) last_activity: std::time::Instant,
}

fn state() -> &'static Mutex<LocalState> {
    static STATE: OnceLock<Mutex<LocalState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(LocalState {
            use_china_mirror: false,
            download: None,
            server: None,
            last_error: None,
        })
    })
}

fn root(store: &Store) -> PathBuf {
    store.root.join("local-translation")
}

fn runtime_dir(store: &Store) -> PathBuf {
    root(store).join("runtime")
}

fn server_binary(store: &Store) -> PathBuf {
    runtime_dir(store).join("llama-server.exe")
}

fn model_path(store: &Store, entry: &ModelEntry) -> PathBuf {
    root(store).join("models").join(entry.filename)
}

fn hf_base(use_china_mirror: bool) -> &'static str {
    if use_china_mirror {
        "https://hf-mirror.com"
    } else {
        "https://huggingface.co"
    }
}

fn runtime_installed(store: &Store) -> bool {
    server_binary(store).is_file()
}

fn sha256_file(path: &Path) -> Option<String> {
    use sha2::Digest;
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn download_file(url: &str, dest: &Path, expected_sha: Option<&str>, progress: &mut dyn FnMut(u64, u64), cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|error| format!("download failed: {error}"))?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let scratch = dest.with_extension("part");
    if let Some(parent) = scratch.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut out = std::fs::File::create(&scratch).map_err(|error| error.to_string())?;
    let mut reader = response.into_body().into_reader();
    let mut buffer = [0u8; 262144];
    let mut downloaded = 0u64;
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            drop(out);
            let _ = std::fs::remove_file(&scratch);
            return Err("cancelled".into());
        }
        let read = reader.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        use std::io::Write;
        out.write_all(&buffer[..read]).map_err(|error| error.to_string())?;
        downloaded += read as u64;
        progress(downloaded, total);
    }
    drop(out);
    if let Some(expected_hash) = expected_sha {
        if !expected_hash.is_empty() {
            let actual = sha256_file(&scratch).unwrap_or_default();
            if actual != expected_hash {
                let _ = std::fs::remove_file(&scratch);
                return Err(format!("sha256 mismatch: got {actual}"));
            }
        }
    }
    std::fs::rename(&scratch, dest).map_err(|error| error.to_string())?;
    Ok(())
}

fn extract_zip(zip: &Path, dest: &Path) -> Result<(), String> {
    let _ = std::fs::create_dir_all(dest);
    // bsdtar ships with Windows and handles zip fine.
    let status = std::process::Command::new("tar")
        .arg("-xf")
        .arg(zip)
        .arg("-C")
        .arg(dest)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|error| format!("tar failed to start: {error}"))?;
    if !status.success() {
        return Err(format!("tar exited with {status}"));
    }
    Ok(())
}

fn find_server_exe(dir: &Path) -> Option<PathBuf> {
    // The zip nests binaries; locate llama-server.exe wherever it landed.
    for entry in walkdir(dir, 3) {
        if entry.file_name().is_some_and(|name| name == "llama-server.exe") {
            return Some(entry);
        }
    }
    None
}

fn walkdir(root: &Path, depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if depth == 0 {
        return out;
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            out.push(path.clone());
            if path.is_dir() {
                out.extend(walkdir(&path, depth - 1));
            }
        }
    }
    out
}

fn ensure_runtime(store: &Store, cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<PathBuf, String> {
    let binary = server_binary(store);
    if binary.is_file() {
        return Ok(binary);
    }
    {
        let mut guard = state().lock().unwrap();
        if let Some(download) = &mut guard.download {
            download.stage = "runtime".into();
        }
    }
    let url = format!(
        "https://github.com/ggml-org/llama.cpp/releases/download/{PINNED_VERSION}/llama-{PINNED_VERSION}-bin-win-cpu-x64.zip"
    );
    let zip_path = runtime_dir(store).join("llama-runtime.zip");
    download_file(&url, &zip_path, None, &mut |downloaded, _total| {
        let mut guard = state().lock().unwrap();
        if let Some(download) = &mut guard.download {
            if download.stage == "runtime" {
                download.downloaded_bytes = downloaded;
            }
        }
    }, cancel)?;
    let extracted = runtime_dir(store).join("extracted");
    let _ = std::fs::remove_dir_all(&extracted);
    extract_zip(&zip_path, &extracted)?;
    let found = find_server_exe(&extracted).ok_or("llama-server.exe not in archive")?;
    let source_dir = found.parent().ok_or("invalid layout")?.to_path_buf();
    // Keep the whole binary directory: llama-server needs its sibling DLLs.
    let target = runtime_dir(store);
    for entry in walkdir(&source_dir, 1) {
        if entry.is_file()
            && let Some(name) = entry.file_name() {
                let _ = std::fs::copy(&entry, target.join(name));
            }
    }
    let _ = std::fs::remove_dir_all(&extracted);
    let _ = std::fs::remove_file(&zip_path);
    if !binary.is_file() {
        return Err("runtime extraction failed".into());
    }
    Ok(binary)
}

pub(crate) fn local(store: &Store, method: &str, params: &Value) -> Result<Value, (String, String)> {
    match method {
        "translation.local.status" => Ok(status_json(store)),
        "translation.local.setConfig" => {
            if let Some(value) = params.get("useChinaMirror").and_then(Value::as_bool) {
                state().lock().unwrap().use_china_mirror = value;
            }
            Ok(status_json(store))
        }
        "translation.local.download" => {
            let model_id = params
                .get("modelID")
                .or_else(|| params.get("modelId"))
                .or_else(|| params.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let entry = model_by_id(&model_id)
                .or_else(|| MODELS.first())
                .ok_or_else(|| ("invalid_model".into(), "unknown model".into()))?;
            if state().lock().unwrap().download.is_some() {
                return Err(("download_busy".into(), "a download is already running".into()));
            }
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            {
                let mut guard = state().lock().unwrap();
                guard.download = Some(DownloadState {
                    model_id: entry.id.into(),
                    stage: "runtime".into(),
                    downloaded_bytes: 0,
                    total_bytes: RUNTIME_SIZE + entry.size_bytes,
                    bytes_per_second: 0,
                    cancel: cancel.clone(),
                });
                guard.last_error = None;
            }
            let root = store.root.clone();
            let use_mirror = state().lock().unwrap().use_china_mirror;
            let entry_id = entry.id;
            std::thread::spawn(move || {
                let store = Store::load_at(&root);
                let entry = model_by_id(entry_id).expect("catalog entry");
                let outcome = run_download(&store, entry, use_mirror, &cancel);
                let mut guard = state().lock().unwrap();
                if let Err(reason) = outcome {
                    if reason != "cancelled" {
                        guard.last_error = Some(reason);
                    }
                }
                guard.download = None;
            });
            Ok(json!({ "started": true }))
        }
        "translation.local.cancelDownload" => {
            let mut guard = state().lock().unwrap();
            if let Some(download) = &guard.download {
                download.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            guard.download = None;
            Ok(json!({ "cancelled": true }))
        }
        "translation.local.delete" => {
            let model_id = params
                .get("modelID")
                .or_else(|| params.get("modelId"))
                .and_then(Value::as_str)
                .unwrap_or("");
            unload_server();
            let mut removed = false;
            if let Some(entry) = model_by_id(model_id) {
                let path = model_path(store, entry);
                if path.is_file() {
                    let _ = std::fs::remove_file(&path);
                    removed = true;
                }
            }
            Ok(json!({ "deleted": removed }))
        }
        "translation.local.unload" => {
            unload_server();
            Ok(json!({ "unloaded": true }))
        }
        _ => Err(("method_unavailable".into(), format!("{method} is not implemented"))),
    }
}

fn run_download(store: &Store, entry: &ModelEntry, use_mirror: bool, cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<(), String> {
    ensure_runtime(store, cancel)?;
    let dest = model_path(store, entry);
    if dest.is_file() {
        return Ok(());
    }
    {
        let mut guard = state().lock().unwrap();
        if let Some(download) = &mut guard.download {
            download.stage = "model".into();
            download.downloaded_bytes = 0;
        }
    }
    let url = format!(
        "{}/{}/resolve/main/{}?download=true",
        hf_base(use_mirror),
        entry.repo_id,
        entry.filename
    );
    let started = std::time::Instant::now();
    download_file(&url, &dest, Some(entry.sha256), &mut |downloaded, _total| {
        let elapsed = started.elapsed().as_secs().max(1);
        let mut guard = state().lock().unwrap();
        if let Some(download) = &mut guard.download {
            if download.stage == "model" {
                download.downloaded_bytes = downloaded;
                download.bytes_per_second = downloaded / elapsed;
            }
        }
    }, cancel)
}

fn status_json(store: &Store) -> Value {
    let guard = state().lock().unwrap();
    let runtime_installed = runtime_installed(store);
    let server_running = guard
        .server
        .as_ref()
        .is_some_and(|server| server.child.id() > 0);
    let models: Vec<Value> = MODELS
        .iter()
        .map(|entry| {
            let path = model_path(store, entry);
            let installed = path.is_file();
            let installed_size = if installed {
                std::fs::metadata(&path).map(|meta| meta.len()).ok()
            } else {
                None
            };
            let download = guard
                .download
                .as_ref()
                .filter(|download| download.model_id == entry.id)
                .map(|download| {
                    json!({
                        "stage": download.stage,
                        "downloadedBytes": download.downloaded_bytes,
                        "totalBytes": download.total_bytes,
                        "bytesPerSecond": download.bytes_per_second,
                        "etaSeconds": Value::Null,
                        "jobID": "local-download",
                    })
                });
            json!({
                "id": entry.id,
                "displayName": entry.display_name,
                "subtitle": entry.subtitle,
                "recommended": entry.recommended,
                "sizeBytes": entry.size_bytes,
                "installed": installed,
                "installedSizeBytes": installed_size,
                "contextWindow": entry.context_window,
                "recommendedRamGb": entry.recommended_ram_gb,
                "licenseName": entry.license_name,
                "licenseUrl": entry.license_url,
                "loaded": guard.server.as_ref().is_some_and(|server| server.model_id == entry.id),
                "download": download,
                "lastError": Value::Null,
            })
        })
        .collect();
    json!({
        "useChinaMirror": guard.use_china_mirror,
        "hfEndpoint": hf_base(guard.use_china_mirror),
        "hfEndpointOverride": Value::Null,
        "runtime": {
            "installed": runtime_installed,
            "version": if runtime_installed { Value::from(PINNED_VERSION) } else { Value::Null },
            "pinnedVersion": PINNED_VERSION,
            "sizeBytes": RUNTIME_SIZE,
            "gpuAcceleration": "cpu",
            "running": server_running,
            "loadedModelID": guard.server.as_ref().map(|server| json!(server.model_id)).unwrap_or(Value::Null),
            "lastError": guard.last_error,
        },
        "models": models,
    })
}

fn unload_server() {
    let server = state().lock().unwrap().server.take();
    if let Some(mut server) = server {
        let _ = server.child.kill();
    }
}

fn free_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|listener| listener.local_addr().ok())
        .map(|addr| addr.port())
}

fn random_key() -> String {
    format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos())
}

/// Ensure the inference server is running for the model, starting it when
/// needed. Returns (base_url, api_key). Blocking; call off the dispatch path.
pub(crate) fn ensure_server(store: &Store, model_id: &str) -> Result<(String, String), (String, String)> {
    // Fast path: same model already running.
    {
        let mut guard = state().lock().unwrap();
        if let Some(server) = &mut guard.server {
            if server.model_id == model_id {
                server.last_activity = std::time::Instant::now();
                return Ok((format!("http://127.0.0.1:{}", server.port), server.api_key.clone()));
            }
        }
    }
    unload_server();
    let entry = model_by_id(model_id).unwrap_or(&MODELS[0]);
    let binary = server_binary(store);
    if !binary.is_file() {
        return Err(("local_not_ready".into(), "local inference engine is not installed".into()));
    }
    let model_file = model_path(store, entry);
    if !model_file.is_file() {
        return Err(("local_model_missing".into(), "local model is not downloaded".into()));
    }
    let port = free_port().ok_or_else(|| ("local_error".into(), "no free port".into()))?;
    let key = random_key();
    let context = entry.server_context_window;
    let child = std::process::Command::new(&binary)
        .current_dir(runtime_dir(store))
        .args([
            "-m", &model_file.to_string_lossy(),
            "--host", "127.0.0.1",
            "--port", &port.to_string(),
            "--api-key", &key,
            "-c", &context.to_string(),
            "-np", "1",
            "--no-webui",
            "-ngl", "0",
            "--jinja",
            "--no-prefill-assistant",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| ("local_start_failed".into(), format!("cannot start llama-server: {error}")))?;
    {
        let mut guard = state().lock().unwrap();
        guard.server = Some(ServerState {
            child,
            port,
            api_key: key.clone(),
            model_id: model_id.to_owned(),
            last_activity: std::time::Instant::now(),
        });
    }
    // Wait for /health, bounded.
    let health = format!("http://127.0.0.1:{port}/health");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(HEALTH_TIMEOUT_SECS);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let alive = {
            let mut guard = state().lock().unwrap();
            guard.server.as_mut().is_some_and(|server| server.child.try_wait().ok().flatten().is_none())
        };
        if !alive {
            return Err(("local_start_failed".into(), "llama-server exited during startup".into()));
        }
        if ureq::get(&health).call().is_ok() {
            // Idle reaper: unload after IDLE_TIMEOUT_SECS without activity.
            std::thread::spawn(|| loop {
                std::thread::sleep(std::time::Duration::from_secs(30));
                let idle = {
                    let guard = state().lock().unwrap();
                    guard.server.as_ref().map(|server| server.last_activity.elapsed().as_secs())
                };
                match idle {
                    Some(secs) if secs > IDLE_TIMEOUT_SECS => {
                        unload_server();
                        break;
                    }
                    None => break,
                    _ => {}
                }
            });
            return Ok((format!("http://127.0.0.1:{port}"), key));
        }
        if std::time::Instant::now() > deadline {
            unload_server();
            return Err(("local_start_failed".into(), "llama-server health check timed out".into()));
        }
    }
}

/// Translate through the local server using the OpenAI-compatible endpoint.
pub(crate) fn translate_local(
    store: &Store,
    text: &str,
    source_language: &str,
    target_language: &str,
    custom_prompt: &str,
) -> Result<String, (String, String)> {
    let model_id = {
        let guard = state().lock().unwrap();
        guard.server.as_ref().map(|server| server.model_id.clone())
    };
    // Default to the first installed model when nothing is loaded yet.
    let model_id = model_id.unwrap_or_else(|| {
        MODELS
            .iter()
            .find(|entry| model_path(store, entry).is_file())
            .map(|entry| entry.id.to_owned())
            .unwrap_or_else(|| MODELS[0].id.to_owned())
    });
    let (base, key) = ensure_server(store, &model_id)?;
    let body = if model_id.starts_with("plamo") {
        // PLaMo 3 Translate speaks the PFML3 format through the embedded chat
        // template: the current source text goes in the last user message and
        // the languages in chat_template_kwargs (lowercase zh-hans style
        // codes). A system message not starting with [plamo-translate] is
        // treated as reference context, which is where a custom prompt
        // belongs. Docs: pfnet/plamo-3-translate-*-GGUF README.
        let plamo_code = |code: &str| code.trim().to_lowercase().replace('_', "-");
        let target = plamo_code(target_language);
        let source = plamo_code(source_language);
        let mut kwargs = json!({ "target_lang": target });
        if !source.is_empty() && source != "auto" {
            kwargs["source_lang"] = json!(source);
        }
        let mut messages = Vec::new();
        if !custom_prompt.trim().is_empty() {
            messages.push(json!({ "role": "system", "content": custom_prompt.trim() }));
        }
        messages.push(json!({ "role": "user", "content": text }));
        json!({
            "model": model_id,
            "messages": messages,
            "chat_template_kwargs": kwargs,
            "temperature": 0,
            "max_tokens": 512,
            "stop": ["<|plamo:tag|>", "<|plamo:bos|>"],
        })
    } else {
        let instruction = if custom_prompt.trim().is_empty() {
            format!("Translate the following text from {source_language} to {target_language}. Output only the translation.")
        } else {
            format!("{custom_prompt}\n\nTranslate from {source_language} to {target_language}. Output only the translation.")
        };
        json!({
            "model": model_id,
            "messages": [
                { "role": "system", "content": instruction },
                { "role": "user", "content": text },
            ],
            "temperature": 0.3,
        })
    };
    let response = ureq::post(&format!("{base}/v1/chat/completions"))
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {key}"))
        .send_json(&body)
        .map_err(|error| ("network".into(), error.to_string()));
    match response {
        Ok(mut response) => {
            {
                let mut guard = state().lock().unwrap();
                if let Some(server) = &mut guard.server {
                    server.last_activity = std::time::Instant::now();
                }
            }
            let body: Value = response.body_mut().read_json().map_err(|error| ("network".into(), error.to_string()))?;
            body.pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty())
                .ok_or_else(|| ("translation_failed".into(), "empty completion".into()))
        }
        Err(error) => Err(error),
    }
}
