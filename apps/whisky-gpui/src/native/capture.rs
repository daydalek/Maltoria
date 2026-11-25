//! Windows-native text capture: drives the MaltoriaLunaHost helper over the
//! documented loopback NDJSON protocol, selects text threads, runs captured
//! text through the translation pipeline, and replicates the whole session
//! as one `translation.snapshot` the frontend polls.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::native::store::Store;

const MAX_FRAME: usize = 64 * 1024;

fn payload_root() -> PathBuf {
    if let Some(custom) = std::env::var_os("MALTORIA_LUNAHOOK_ARTIFACTS") {
        return PathBuf::from(custom);
    }
    PathBuf::from(r"C:\MaltoriaNative")
}

fn now_seconds() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_id() -> String {
    format!("req-{}", REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst))
}

fn random_token() -> String {
    format!(
        "{:x}{:x}",
        now_seconds() as u64 ^ (std::process::id() as u64) << 17,
        REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst) ^ 0x9e3779b97f4a7c15
    )
}

/// One attached helper connection: the writer half plus the session secrets.
struct HelperLink {
    writer: Arc<Mutex<TcpStream>>,
    session: String,
}

pub(crate) struct CaptureService {
    pub(crate) inner: Arc<Mutex<CaptureInner>>,
}

pub(crate) struct CaptureInner {
    pub(crate) capture_state: String,
    pub(crate) last_error: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) game_id: Option<String>,
    pub(crate) container: Option<String>,
    pub(crate) entry_id: Option<String>,
    pub(crate) provider_id: Option<String>,
    pub(crate) architecture: String,
    pub(crate) target: Option<Value>,
    pub(crate) processes: Vec<Value>,
    pub(crate) processes_truncated: bool,
    pub(crate) processes_generation: u64,
    /// The helper is single-use per the protocol: once a target was attached
    /// (or exited), the channel must be recycled before the next attach.
    pub(crate) helper_tainted: bool,
    pub(crate) threads: BTreeMap<String, Value>,
    /// thread key → role ("body" | "speaker" | "ignore"); absent = unselected.
    pub(crate) roles: BTreeMap<String, String>,
    pub(crate) sequences: BTreeMap<String, i64>,
    /// thread key → last text forwarded to the pipeline. Redraw spam is the
    /// norm in visual novels; only consecutive repeats on the same thread are
    /// noise. Reset with every session, unlike the persistent history.
    pub(crate) last_texts: BTreeMap<String, String>,
    pub(crate) subtitle_entries: Vec<Value>,
    pub(crate) history: Vec<Value>,
    pub(crate) cursor: u64,
    pub(crate) helper: Option<HelperLink>,
    pub(crate) helper_child: Option<std::process::Child>,
    pub(crate) listener_port: Option<u16>,
    pub(crate) token: String,
    pub(crate) pending_commands: BTreeMap<String, String>,
    pub(crate) selected_thread: Option<String>,
    pub(crate) overlay: Value,
    pub(crate) presets: BTreeMap<String, Value>,
    pub(crate) store_root: PathBuf,
}

fn default_overlay() -> Value {
    json!({
        "visible": true,
        "clickThrough": false,
        "showSource": true,
        "showTranslation": true,
        "maxEntries": 3,
        "fontSize": 16.0,
        "opacity": 0.85,
        "width": 480.0,
        "height": 160.0,
        "x": null,
        "y": null,
        "textColor": "#FFFFFF",
        "sourceTextColor": "#AAAAAA",
    })
}

impl CaptureService {
    pub(crate) fn new(store: &Store) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CaptureInner {
                capture_state: "stopped".into(),
                last_error: None,
                session_id: None,
                game_id: None,
                container: None,
                entry_id: None,
                provider_id: None,
                architecture: "x64".into(),
                target: None,
                processes: Vec::new(),
                processes_truncated: false,
                processes_generation: 0,
                helper_tainted: false,
                threads: BTreeMap::new(),
                roles: BTreeMap::new(),
                sequences: BTreeMap::new(),
                last_texts: BTreeMap::new(),
                subtitle_entries: Vec::new(),
                history: Vec::new(),
                cursor: 0,
                helper: None,
                helper_child: None,
                listener_port: None,
                token: String::new(),
                pending_commands: BTreeMap::new(),
                selected_thread: None,
                overlay: default_overlay(),
                presets: BTreeMap::new(),
                store_root: store.root.clone(),
            })),
        }
    }

    pub(crate) fn shared(&self) -> Arc<Mutex<CaptureInner>> {
        Arc::clone(&self.inner)
    }
}


fn log_secs() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

fn send_command(inner: &mut CaptureInner, command: &str, params: Value) -> Result<(), String> {
    let Some(link) = &inner.helper else {
        return Err("no_helper".into());
    };
    let id = next_id();
    let frame = json!({
        "type": "command",
        "id": id,
        "session": link.session,
        "command": command,
        "params": params,
    });
    let mut line = serde_json::to_string(&frame).map_err(|error| error.to_string())?;
    line.push('\n');
    let writer = Arc::clone(&link.writer);
    // Register before writing: the reader thread can otherwise process the
    // response before the id is known and drop it as unsolicited.
    inner.pending_commands.insert(id.clone(), command.to_owned());
    let write_result = writer
        .lock()
        .map_err(|_| "locked".to_string())
        .and_then(|mut guard| guard.write_all(line.as_bytes()).map_err(|error| error.to_string()));
    if let Err(error) = write_result {
        eprintln!("capture[{:.3}]: send {command} failed: {error}", log_secs());
        inner.pending_commands.remove(&id);
        return Err(error);
    }
    eprintln!("capture[{:.3}]: sent {command}", log_secs());
    Ok(())
}

/// The accept + reader loop for one helper. Owns the TCP stream lifecycle and
/// feeds events back into the shared state.
fn pump_connection(
    stream: TcpStream,
    inner: Arc<Mutex<CaptureInner>>,
    expected_session: String,
    expected_token: String,
) {
    let _ = stream.set_nodelay(true);
    let writer_stream = match stream.try_clone() {
        Ok(clone) => clone,
        Err(_) => return,
    };
    let writer = Arc::new(Mutex::new(writer_stream));
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() || first.len() > MAX_FRAME {
        return;
    }
    let Ok(hello) = serde_json::from_str::<Value>(&first) else {
        return;
    };
    let valid = hello.get("type").and_then(Value::as_str) == Some("hello")
        && hello.get("token").and_then(Value::as_str) == Some(expected_token.as_str())
        && hello.get("session").and_then(Value::as_str) == Some(expected_session.as_str())
        && hello.get("version").and_then(Value::as_i64) == Some(1);
    eprintln!("capture[{:.3}]: hello valid={valid}", log_secs());
    if !valid {
        return;
    }
    {
        let mut guard = inner.lock().unwrap();
        guard.helper = Some(HelperLink {
            writer: Arc::clone(&writer),
            session: expected_session.clone(),
        });
        eprintln!("capture[{:.3}]: helper link established", log_secs());
    }
    // Acknowledge the handshake.
    {
        let ack = json!({
            "type": "hello",
            "version": 1,
            "session": expected_session,
            "ok": true,
        });
        let mut line = serde_json::to_string(&ack).unwrap_or_default();
        line.push('\n');
        if let Ok(mut guard) = writer.lock() {
            let _ = guard.write_all(line.as_bytes());
        }
    }
    // Heartbeat: the helper expects a ping at least every 15 seconds.
    {
        let inner = Arc::clone(&inner);
        let writer = Arc::clone(&writer);
        let expected_session = expected_session.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(10));
            let alive = inner.lock().unwrap().helper.is_some();
            if !alive {
                break;
            }
            let id = next_id();
            let frame = json!({
                "type": "command",
                "id": id,
                "session": expected_session,
                "command": "ping",
            });
            let mut line = serde_json::to_string(&frame).unwrap_or_default();
            line.push('\n');
            let Ok(mut guard) = writer.lock() else { break };
            if guard.write_all(line.as_bytes()).is_err() {
                break;
            }
            eprintln!("capture[{:.3}]: ping sent", log_secs());
        });
    }
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if line.len() > MAX_FRAME {
                    eprintln!("capture[{:.3}]: oversized frame {} bytes", log_secs(), line.len());
                    break;
                }
                let Ok(frame) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                let kind = frame.get("type").and_then(Value::as_str).unwrap_or("?");
                if kind != "text" {
                    eprintln!("capture[{:.3}]: frame {kind}", log_secs());
                }
                handle_frame(&inner, &frame);
            }
        }
    }
    eprintln!("capture[{:.3}]: reader loop ended", log_secs());
    // EOF or error: drop the link unless a newer helper replaced it.
    let mut guard = inner.lock().unwrap();
    if guard
        .helper
        .as_ref()
        .is_some_and(|link| link.session == expected_session)
    {
        guard.helper = None;
        // targetExited is a terminal state of its own — the channel dying
        // afterwards is expected, not an error.
        if !matches!(guard.capture_state.as_str(), "stopped" | "targetExited") {
            guard.capture_state = "error".into();
            guard.last_error = Some("helper disconnected".into());
        }
    }
}

fn thread_key(thread: &Value) -> String {
    format!(
        "{}:{}:{}",
        thread.get("hookCode").and_then(Value::as_str).unwrap_or(""),
        thread.get("addressHex").and_then(Value::as_str).unwrap_or(""),
        thread.get("contextHex").and_then(Value::as_str).unwrap_or("")
    )
}

/// The helper speaks camelCase; the snapshot DTOs the frontend decodes are
/// snake_case. Normalize helper records once at ingestion.
fn normalize_process(record: &Value) -> Value {
    record.clone()
}

fn normalize_thread(thread: &Value) -> Value {
    thread.clone()
}

fn handle_frame(inner: &Arc<Mutex<CaptureInner>>, frame: &Value) {
    match frame.get("type").and_then(Value::as_str) {
        Some("response") => {
            let id = frame.get("id").and_then(Value::as_str).unwrap_or("");
            let command = {
                let mut guard = inner.lock().unwrap();
                eprintln!("capture[{:.3}]: response id={id} pending={:?}", log_secs(), guard.pending_commands.keys().collect::<Vec<_>>());
                guard.pending_commands.remove(id)
            };
            eprintln!("capture[{:.3}]: response id={id} resolved={command:?}", log_secs());
            let ok = frame.get("ok").and_then(Value::as_bool).unwrap_or(false);
            let result = frame.get("result").cloned().unwrap_or(Value::Null);
            match (command.as_deref(), ok) {
                (Some("listProcesses"), true) => {
                    eprintln!("capture[{:.3}]: listProcesses response received", log_secs());
                    let mut guard = inner.lock().unwrap();
                    guard.processes_generation += 1;
                    guard.processes = result
                        .get("processes")
                        .and_then(Value::as_array)
                        .map(|list| list.iter().map(normalize_process).collect())
                        .unwrap_or_default();
                    guard.processes_truncated = result
                        .get("truncated")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                }
                (Some("attach"), true) => {
                    let mut guard = inner.lock().unwrap();
                    guard.capture_state = "attached".into();
                    guard.helper_tainted = true;
                    if let Some(target) = result.get("target") {
                        guard.target = Some(normalize_process(target));
                    }
                }
                (Some("attach"), false) => {
                    let mut guard = inner.lock().unwrap();
                    guard.capture_state = "selecting".into();
                    guard.last_error = frame
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                }
                (Some("detach"), _) => {
                    let mut guard = inner.lock().unwrap();
                    guard.capture_state = "selecting".into();
                    guard.target = None;
                    guard.threads.clear();
                }
                _ => {}
            }
        }
        Some("event") | Some("process") | Some("thread") | Some("text") | Some("error") => {
            let event_type = frame.get("type").and_then(Value::as_str).unwrap_or("");
            match event_type {
                "process" => {
                    if frame.get("action").and_then(Value::as_str) == Some("disconnected") {
                        let mut guard = inner.lock().unwrap();
                        guard.capture_state = "targetExited".into();
                        guard.last_error = None;
                        guard.target = None;
                        guard.threads.clear();
                        // The attachment is gone; the helper cannot attach
                        // another target. Retire it so the next attach spawns
                        // a fresh channel.
                        guard.helper_tainted = true;
                        if let Some(mut child) = guard.helper_child.take() {
                            let _ = child.kill();
                        }
                    }
                }
                "thread" => {
                    eprintln!("capture[{:.3}]: thread event", log_secs());
                    let action = frame.get("action").and_then(Value::as_str).unwrap_or("");
                    if let Some(thread) = frame.get("thread").cloned() {
                        let thread = normalize_thread(&thread);
                        let key = thread_key(&thread);
                        let mut guard = inner.lock().unwrap();
                        if action == "created" {
                            guard.threads.insert(key, thread);
                        } else {
                            guard.threads.remove(&key);
                        }
                    }
                }
                "text" => {
                    let text = frame.get("text").and_then(Value::as_str).unwrap_or("");
                    let sequence = frame.get("sequence").and_then(Value::as_i64).unwrap_or(0);
                    if let Some(thread) = frame.get("thread").cloned() {
                        let thread = normalize_thread(&thread);
                        let key = thread_key(&thread);
                        let mut guard = inner.lock().unwrap();
                        let previous = guard.sequences.get(&key).copied().unwrap_or(0);
                        if sequence > previous {
                            guard.sequences.insert(key.clone(), sequence);
                            if let Some(existing) = guard.threads.get_mut(&key) {
                                existing["preview"] = json!(text.chars().take(80).collect::<String>());
                            } else {
                                let mut fresh = thread.clone();
                                fresh["preview"] = json!(text.chars().take(80).collect::<String>());
                                guard.threads.insert(key.clone(), fresh);
                            }
                            if guard.last_texts.get(&key).map(String::as_str) == Some(text) {
                                return;
                            }
                            let role = guard
                                .roles
                                .get(&key)
                                .cloned()
                                .unwrap_or_else(|| {
                                    if guard.roles.is_empty() {
                                        // First thread seen becomes the body
                                        // automatically, mirroring the Swift
                                        // auto-bind of the most active thread.
                                        guard.roles.insert(key.clone(), "body".into());
                                        "body".into()
                                    } else {
                                        String::new()
                                    }
                                });
                            if role != "ignore" && !role.is_empty() && !text.trim().is_empty() {
                                guard.last_texts.insert(key.clone(), text.to_owned());
                                let store_root = guard.store_root.clone();
                                let game_id = guard.game_id.clone().unwrap_or_default();
                                let provider_id = guard.provider_id.clone();
                                let speaker = if role == "speaker" { Some("".to_owned()) } else { None };
                                let inner = Arc::clone(inner);
                                let text = text.to_owned();
                                guard.cursor += 1;
                                let cursor = guard.cursor;
                                drop(guard);
                                std::thread::spawn(move || {
                                    translate_capture_text(inner, cursor, text, speaker, game_id, provider_id, store_root);
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn translate_capture_text(
    inner: Arc<Mutex<CaptureInner>>,
    cursor: u64,
    text: String,
    speaker: Option<String>,
    game_id: String,
    provider_id: Option<String>,
    store_root: PathBuf,
) {
    let store = Store::load_at(&store_root);
    let entry_id = {
        let mut guard = inner.lock().unwrap();
        let entry = json!({
            "id": format!("cap-{cursor}"),
            "source": {
                "eventID": format!("cap-{cursor}"),
                "revision": 0,
                "streamID": "hook",
                "generation": cursor,
                "text": text,
                "speaker": speaker,
                "capturedAt": now_seconds(),
            },
            "translatedText": Value::Null,
            "translationTruncated": false,
            "status": "translating",
            "includeInContext": true,
            "failureDescription": Value::Null,
            "createdAt": now_seconds(),
        });
        guard.history.push(entry.clone());
        if guard.history.len() > 200 {
            guard.history.remove(0);
        }
        entry.get("id").and_then(Value::as_str).unwrap_or("").to_owned()
    };
    let settings = crate::native::translation::settings(&store, "translation.settings/get", &json!({}))
        .unwrap_or_else(|_| json!({}));
    let target_language = settings["settings"]["targetLanguage"].as_str().unwrap_or("zh-Hans").to_owned();
    let source_language = settings["settings"]["sourceLanguage"].as_str().unwrap_or("auto").to_owned();
    let prompt = settings["settings"]["customPrompt"].as_str().unwrap_or("").to_owned();
    let cache_enabled = settings["settings"]["sentenceCacheEnabled"].as_bool().unwrap_or(true);
    // Sentence cache first.
    let cached = if cache_enabled {
        crate::native::translation::cache_lookup(&store, &text, &target_language)
    } else {
        None
    };
    let result = if let Some(hit) = cached {
        Ok(hit)
    } else {
        let provider_id = provider_id.unwrap_or_default();
        match crate::native::translation::find_provider_pub(&store, &provider_id) {
            Ok((provider, key)) => {
                let outcome = crate::native::translation::translate_with(
                    &store,
                    &provider,
                    key.as_deref(),
                    &text,
                    &source_language,
                    &target_language,
                    &prompt,
                );
                if let Ok(translated) = &outcome {
                    crate::native::translation::record_cache(
                        &store,
                        &game_id,
                        &text,
                        translated,
                        provider.get("name").and_then(Value::as_str).unwrap_or(""),
                        provider.get("kind").and_then(Value::as_str).unwrap_or(""),
                        provider.get("modelID").and_then(Value::as_str),
                        &source_language,
                        &target_language,
                    );
                }
                outcome
            }
            Err(error) => Err(error),
        }
    };
    let mut guard = inner.lock().unwrap();
    let (translated, status, failure) = match &result {
        Ok(text) => (Some(text.clone()), "translated", None),
        Err((_, message)) => (None, "failed", Some(message.clone())),
    };
    for entry in guard.history.iter_mut().rev() {
        if entry.get("id").and_then(Value::as_str) == Some(entry_id.as_str()) {
            entry["translatedText"] = translated.clone().map(Value::String).unwrap_or(Value::Null);
            entry["status"] = json!(status);
            entry["failureDescription"] = failure.clone().map(Value::String).unwrap_or(Value::Null);
            break;
        }
    }
    // Subtitle entries: replace the pending one or push a fresh record.
    let subtitle_entry = json!({
        "source": text,
        "translation": translated.unwrap_or_default(),
        "speaker": speaker,
        "status": status,
        "error": failure,
    });
    if let Some(pending) = guard.subtitle_entries.iter_mut().rev().find(|entry| {
        entry.get("source").and_then(Value::as_str) == Some(text.as_str())
            && entry.get("status").and_then(Value::as_str) == Some("translating")
    }) {
        *pending = subtitle_entry;
    } else {
        guard.subtitle_entries.push(subtitle_entry);
    }
    if guard.subtitle_entries.len() > 10 {
        guard.subtitle_entries.remove(0);
    }
}

fn spawn_helper(inner: &Arc<Mutex<CaptureInner>>, arch: &str) -> Result<(), String> {
    let bits = if arch == "x86" { "32" } else { "64" };
    let root = payload_root().join(arch);
    let helper_exe = root.join(format!("MaltoriaLunaHost{bits}.exe"));
    if !helper_exe.is_file() {
        return Err(format!("payload missing: {}", helper_exe.display()));
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    let token = random_token();
    let session = format!("capture-{}", random_token());
    let child = std::process::Command::new(&helper_exe)
        .env("MALTORIA_HOOK_PORT", port.to_string())
        .env("MALTORIA_HOOK_TOKEN", &token)
        .env("MALTORIA_HOOK_SESSION", &session)
        .current_dir(&root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot start helper: {error}"))?;
    eprintln!("capture[{:.3}]: helper spawned {} port {port}", log_secs(), helper_exe.display());
    {
        let mut guard = inner.lock().unwrap();
        guard.listener_port = Some(port);
        guard.token = token.clone();
        guard.session_id = Some(session.clone());
        guard.helper_child = Some(child);
    }
    let inner = Arc::clone(inner);
    std::thread::spawn(move || {
        // One helper per session: a single accept is enough.
        if let Ok((stream, _)) = listener.accept() {
            pump_connection(stream, inner, session, token);
        }
    });
    Ok(())
}

fn stop_helper(inner: &Arc<Mutex<CaptureInner>>) {
    let mut guard = inner.lock().unwrap();
    if let Some(link) = guard.helper.take() {
        let frame = json!({
            "type": "command",
            "id": next_id(),
            "session": link.session,
            "command": "shutdown",
        });
        let mut line = serde_json::to_string(&frame).unwrap_or_default();
        line.push('\n');
        if let Ok(mut writer) = link.writer.lock() {
            let _ = writer.write_all(line.as_bytes());
        }
    }
    if let Some(mut child) = guard.helper_child.take() {
        // Bounded grace: the helper exits on shutdown or EOF; kill otherwise.
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = child.kill();
        });
    }
}

pub(crate) fn dispatch(
    capture: &CaptureService,
    store: &Store,
    method: &str,
    params: &Value,
) -> Result<Value, (String, String)> {
    let inner = capture.shared();
    match method {
        "translation.capture.start" => {
            {
                let mut guard = inner.lock().unwrap();
                guard.container = params.get("container").and_then(Value::as_str).map(str::to_owned);
                guard.entry_id = params
                    .get("entryID")
                    .or_else(|| params.get("entry"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                guard.game_id = guard.entry_id.clone();
                guard.provider_id = params
                    .get("providerID")
                    .or_else(|| params.get("provider"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                if guard.provider_id.is_none() {
                    // Default to the first configured provider.
                    if let Ok(list) = crate::native::translation::providers(store, "translation.providers/list", &json!({}))
                        && let Some(first) = list.as_array().and_then(|list| list.first()) {
                        guard.provider_id = first.get("id").and_then(Value::as_str).map(str::to_owned);
                    }
                }
                guard.capture_state = "selecting".into();
                guard.last_error = None;
                guard.target = None;
                guard.threads.clear();
                guard.subtitle_entries.clear();
                guard.last_texts.clear();
            }
            // Sessions are single-use: a leftover helper from a previous run
            // can never accept another target, so recycle it up front.
            stop_helper(&inner);
            std::thread::sleep(std::time::Duration::from_millis(300));
            let arch = params
                .get("architecture")
                .and_then(Value::as_str)
                .unwrap_or("x64")
                .to_owned();
            {
                let mut guard = inner.lock().unwrap();
                guard.architecture = arch.clone();
                guard.helper_tainted = false;
            }
            if let Err(reason) = spawn_helper(&inner, &arch) {
                let mut guard = inner.lock().unwrap();
                guard.capture_state = "error".into();
                guard.last_error = Some(reason.clone());
                return Err(("capture_failed".into(), reason));
            }
            // Refresh the process list once the channel is up.
            let inner_clone = Arc::clone(&inner);
            std::thread::spawn(move || {
                for _ in 0..30 {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    let ready = inner_clone.lock().unwrap().helper.is_some();
                    if ready {
                        let mut guard = inner_clone.lock().unwrap();
                        let _ = send_command(&mut guard, "listProcesses", json!({}));
                        break;
                    }
                }
            });
            let guard = inner.lock().unwrap();
            Ok(snapshot_json(&guard))
        }
        "translation.capture.processes" => {
            let generation_before = {
                let mut guard = inner.lock().unwrap();
                let generation = guard.processes_generation;
                if guard.helper.is_some() {
                    let _ = send_command(&mut guard, "listProcesses", json!({}));
                }
                generation
            };
            // The reply must carry the fresh scan, not the previous cache:
            // wait for the response the helper is about to send, bounded.
            for _ in 0..100 {
                std::thread::sleep(std::time::Duration::from_millis(20));
                let advanced = inner.lock().unwrap().processes_generation != generation_before;
                if advanced {
                    break;
                }
            }
            let guard = inner.lock().unwrap();
            Ok(json!({
                "processes": guard.processes,
                "truncated": guard.processes_truncated,
            }))
        }
        "translation.capture.attach" | "translation.capture.autoAttach" => {
            let target = params
                .get("target")
                .cloned()
                .ok_or_else(|| ("invalid_params".into(), "attach needs target".into()))?;
            // Respawn the channel when the target bitness differs.
            eprintln!("capture[{:.3}]: attach dispatch", log_secs());
            let target_arch = target.get("arch").and_then(Value::as_str).unwrap_or("x64");
            let needs_respawn = {
                let guard = inner.lock().unwrap();
                guard.architecture != target_arch || guard.helper.is_none() || guard.helper_tainted
            };
            if needs_respawn {
                eprintln!("capture[{:.3}]: attach respawning helper", log_secs());
                stop_helper(&inner);
                std::thread::sleep(std::time::Duration::from_millis(300));
                {
                    let mut guard = inner.lock().unwrap();
                    guard.architecture = target_arch.to_owned();
                    guard.helper_tainted = false;
                }
                if let Err(reason) = spawn_helper(&inner, target_arch) {
                    let mut guard = inner.lock().unwrap();
                    guard.capture_state = "error".into();
                    guard.last_error = Some(reason.clone());
                    return Err(("capture_failed".into(), reason));
                }
                // Wait for the fresh handshake before attaching.
                for _ in 0..30 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if inner.lock().unwrap().helper.is_some() {
                        break;
                    }
                }
                eprintln!("capture[{:.3}]: attach handshake wait done", log_secs());
            }
            let mut guard = inner.lock().unwrap();
            if guard.helper.is_none() {
                return Err(("no_helper".into(), "capture helper is not connected".into()));
            }
            // The frontend DTO is camelCase all along, identical to the
            // helper record it originated from: pass it back untouched.
            send_command(&mut guard, "attach", json!({ "target": target }))
                .map_err(|error| ("capture_failed".into(), error))?;
            guard.capture_state = "attaching".into();
            Ok(json!({ "attached": false, "snapshot": snapshot_json(&guard) }))
        }
        "translation.capture.detach" => {
            {
                let mut guard = inner.lock().unwrap();
                if guard.helper.is_some() {
                    let _ = send_command(&mut guard, "detach", json!({}));
                }
                guard.helper_tainted = true;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
            stop_helper(&inner);
            let guard = inner.lock().unwrap();
            Ok(snapshot_json(&guard))
        }
        "translation.overlay.read" => {
            let settings = read_overlay_settings(store);
            Ok(json!({
                "settings": settings,
                "platform": "windows",
                "supportsClickThrough": true,
            }))
        }
        "translation.overlay.configure" => {
            let settings = params.get("settings").cloned().unwrap_or_else(|| params.clone());
            write_overlay_settings(store, &settings);
            Ok(json!({
                "settings": read_overlay_settings(store),
                "platform": "windows",
                "supportsClickThrough": true,
            }))
        }
        "translation.capture.stop" => {
            stop_helper(&inner);
            let mut guard = inner.lock().unwrap();
            guard.capture_state = "stopped".into();
            guard.target = None;
            guard.threads.clear();
            guard.processes.clear();
            guard.session_id = None;
            Ok(snapshot_json(&guard))
        }
        "translation.threads.list" => {
            let guard = inner.lock().unwrap();
            Ok(json!({ "threads": threads_json(&guard) }))
        }
        "translation.threads.select" => {
            let thread_id = params
                .get("threadID")
                .or_else(|| params.get("threadId"))
                .or_else(|| params.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let role = params
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("body")
                .to_owned();
            let mut guard = inner.lock().unwrap();
            if role == "ignore" || role == "body" || role == "speaker" {
                if thread_id.is_empty() {
                    guard.roles.clear();
                } else {
                    guard.roles.insert(thread_id, role);
                }
            }
            Ok(json!({ "selected": true }))
        }
        "translation.snapshot" => {
            let guard = inner.lock().unwrap();
            Ok(snapshot_json(&guard))
        }
        "translation.overlay.read" => {
            let guard = inner.lock().unwrap();
            Ok(guard.overlay.clone())
        }
        "translation.overlay.configure" => {
            let mut guard = inner.lock().unwrap();
            if let Some(patch) = params.get("settings").or_else(|| params.get("overlay")).or_else(|| params.get("patch")).cloned().or_else(|| {
                // flat patch fields directly on params
                let flat = params.as_object().map(|map| {
                    json!(map.iter().filter(|(key, _)| {
                        !matches!(key.as_str(), "container" | "entryID" | "gameID")
                    }).map(|(k, v)| (k.clone(), v.clone())).collect::<BTreeMap<_, _>>())
                });
                flat
            }) {
                if let (Some(target), Some(patch)) = (guard.overlay.as_object_mut(), patch.as_object()) {
                    for (key, value) in patch {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
            Ok(guard.overlay.clone())
        }
        "translation.capture.preset/get" => {
            let key = preset_key(params);
            let document = capture_presets::load(store);
            Ok(json!({
                "preset": document.get(&key).cloned().unwrap_or(Value::Null),
                "found": document.get(&key).is_some(),
            }))
        }
        "translation.capture.preset/save" => {
            let key = preset_key(params);
            let mut document = capture_presets::load(store);
            let preset = params.get("preset").cloned().unwrap_or_else(|| params.clone());
            document[&key] = preset.clone();
            capture_presets::save(store, &document);
            Ok(json!({ "saved": true, "preset": preset }))
        }
        "translation.capture.preset/clear" => {
            let key = preset_key(params);
            let mut document = capture_presets::load(store);
            document.as_object_mut().map(|map| map.remove(&key));
            capture_presets::save(store, &document);
            Ok(json!({ "cleared": true }))
        }
        "translation.history" => {
            let guard = inner.lock().unwrap();
            let offset = params.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
            let max_count = params.get("maxCount").and_then(Value::as_u64).unwrap_or(20) as usize;
            let total = guard.history.len();
            let entries: Vec<Value> = guard
                .history
                .iter()
                .skip(offset)
                .take(max_count)
                .cloned()
                .collect();
            let next_offset = offset + entries.len();
            Ok(json!({
                "entries": entries,
                "total": total,
                "nextOffset": next_offset,
            }))
        }
        "translation.history.text" => {
            let id = params.get("entryID").or_else(|| params.get("id")).and_then(Value::as_str).unwrap_or("");
            let guard = inner.lock().unwrap();
            let entry = guard
                .history
                .iter()
                .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
                .cloned();
            Ok(json!({ "entry": entry }))
        }
        "translation.history.include" => {
            let id = params.get("entryID").or_else(|| params.get("id")).and_then(Value::as_str).unwrap_or("");
            let include = params.get("include").and_then(Value::as_bool).unwrap_or(true);
            let mut guard = inner.lock().unwrap();
            for entry in guard.history.iter_mut() {
                if entry.get("id").and_then(Value::as_str) == Some(id) {
                    entry["includeInContext"] = json!(include);
                }
            }
            Ok(json!({ "updated": true }))
        }
        "translation.history.clear" => {
            let mut guard = inner.lock().unwrap();
            guard.history.clear();
            Ok(json!({ "cleared": true }))
        }
        "translation.capture.hud/show" => {
            // On Windows the floating HUD is the subtitle overlay: flip its
            // visible flag in the persisted settings and answer with the
            // overlay reply shape so the frontend updates its state.
            let mut settings = read_overlay_settings(store);
            if let Some(obj) = settings.as_object_mut() {
                obj.insert("visible".into(), json!(true));
            }
            write_overlay_settings(store, &settings);
            Ok(json!({
                "settings": read_overlay_settings(store),
                "platform": "windows",
                "supportsClickThrough": true,
            }))
        }
        "translation.capture.hud/localize" => Ok(json!({ "ok": true })),
        _ => Err((
            "method_unavailable".into(),
            format!("{method} is not implemented"),
        )),
    }
}

fn preset_key(params: &Value) -> String {
    let container = params.get("container").and_then(Value::as_str).unwrap_or("");
    let entry = params
        .get("entryID")
        .or_else(|| params.get("entry"))
        .and_then(Value::as_str)
        .unwrap_or("");
    format!("{container}|{entry}")
}

fn threads_json(guard: &CaptureInner) -> Vec<Value> {
    guard
        .threads
        .iter()
        .map(|(key, thread)| {
            let mut shown = thread.clone();
            shown["id"] = json!(key);
            shown["role"] = guard
                .roles
                .get(key)
                .cloned()
                .map(Value::String)
                .unwrap_or(Value::Null);
            shown
        })
        .collect()
}

fn snapshot_json(guard: &CaptureInner) -> Value {
    let latest = guard.subtitle_entries.last().cloned().unwrap_or_else(|| {
        json!({
            "source": "",
            "translation": "",
            "speaker": null,
            "status": "idle",
            "error": null,
        })
    });
    let threads = threads_json(guard);
    let threads_total = threads.len();
    json!({
        "sessionID": guard.session_id,
        "gameID": guard.game_id,
        "captureState": guard.capture_state,
        "lastError": guard.last_error,
        "target": guard.target,
        "subtitle": {
            "source": latest.get("source").cloned().unwrap_or(json!("")),
            "translation": latest.get("translation").cloned().unwrap_or(json!("")),
            "speaker": latest.get("speaker").cloned().unwrap_or(Value::Null),
            "kind": Value::Null,
            "status": latest.get("status").cloned().unwrap_or(json!("idle")),
            "error": latest.get("error").cloned().unwrap_or(Value::Null),
            "translationTruncated": false,
            "entries": guard.subtitle_entries,
        },
        "cursor": guard.cursor,
        "processes": guard.processes,
        "historyCount": guard.history.len(),
        "threads": threads,
        "threadsTotal": threads_total,
        "autoBoundCount": guard.roles.len(),
        "preset": Value::Null,
    })
}

pub(crate) mod capture_presets {
    use serde_json::{Value, json};
    use std::path::PathBuf;

    use crate::native::store::Store;

    fn path(store: &Store) -> PathBuf {
        store.root.join("capture-presets.json")
    }

    pub(crate) fn load(store: &Store) -> Value {
        std::fs::read(path(store))
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_else(|| json!({}))
    }

    pub(crate) fn save(store: &Store, document: &Value) {
        if let Ok(data) = serde_json::to_vec_pretty(document) {
            let target = path(store);
            let scratch = target.with_extension("tmp");
            if std::fs::write(&scratch, &data).is_ok() {
                let _ = std::fs::rename(&scratch, &target);
            }
        }
    }
}

#[cfg(test)]
mod hook_integration_tests {
    use super::*;

    #[test]
    fn capture_processes_reach_the_helper() {
        if !cfg!(target_os = "windows") {
            return;
        }
        let root = std::env::temp_dir().join("maltoria-capture-test");
        std::fs::create_dir_all(&root).unwrap();
        let store = Store::load_at(&root);
        let service = CaptureService::new(&store);
        let inner = service.shared();

        dispatch(&service, &store, "translation.capture.start", &json!({
            "container": "native:",
            "entryID": "test-entry",
        })).expect("start");

        // wait for the helper handshake
        let mut connected = false;
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if inner.lock().unwrap().helper.is_some() {
                connected = true;
                break;
            }
        }
        assert!(connected, "helper never connected");

        // clear any cache, then dispatch processes like the frontend does
        dispatch(&service, &store, "translation.capture.processes", &json!({})).expect("processes");
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let count = inner.lock().unwrap().processes.len();
        eprintln!("test: cached processes after refresh = {count}");
        assert!(count > 0, "process cache stayed empty after listProcesses");

        dispatch(&service, &store, "translation.capture.stop", &json!({})).expect("stop");
    }
}

fn overlay_settings_path(store: &Store) -> std::path::PathBuf {
    store.root.join("overlay-settings.json")
}

fn default_overlay_settings() -> serde_json::Value {
    json!({
        "visible": false,
        "clickThrough": true,
        "showSource": true,
        "showTranslation": true,
        "maxEntries": 3,
        "fontSize": 28.0,
        "opacity": 0.85,
        "width": 900.0,
        "height": 180.0,
        "x": null,
        "y": null,
        "textColor": "#FFFFFF",
        "sourceTextColor": "#A0A0A0",
    })
}

fn read_overlay_settings(store: &Store) -> serde_json::Value {
    let path = overlay_settings_path(store);
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(mut stored) = serde_json::from_str::<serde_json::Value>(&text) {
            let defaults = default_overlay_settings();
            if let (Some(stored_obj), Some(defaults_obj)) = (stored.as_object_mut(), defaults.as_object()) {
                for (key, value) in defaults_obj {
                    stored_obj.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
            return stored;
        }
    }
    default_overlay_settings()
}

fn write_overlay_settings(store: &Store, settings: &serde_json::Value) {
    let path = overlay_settings_path(store);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(&path, text);
    }
}

