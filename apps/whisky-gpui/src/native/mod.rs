//! Native in-process bridge backend for platforms without the Swift helper.
//!
//! On Windows there is no Wine and no `whisky-bridge` helper process: the
//! library is folder games launched natively. This module implements the
//! bridge protocol subset such a platform needs, in Rust, inside the UI
//! process. The wire shapes mirror the Swift service so the supervisor and
//! the views need no platform forks.

pub(crate) mod capture;
pub(crate) mod local_llm;
pub(crate) mod jobs;
pub(crate) mod library;
pub(crate) mod store;
pub(crate) mod translation;
pub(crate) mod system;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};
use whisky_bridge::{MockFaultAction, MockResponse, PROTOCOL_VERSION};

use crate::native::jobs::JobStore;
use crate::native::store::Store;

/// Advertised methods: the wine/bottle/runtime surface is absent by design.
pub(crate) const METHODS: &[&str] = &[
    "bridge.describe",
    "bridge.shutdown",
    "workspace.recover",
    "workspace.containers",
    "library.entries",
    "library.scan",
    "library.page",
    "library.edit",
    "library.tags.rename",
    "library.tags.delete",
    "entry.rating",
    "entry.hidden",
    "entry.tags",
    "entry.launchOptions",
    "folders.read",
    "folders.addRoot",
    "folders.addRoots",
    "folders.removeRoot",
    "folders.moveRoot",
    "folders.assign",
    "folders.exclude",
    "folders.executable",
    "folders.candidates",
    "folders.edit",
    "folders.visibility",
    "folders.automaticMetadata",
    "programs.list",
    "programs.launch",
    "programs.run",
    "programs.pin",
    "programs.block",
    "programs.edit",
    "processes.list",
    "processes.kill",
    "processes.killAll",
    "preferences.schema",
    "preferences.read",
    "preferences.write",
    "preferences.set",
    "preferences.setMany",
    "system.info",
    "system.open",
    "system.reveal",
    "system.files",
    "system.choosePath",
    "system.chooseFolder",
    "system.chooseFolders",
    "metadata.search",
    "metadata.sync",
    "runtime.launch",
    "translation.settings/get",
    "translation.settings/save",
    "translation.providers/list",
    "translation.providers/save",
    "translation.providers/delete",
    "translation.credential.set",
    "translation.credential.delete",
    "translation.models",
    "translation.modelInfo",
    "translation.testConnection",
    "translation.cache/list",
    "translation.cache/delete",
    "translation.cache.clear",
    "translation.local.status",
    "translation.local.setConfig",
    "translation.local.download",
    "translation.local.cancelDownload",
    "translation.local.delete",
    "translation.local.unload",
    "translation.overlay.read",
    "translation.overlay.configure",
    "translation.capture.start",
    "translation.capture.processes",
    "translation.capture.attach",
    "translation.capture.autoAttach",
    "translation.capture.detach",
    "translation.capture.stop",
    "translation.capture.preset/get",
    "translation.capture.preset/save",
    "translation.capture.preset/clear",
    "translation.capture.hud/show",
    "translation.capture.hud/localize",
    "translation.threads.select",
    "translation.threads.list",
    "translation.snapshot",
    "translation.history",
    "translation.history.text",
    "translation.history.include",
    "translation.history.clear",
    "runtime.stop",
    "runtime.snapshot",
    "runtime.configure",
    "runtime.cancel",
    "runtime.events",
    "programs.settings.read",
    "programs.settings.set",
    "programs.history.read",
    "programs.history.delete",
    "metadata.apply",
    "metadata.refresh",
    "metadata.edit",
    "metadata.clear",
    "metadata.configuration.get",
    "metadata.configuration.save",
    "metadata.cover.import",
    "metadata.cover.reset",
    "launchers.read",
    "launchers.setAutoStart",
    "jobs.get",
    "jobs.cancel",
    "probe",
];

pub(crate) struct NativeServer {
    pub(crate) store: Arc<Store>,
    pub(crate) jobs: Arc<Mutex<JobStore>>,
    pub(crate) capture: capture::CaptureService,
    closed: AtomicBool,
}

impl NativeServer {
    pub(crate) fn new() -> Arc<Self> {
        let store = Arc::new(Store::load());
        let capture = capture::CaptureService::new(&store);
        Arc::new(Self {
            store,
            jobs: Arc::new(Mutex::new(JobStore::new())),
            capture,
            closed: AtomicBool::new(false),
        })
    }

    fn error(id: &Value, code: &str, message: impl Into<String>) -> MockResponse {
        respond(id, Err((code.into(), message.into())))
    }

    fn dispatch(&self, method: &str, params: &Value) -> Result<Value, (String, String)> {
        match method {
            "bridge.describe" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "platform": if cfg!(target_os = "windows") { "windows" } else { "native" },
                "methods": METHODS,
                "capabilities": ["fast_indexing"],
                "maximumFrameBytes": whisky_bridge::MAX_FRAME_BYTES,
                "registry": "",
                "dateUnit": "unix_seconds",
                "pathFormat": "absolute_native"
            })),
            "bridge.shutdown" => Ok(json!({})),
            "workspace.recover" => Ok(json!({})),
            "workspace.containers" => Ok(json!(self.store.containers())),
            "probe" => Ok(Value::Null),
            "system.info" => Ok(system::system_info()),
            "system.open" => system::open(params),
            "system.reveal" => system::reveal(params),
            "system.files" => system::files(params),
            "system.choosePath" => system::choose_path(params),
            "system.chooseFolder" => system::choose_folder(params),
            "system.chooseFolders" => system::choose_folders(params),
            "preferences.schema" => Ok(self.store.preferences_schema()),
            "preferences.read" => Ok(self.store.preferences_read()),
            "preferences.write" => Ok(json!({})),
            "preferences.set" => self.store.preferences_set(params),
            "preferences.setMany" => self.store.preferences_set_many(params),
            "folders.read" => Ok(self.store.folders_read()),
            "folders.addRoot" | "folders.addRoots" => self.store.folders_add_roots(params),
            "folders.removeRoot" => self.store.folders_remove_root(params),
            "folders.moveRoot" => self.store.folders_move_root(params),
            "folders.assign" => self.store.folders_assign(params),
            "folders.exclude" => self.store.folders_exclude(params),
            "folders.executable" => self.store.folders_executable(params),
            "folders.candidates" => Ok(library::folder_candidates(params)),
            "folders.edit" => self.store.folders_edit(params),
            "folders.visibility" => self.store.folders_visibility(params),
            "folders.automaticMetadata" => self.store.folders_automatic_metadata(params),
            "library.entries" => Ok(library::entries(&self.store, params)),
            "library.scan" => library::start_scan(&self.store, &self.jobs, params),
            "library.page" => library::page(&self.store, params),
            "library.edit" => self.store.library_edit(params),
            "library.tags.rename" | "library.tags.delete" => self.store.library_tags(method, params),
            "entry.rating" => self.store.entry_rating(params),
            "entry.hidden" => self.store.entry_hidden(params),
            "entry.tags" => self.store.entry_tags(params),
            "entry.launchOptions" => self.store.entry_launch_options(params),
            "programs.list" => Ok(library::programs_list(&self.store)),
            "programs.launch" | "programs.run" => {
                library::launch(&self.store, &self.jobs, params)
            }
            "programs.pin" => self.store.programs_pin(params),
            "programs.block" => self.store.programs_block(params),
            "programs.edit" => self.store.programs_edit(params),
            "processes.list" => Ok(library::processes_list(&self.store)),
            "processes.kill" => Ok(library::processes_kill(&self.store, params)),
            "processes.killAll" => Ok(library::processes_kill_all(&self.store)),
            "metadata.search" => {
                let jobs = Arc::clone(&self.jobs);
                let params = params.clone();
                let (job_id, state) = crate::native::jobs::JobStore::start(&jobs, "metadata.search");
                std::thread::spawn(move || {
                    let outcome = crate::native_metadata::search(&params);
                    match outcome {
                        Ok(result) => crate::native::jobs::JobStore::finish(&state, result),
                        Err((_, message)) => crate::native::jobs::JobStore::fail(&state, message),
                    }
                });
                Ok(json!({ "jobID": job_id }))
            }
            "metadata.sync" => {
                let jobs = Arc::clone(&self.jobs);
                let store = Arc::clone(&self.store);
                let params = params.clone();
                let (job_id, state) = crate::native::jobs::JobStore::start(&jobs, "metadata.sync");
                std::thread::spawn(move || {
                    let outcome = crate::native_metadata::sync(&store, &params, &state);
                    match outcome {
                        Ok(result) => crate::native::jobs::JobStore::finish(&state, result),
                        Err((_, message)) => crate::native::jobs::JobStore::fail(&state, message),
                    }
                });
                Ok(json!({ "jobID": job_id }))
            }
            "translation.settings/get" | "translation.settings/save" => {
                crate::native::translation::settings(&self.store, method, params)
            }
            "translation.providers/list" | "translation.providers/save" | "translation.providers/delete" => {
                crate::native::translation::providers(&self.store, method, params)
            }
            "translation.credential.set" | "translation.credential.delete" => {
                crate::native::translation::credential(&self.store, method, params)
            }
            "translation.models" => crate::native::translation::models(&self.store, params),
            "translation.modelInfo" => crate::native::translation::model_info(&self.store, params),
            "translation.testConnection" => crate::native::translation::test_connection(&self.store, params),
            "translation.cache/list" | "translation.cache/delete" | "translation.cache.clear" => {
                crate::native::translation::cache(&self.store, method, params)
            }
            "translation.local.status" | "translation.local.setConfig" | "translation.local.download"
            | "translation.local.cancelDownload" | "translation.local.delete" | "translation.local.unload" => {
                crate::native::local_llm::local(&self.store, method, params)
            }
            "translation.capture.start" | "translation.capture.processes"
            | "translation.capture.attach" | "translation.capture.autoAttach"
            | "translation.capture.detach" | "translation.capture.stop"
            | "translation.capture.preset/get" | "translation.capture.preset/save"
            | "translation.capture.preset/clear" | "translation.capture.hud/show"
            | "translation.capture.hud/localize"
            | "translation.threads.select" | "translation.threads.list"
            | "translation.snapshot" | "translation.overlay.read"
            | "translation.overlay.configure"
            | "translation.history" | "translation.history.text"
            | "translation.history.include" | "translation.history.clear" => {
                capture::dispatch(&self.capture, &self.store, method, params)
            }
            "runtime.stop" => {
                crate::native::library::stop(params)
            }
            "runtime.configure" => Ok(json!({ "configured": true })),
            "runtime.cancel" => Ok(json!({ "accepted": true })),
            "runtime.snapshot" => Ok(crate::native::library::runtime_snapshot()),
            "runtime.events" => Ok(crate::native::library::runtime_events(params)),
            "programs.settings.read" => {
                crate::native::library::settings_read(&self.store, params)
            }
            "programs.settings.set" => {
                crate::native::library::settings_set(&self.store, params)
            }
            "programs.history.read" => {
                crate::native::library::history_read(&self.store, params)
            }
            "programs.history.delete" => {
                crate::native::library::history_delete(&self.store, params)
            }
            "runtime.launch" => {
                crate::native::library::launch(&self.store, &self.jobs, params)
                    .map(|_| json!({ "accepted": true, "backend": "native" }))
            }
            "metadata.apply" => crate::native_metadata::apply(&self.store, params),
            "metadata.edit" => crate::native_metadata::edit(&self.store, params),
            "metadata.clear" => crate::native_metadata::clear(&self.store, params),
            "metadata.configuration.get" => {
                crate::native_metadata::configuration(&self.store, method, params)
            }
            "metadata.configuration.save" | "metadata.refresh" => {
                if params["background"].as_bool().unwrap_or(false) {
                    let jobs = Arc::clone(&self.jobs);
                    let store = Arc::clone(&self.store);
                    let params = params.clone();
                    let method = method.to_owned();
                    let (job_id, state) = crate::native::jobs::JobStore::start(&jobs, "metadata.configuration");
                    std::thread::spawn(move || {
                        let outcome = crate::native_metadata::configuration(&store, &method, &params);
                        match outcome {
                            Ok(result) => crate::native::jobs::JobStore::finish(&state, result),
                            Err((_, message)) => crate::native::jobs::JobStore::fail(&state, message),
                        }
                    });
                    Ok(json!({ "jobID": job_id }))
                } else {
                    crate::native_metadata::configuration(&self.store, method, params)
                }
            }
            "metadata.cover.import" | "metadata.cover.reset" => {
                crate::native_metadata::cover(&self.store, method, params)
            }
            "launchers.read" => Ok(json!({ "autoStart": [] })),
            "launchers.setAutoStart" => Ok(json!({ "autoStart": [] })),
            "jobs.get" => Ok(jobs::get(&self.jobs, params)),
            "jobs.cancel" => Ok(jobs::cancel(&self.jobs, params)),
            _ => Err((
                "method_unavailable".into(),
                format!("Method is unavailable: {method}"),
            )),
        }
    }
}

fn respond(id: &Value, result: Result<Value, (String, String)>) -> MockResponse {
    let payload = match result {
        Ok(value) => json!({
            "version": PROTOCOL_VERSION,
            "id": id,
            "result": value,
        }),
        Err((code, message)) => json!({
            "version": PROTOCOL_VERSION,
            "id": id,
            "error": {
                "code": code,
                "message": message,
                "retryable": false,
            }
        }),
    };
    let mut bytes = serde_json::to_vec(&payload).unwrap_or_else(|_| b"{}".to_vec());
    bytes.push(b'\n');
    MockResponse::Immediate(vec![bytes])
}

impl whisky_bridge::InProcessServer for NativeServer {
    fn process_request(&self, request_bytes: &[u8]) -> Result<MockResponse, MockFaultAction> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(MockFaultAction::Crash);
        }
        let request: Value = match serde_json::from_slice(request_bytes) {
            Ok(value) => value,
            Err(error) => {
                return Ok(respond(
                    &Value::Null,
                    Err((
                        "invalid_request".into(),
                        format!("Cannot parse request JSON: {error}"),
                    )),
                ));
            }
        };
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = self.dispatch(&method, &params);
        Ok(respond(&id, result))
    }

    fn mark_closed(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}
