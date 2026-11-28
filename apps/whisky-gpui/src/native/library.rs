//! Folder game library: scanning, entry synthesis, native launching, and
//! process tracking for the native backend.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::native::jobs::JobStore;
use crate::native::store::{Document, NATIVE_CONTAINER_PATH, Store, normalize_path};

pub(crate) fn now_unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|span| span.as_secs_f64())
        .unwrap_or(0.0)
}

pub(crate) fn day_key(unix: f64) -> String {
    let days = (unix / 86_400.0).floor() as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

/// Every game folder discovered under the configured roots: direct children
/// only, plus assigned standalone folders anywhere on disk.
pub(crate) fn scan_folders(document: &Document) -> Vec<String> {
    let mut folders: Vec<String> = Vec::new();
    for root in &document.roots {
        let root_path = Path::new(root);
        let Ok(read) = std::fs::read_dir(root_path) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let normalized = normalize_path(&path.to_string_lossy());
            if !folders.contains(&normalized) {
                folders.push(normalized);
            }
        }
    }
    for assigned in &document.assigned_folders {
        if !folders.contains(assigned) && Path::new(assigned).is_dir() {
            folders.push(assigned.clone());
        }
    }
    folders.retain(|folder| !document.excluded_folders.contains(folder));
    folders.sort();
    folders
}

fn folder_name(folder: &str) -> String {
    Path::new(folder)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| folder.to_owned())
}

/// `.exe` candidates under a folder, shallow-first with a depth cap so huge
/// trees (redist folders, unity builds) stay responsive.
pub(crate) fn find_executables(folder: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut queue: Vec<(PathBuf, u32)> = vec![(PathBuf::from(folder), 0)];
    while let Some((dir, depth)) = queue.pop() {
        if depth > 3 || out.len() >= 200 {
            continue;
        }
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children = Vec::new();
        for entry in read.flatten() {
            children.push(entry.path());
        }
        children.sort();
        for path in children {
            if path.is_dir() {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if !matches!(
                    name.as_str(),
                    "redist" | "redistributable" | "_redist" | "commonredist" | "support" | "prerequisites"
                ) {
                    queue.push((path, depth + 1));
                }
                continue;
            }
            let is_exe = path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
            if !is_exe {
                continue;
            }
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let lower = file_name.to_lowercase();
            if lower.contains("unins")
                || lower.contains("setup")
                || lower.contains("install")
                || lower.contains("crash")
                || lower.contains("report")
                || lower.contains("update")
                || lower == "unitycrashhandler64.exe"
                || lower == "unitycrashhandler32.exe"
            {
                continue;
            }
            out.push((normalize_path(&path.to_string_lossy()), file_name));
        }
    }
    out.sort();
    out
}

pub(crate) fn folder_candidates(params: &Value) -> Value {
    let folder = params
        .get("folder")
        .or_else(|| params.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let candidates = find_executables(&normalize_path(folder))
        .into_iter()
        .map(|(path, name)| json!({ "path": path, "name": name }))
        .collect::<Vec<_>>();
    json!({
        "folder": folder,
        "candidates": candidates,
    })
}

fn entry_activity(overrides: &Value) -> Value {
    json!({
        "playTime": overrides.get("playTime").cloned().unwrap_or(Value::Null),
        "dailyPlayTime": overrides.get("dailyPlayTime").cloned().unwrap_or_else(|| json!({})),
        "rating": overrides.get("rating").cloned().unwrap_or(Value::Null),
        "lastPlayed": overrides.get("lastPlayed").cloned().unwrap_or(Value::Null),
        "lastSessionEndedAt": overrides.get("lastSessionEndedAt").cloned().unwrap_or(Value::Null),
    })
}

fn load_metadata(store: &Store, id: &str) -> Option<Value> {
    let path = store.metadata_dir(id).join("metadata.json");
    let data = std::fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

fn load_cover(store: &Store, id: &str) -> Option<(String, Option<f64>)> {
    let dir = store.metadata_dir(id);
    for name in ["cover.jpg", "cover.png", "cover.webp"] {
        let path = dir.join(name);
        if path.is_file() {
            return Some((
                normalize_path(&path.to_string_lossy()),
                crate::native::library::cover_aspect(&path),
            ));
        }
    }
    None
}

fn cover_aspect(path: &Path) -> Option<f64> {
    let data = std::fs::read(path).ok()?;
    let (width, height) = image_dimensions(&data)?;
    (height > 0).then(|| width as f64 / height as f64)
}

/// Minimal PNG/JPEG dimension sniffing, avoiding a decoder dependency.
fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() > 24 && &data[..8] == b"\x89PNG\r\n\x1a\n" {
        let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        return Some((width, height));
    }
    if data.len() > 4 && data[0] == 0xFF && data[1] == 0xD8 {
        let mut index = 2;
        while index + 9 < data.len() {
            if data[index] != 0xFF {
                index += 1;
                continue;
            }
            let marker = data[index + 1];
            let length = u16::from_be_bytes([data[index + 2], data[index + 3]]) as usize;
            if (0xC0..=0xC3).contains(&marker) && index + 9 < data.len() {
                let height = u16::from_be_bytes([data[index + 5], data[index + 6]]) as u32;
                let width = u16::from_be_bytes([data[index + 7], data[index + 8]]) as u32;
                return Some((width, height));
            }
            if length < 2 {
                return None;
            }
            index += 2 + length;
        }
    }
    if data.len() > 30 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        if &data[12..16] == b"VP8 " {
            let width = u16::from_le_bytes([data[26], data[27]]) as u32 & 0x3FFF;
            let height = u16::from_le_bytes([data[28], data[29]]) as u32 & 0x3FFF;
            return Some((width, height));
        }
        if &data[12..16] == b"VP8L" {
            let bits = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
            let width = (bits & 0x3FFF) + 1;
            let height = ((bits >> 14) & 0x3FFF) + 1;
            return Some((width, height));
        }
        if &data[12..16] == b"VP8X" {
            let width = (data[24] as u32 | (data[25] as u32) << 8 | (data[26] as u32) << 16) + 1;
            let height = (data[27] as u32 | (data[28] as u32) << 8 | (data[29] as u32) << 16) + 1;
            return Some((width, height));
        }
    }
    None
}

fn folder_entry(store: &Store, document: &Document, folder: &str) -> Value {
    let id = format!("folder:{folder}");
    let chosen = document
        .executables
        .get(folder)
        .cloned()
        .flatten()
        .filter(|path| !path.is_empty());
    let executable = chosen.or_else(|| {
        find_executables(folder).first().map(|(path, _)| path.clone())
    });
    let overrides = document.entries.get(&id).cloned().unwrap_or_else(|| json!({}));
    let metadata = load_metadata(store, &id);
    let cover = load_cover(store, &id);
    let custom_name = overrides
        .get("customName")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let hidden = overrides
        .get("hidden")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || document.hidden_folders.contains(&folder.to_owned());
    let mut entry = json!({
        "id": id,
        "identity": format!("folder:{folder}"),
        "name": custom_name.unwrap_or_else(|| folder_name(folder)),
        "container": NATIVE_CONTAINER_PATH,
        "executable": executable,
        "folder": folder,
        "gameDirectory": folder,
        "source": "folder",
        "needsExecutable": false,
        "hidden": hidden,
        "isLauncher": false,
        "tags": overrides.get("tags").cloned().unwrap_or_else(|| json!([])),
        "activity": entry_activity(&overrides),
    });
    if let Some(metadata) = metadata {
        entry["metadata"] = metadata;
    }
    if let Some((path, aspect)) = cover {
        entry["cover"] = json!(path);
        if let Some(aspect) = aspect {
            entry["coverAspectRatio"] = json!(aspect);
        }
    }
    entry
}

fn pinned_entry(store: &Store, document: &Document, path: &str) -> Value {
    let id = format!("pinned:{path}");
    let overrides = document.entries.get(&id).cloned().unwrap_or_else(|| json!({}));
    let name = Path::new(path)
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned());
    let hidden = overrides
        .get("hidden")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut entry = json!({
        "id": id,
        "identity": format!("pinned:{path}"),
        "name": name,
        "container": NATIVE_CONTAINER_PATH,
        "executable": path,
        "source": "pinned",
        "needsExecutable": false,
        "hidden": hidden,
        "isLauncher": false,
        "tags": overrides.get("tags").cloned().unwrap_or_else(|| json!([])),
        "activity": entry_activity(&overrides),
        "gameDirectory": Path::new(path).parent().map(|dir| normalize_path(&dir.to_string_lossy())),
    });
    if let Some(metadata) = load_metadata(store, &id) {
        entry["metadata"] = metadata;
    }
    if let Some((cover, aspect)) = load_cover(store, &id) {
        entry["cover"] = json!(cover);
        if let Some(aspect) = aspect {
            entry["coverAspectRatio"] = json!(aspect);
        }
    }
    entry
}

pub(crate) fn entries(store: &Store, _params: &Value) -> Value {
    let document = store.document.lock().unwrap();
    let mut entries = Vec::new();
    for folder in scan_folders(&document) {
        entries.push(folder_entry(store, &document, &folder));
    }
    if document.include_pinned_and_steam {
        for path in &document.pinned {
            entries.push(pinned_entry(store, &document, path));
        }
    }
    json!(entries)
}

pub(crate) fn start_scan(
    store: &Arc<Store>,
    jobs: &Arc<Mutex<JobStore>>,
    _params: &Value,
) -> Result<Value, (String, String)> {
    let (id, state) = JobStore::start(jobs, "library.scan");
    let store = Arc::clone(store);
    // The scan itself is cheap filesystem walking; run it on a worker thread
    // so large trees never stall the bridge dispatch. Its total must match
    // `library.entries` exactly because the frontend paginates that count.
    std::thread::spawn(move || {
        let total = entries(&store, &json!({}))
            .as_array()
            .map(Vec::len)
            .unwrap_or(0);
        crate::native::jobs::JobStore::finish(
            &state,
            json!({
                "snapshotID": format!("native-{}", crate::native::library::now_unix() as u64),
                "total": total,
            }),
        );
    });
    Ok(json!({ "jobID": id }))
}

/// `library.page`: stable-offset pagination over the native catalogue. The
/// snapshot id is accepted for protocol compatibility; native scans are small
/// enough that the catalogue is regenerated deterministically for each page.
pub(crate) fn page(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let offset = params
        .get("offset")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(100)
        .clamp(1, 100) as usize;
    let all = entries(store, params);
    let all = all.as_array().cloned().unwrap_or_default();
    let total = all.len();
    let offset = offset as usize;
    if offset > total {
        return Err((
            "invalid_params".into(),
            format!("library.page offset {offset} exceeds total {total}"),
        ));
    }
    let end = (offset + limit).min(total);
    let page_entries = all[offset..end].to_vec();
    let next_offset = (end < total).then_some(end as u64);
    Ok(json!({
        "entries": page_entries,
        "total": total,
        "nextOffset": next_offset,
    }))
}

pub(crate) fn programs_list(store: &Store) -> Value {
    let document = store.document.lock().unwrap();
    let mut programs = Vec::new();
    for folder in scan_folders(&document) {
        for (path, name) in find_executables(&folder) {
            programs.push(json!({
                "path": path,
                "name": name,
                "pinned": document.pinned.contains(&path),
                "blocked": document.blocked.contains(&path),
                "available": true,
            }));
        }
    }
    for path in &document.pinned {
        programs.push(json!({
            "path": path,
            "name": Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
            "pinned": true,
            "blocked": false,
            "available": Path::new(path).is_file(),
        }));
    }
    json!(programs)
}

/// Live sessions the native backend spawned, keyed by entry id. Play time
/// accrues when the process exits.
pub(crate) struct Session {
    pub(crate) entry: String,
    pub(crate) pid: u32,
    pub(crate) started: f64,
    pub(crate) name: String,
}

pub(crate) struct ProcessRegistry {
    pub(crate) sessions: Vec<Session>,
    /// Monotonic cursor for `runtime.events`; bumped on every launch/exit.
    pub(crate) sequence: u64,
    /// Entries that ran and exited — reported once as idle so cards clear.
    pub(crate) idle_entries: std::collections::BTreeSet<String>,
    /// Buffered runtime events as (sequence, payload).
    pub(crate) events: Vec<(u64, Value)>,
}

impl ProcessRegistry {
    fn bump(&mut self, event: Option<Value>) {
        self.sequence += 1;
        if let Some(event) = event {
            self.events.push((self.sequence, event));
            // Ring of 128 matches the Swift helper's buffer contract.
            if self.events.len() > 128 {
                self.events.remove(0);
            }
        }
    }
}

pub(crate) fn registry() -> &'static Mutex<ProcessRegistry> {
    static REGISTRY: std::sync::OnceLock<Mutex<ProcessRegistry>> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(ProcessRegistry {
            sessions: Vec::new(),
            sequence: 0,
            idle_entries: std::collections::BTreeSet::new(),
            events: Vec::new(),
        })
    })
}

pub(crate) fn launch(
    store: &Arc<Store>,
    jobs: &Arc<Mutex<JobStore>>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let executable = params
        .get("path")
        .or_else(|| params.get("executable"))
        .or_else(|| params.get("program"))
        .and_then(Value::as_str)
        .map(normalize_path);
    let entry_id = params
        .get("entry")
        .or_else(|| params.get("id"))
        .or_else(|| params.get("entryID"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    // Folder entries carry the folder instead of an exe: resolve the chosen
    // or first candidate.
    let (executable, entry_id) = match (executable, entry_id) {
        (Some(exe), id) => (exe, id),
        (None, Some(id)) if id.starts_with("folder:") => {
            let folder = normalize_path(id.trim_start_matches("folder:"));
            let chosen = {
                let document = store.document.lock().unwrap();
                document.executables.get(&folder).cloned().flatten()
            };
            let exe = chosen
                .or_else(|| find_executables(&folder).first().map(|(path, _)| path.clone()));
            let Some(exe) = exe else {
                return Err((
                    "no_executable".into(),
                    format!("No executable found in {folder}"),
                ));
            };
            (exe, Some(id))
        }
        (None, _) => {
            return Err((
                "invalid_params".into(),
                "programs.launch needs a path or entry".into(),
            ));
        }
    };
    if !Path::new(&executable).is_file() {
        return Err((
            "not_found".into(),
            format!("Executable does not exist: {executable}"),
        ));
    }
    let working = params
        .get("workingDirectory")
        .and_then(Value::as_str)
        .map(normalize_path)
        .or_else(|| {
            Path::new(&executable)
                .parent()
                .map(|dir| normalize_path(&dir.to_string_lossy()))
        });
    // Per-program run settings supply arguments and environment when the
    // launch request does not carry its own.
    let stored = {
        let document = store.document.lock().unwrap();
        document.program_settings.get(&executable).cloned().unwrap_or(Value::Null)
    };
    let mut arguments: Vec<String> = params
        .get("arguments")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if arguments.is_empty() {
        if let Some(text) = stored.get("arguments").and_then(Value::as_str) {
            arguments = text.split_whitespace().map(str::to_owned).collect();
        }
    }
    let mut command = std::process::Command::new(&executable);
    command.args(&arguments);
    if let Some(environment) = stored.get("environment").and_then(Value::as_object) {
        for (key, value) in environment {
            if let Some(value) = value.as_str() {
                command.env(key, value);
            }
        }
    }
    if let Some(working) = working {
        command.current_dir(working);
    }
    let child = command.spawn().map_err(|error| {
        (
            "launch_failed".into(),
            format!("Cannot start {executable}: {error}"),
        )
    })?;
    let pid = child.id();
    let entry = entry_id.unwrap_or_else(|| format!("pinned:{executable}"));
    let name = Path::new(&executable)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| executable.clone());
    let started = now_unix();
    {
        let mut registry = registry().lock().unwrap();
        registry.idle_entries.remove(&entry);
        registry.sessions.push(Session {
            entry: entry.clone(),
            pid,
            started,
            name: name.clone(),
        });
        registry.bump(Some(json!({
            "kind": "launchResult",
            "entryID": entry,
            "status": "launched",
        })));
    }
    // Reap the child and record the session when it exits. The store is the
    // server's shared instance: reloading from disk here would write history
    // the in-memory document never sees.
    let store = Arc::clone(store);
    let _ = jobs;
    let history_exe = executable.clone();
    std::thread::spawn(move || {
        let mut child = child;
        let status = child.wait();
        let ended = now_unix();
        let mut registry = registry().lock().unwrap();
        registry.sessions.retain(|session| session.pid != pid);
        registry.idle_entries.insert(entry.clone());
        registry.bump(None);
        drop(registry);
        store.record_session(&entry, started, ended);
        let mut document = store.document.lock().unwrap();
        let rows = document.run_history.entry(history_exe.clone()).or_default();
        rows.push(json!({
            "id": format!("run-{started}"),
            "programName": history_exe.rsplit('/').next().unwrap_or(""),
            "logPath": "",
            "startTime": started as f64,
            "endTime": ended as f64,
            "exitCode": status.ok().and_then(|s| s.code()).unwrap_or(0),
            "duration": (ended - started) as f64,
        }));
        if rows.len() > 50 {
            rows.remove(0);
        }
        store.save(&document);
    });
    Ok(json!({
        "pid": pid,
        "launched": true,
        "backend": "native",
    }))
}

pub(crate) fn processes_list(_store: &Store) -> Value {
    let registry = registry().lock().unwrap();
    let rows = registry
        .sessions
        .iter()
        .map(|session| {
            json!({
                "pid": session.pid,
                "name": session.name,
                "kind": "game",
                "source": "native",
                "startedAt": session.started,
            })
        })
        .collect::<Vec<_>>();
    json!(rows)
}

pub(crate) fn processes_kill(_store: &Store, params: &Value) -> Value {
    let pid = params
        .get("pid")
        .and_then(Value::as_i64)
        .unwrap_or(0) as u32;
    let killed = kill_pid(pid);
    json!({ "killed": killed })
}

pub(crate) fn processes_kill_all(_store: &Store) -> Value {
    let pids: Vec<u32> = registry()
        .lock()
        .unwrap()
        .sessions
        .iter()
        .map(|session| session.pid)
        .collect();
    let mut killed = 0_u64;
    for pid in pids {
        if kill_pid(pid) {
            killed += 1;
        }
    }
    json!({ "killed": killed })
}

#[cfg(target_os = "windows")]
fn kill_pid(pid: u32) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn kill_pid(pid: u32) -> bool {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe { kill(pid as i32, 15) == 0 }
}

/// `programs.settings.read`: one executable's run settings. `overrides` is
/// null and `defaults` empty — the Wine override layer does not exist on
/// Windows, which also keeps every override section out of the sheet.
pub(crate) fn settings_read(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let executable = params
        .get("executable")
        .and_then(Value::as_str)
        .map(normalize_path)
        .ok_or_else(|| ("invalid_params".into(), "programs.settings.read needs executable".into()))?;
    let document = store.document.lock().unwrap();
    let stored = document.program_settings.get(&executable).cloned().unwrap_or_else(|| json!({}));
    let program_name = stored
        .get("programName")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            Path::new(&executable)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| executable.clone())
        });
    Ok(json!({
        "container": params.get("container").and_then(Value::as_str).unwrap_or("native:"),
        "executable": executable,
        "programName": program_name,
        "locale": stored.get("locale").and_then(Value::as_str).unwrap_or(""),
        "arguments": stored.get("arguments").and_then(Value::as_str).unwrap_or(""),
        "environment": stored.get("environment").cloned().unwrap_or_else(|| json!({})),
        "overrides": Value::Null,
        "defaults": {},
        "available": true,
    }))
}

/// `programs.settings.set`: merge the patch into the stored record; null
/// values delete keys. The reply is the same shape as a read.
pub(crate) fn settings_set(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let executable = params
        .get("executable")
        .and_then(Value::as_str)
        .map(normalize_path)
        .ok_or_else(|| ("invalid_params".into(), "programs.settings.set needs executable".into()))?;
    let patch = params.get("patch").cloned().unwrap_or_else(|| json!({}));
    let mut document = store.document.lock().unwrap();
    let mut stored = document
        .program_settings
        .get(&executable)
        .cloned()
        .unwrap_or_else(|| json!({}));
    fn merge(target: &mut Value, patch: &Value) {
        if let (Some(target), Some(patch)) = (target.as_object_mut(), patch.as_object()) {
            for (key, value) in patch {
                if value.is_null() {
                    target.remove(key);
                } else if value.is_object() && target.get(key).is_some_and(|v| v.is_object()) {
                    merge(target.get_mut(key).unwrap(), value);
                } else {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
    }
    merge(&mut stored, &patch);
    document.program_settings.insert(executable.clone(), stored);
    store.save(&document);
    drop(document);
    settings_read(store, params)
}

/// `runtime.stop`: terminate everything the entry launched — on Windows the
/// native backend tracks all sessions it spawned, so stop is kill-all when
/// no entry filter applies.
pub(crate) fn stop(params: &Value) -> Result<Value, (String, String)> {
    let entry = params
        .get("entryID")
        .or_else(|| params.get("entry"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let pids: Vec<u32> = registry()
        .lock()
        .unwrap()
        .sessions
        .iter()
        .filter(|session| entry.as_ref().is_none_or(|id| &session.entry == id))
        .map(|session| session.pid)
        .collect();
    let mut killed = 0_u64;
    for pid in pids {
        if kill_pid(pid) {
            killed += 1;
        }
    }
    Ok(json!({ "accepted": true, "killed": killed }))
}

const CAP_LAUNCH_CANCEL_STOP: i64 = 1 | 2 | 4;

fn runtime_entries() -> Vec<Value> {
    let registry = registry().lock().unwrap();
    let mut rows: Vec<Value> = registry
        .sessions
        .iter()
        .map(|session| {
            json!({
                "id": session.entry,
                "state": "running",
                "capabilities": CAP_LAUNCH_CANCEL_STOP,
            })
        })
        .collect();
    for entry in &registry.idle_entries {
        rows.push(json!({
            "id": entry,
            "state": "idle",
            "capabilities": CAP_LAUNCH_CANCEL_STOP,
        }));
    }
    rows
}

pub(crate) fn runtime_snapshot() -> Value {
    let sequence = registry().lock().unwrap().sequence;
    json!({
        "sequence": sequence.to_string(),
        "entries": runtime_entries(),
        "queryFailures": {},
    })
}

pub(crate) fn runtime_events(params: &Value) -> Value {
    let after: u64 = params
        .get("after")
        .and_then(Value::as_str)
        .and_then(|text| text.parse().ok())
        .unwrap_or(0);
    let (sequence, events) = {
        let registry = registry().lock().unwrap();
        let events: Vec<Value> = registry
            .events
            .iter()
            .filter(|(sequence, _)| *sequence > after)
            .map(|(sequence, event)| json!({ "sequence": sequence.to_string(), "event": event }))
            .collect();
        (registry.sequence, events)
    };
    json!({
        "sequence": sequence.to_string(),
        "resyncRequired": false,
        "events": events,
        "snapshot": {
            "sequence": sequence.to_string(),
            "entries": runtime_entries(),
            "queryFailures": {},
        },
    })
}

pub(crate) fn history_read(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let executable = params
        .get("executable")
        .and_then(Value::as_str)
        .map(normalize_path)
        .unwrap_or_default();
    let document = store.document.lock().unwrap();
    // Stored oldest-first; the frontend reverses for display.
    let entries = document
        .run_history
        .get(&executable)
        .cloned()
        .unwrap_or_default();
    Ok(json!({ "entries": entries }))
}

pub(crate) fn history_delete(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let executable = params
        .get("executable")
        .and_then(Value::as_str)
        .map(normalize_path)
        .unwrap_or_default();
    let run_id = params.get("runID").and_then(Value::as_str).map(str::to_owned);
    let mut document = store.document.lock().unwrap();
    if let Some(rows) = document.run_history.get_mut(&executable) {
        match run_id {
            Some(id) => rows.retain(|row| row.get("id").and_then(Value::as_str) != Some(id.as_str())),
            None => rows.clear(),
        }
    }
    store.save(&document);
    drop(document);
    history_read(store, params)
}
