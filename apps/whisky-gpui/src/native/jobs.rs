//! Job handles for long-running native work (scans, launches), matching the
//! Swift `BridgeJobStore` wire shape: `jobs.get` polls, `jobs.cancel` stops.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

#[derive(Clone)]
pub(crate) struct Job {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) state: Arc<Mutex<JobState>>,
}

pub(crate) struct JobState {
    pub(crate) status: &'static str,
    pub(crate) progress: Value,
    pub(crate) message: Option<String>,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<String>,
    pub(crate) cancel: bool,
}

impl JobState {
    pub(crate) fn running() -> Self {
        Self {
            status: "running",
            progress: Value::Null,
            message: None,
            result: None,
            error: None,
            cancel: false,
        }
    }
}

pub(crate) struct JobStore {
    jobs: BTreeMap<String, Job>,
    next: AtomicU64,
}

impl JobStore {
    pub(crate) fn new() -> Self {
        Self {
            jobs: BTreeMap::new(),
            next: AtomicU64::new(1),
        }
    }

    pub(crate) fn start(store: &Arc<Mutex<JobStore>>, kind: &str) -> (String, Arc<Mutex<JobState>>) {
        let id = {
            let mut guard = store.lock().unwrap();
            let sequence = guard.next.fetch_add(1, Ordering::SeqCst);
            let id = format!("native-job-{sequence}");
            let state = Arc::new(Mutex::new(JobState::running()));
            guard.jobs.insert(
                id.clone(),
                Job {
                    id: id.clone(),
                    kind: kind.to_owned(),
                    state: state.clone(),
                },
            );
            id
        };
        let state = store
            .lock()
            .unwrap()
            .jobs
            .get(&id)
            .map(|job| job.state.clone())
            .expect("job inserted above");
        (id, state)
    }

    pub(crate) fn finish(state: &Arc<Mutex<JobState>>, result: Value) {
        let mut guard = state.lock().unwrap();
        guard.status = "completed";
        guard.result = Some(result);
    }

    pub(crate) fn fail(state: &Arc<Mutex<JobState>>, error: impl Into<String>) {
        let mut guard = state.lock().unwrap();
        guard.status = "failed";
        guard.error = Some(error.into());
    }
}

pub(crate) fn get(store: &Arc<Mutex<JobStore>>, params: &Value) -> Value {
    let id = params
        .get("jobID")
        .or_else(|| params.get("jobId"))
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let guard = store.lock().unwrap();
    let Some(job) = guard.jobs.get(id) else {
        return json!({
            "jobID": id,
            "status": "missing",
        });
    };
    let state = job.state.lock().unwrap();
    let mut value = json!({
        "jobID": job.id,
        "kind": job.kind,
        "status": state.status,
        "progress": state.progress,
        "message": state.message,
    });
    if let Some(result) = &state.result {
        value["result"] = result.clone();
    }
    if let Some(error) = &state.error {
        value["error"] = json!({
            "code": "job_failed",
            "message": error,
            "retryable": false,
        });
    }
    value
}

pub(crate) fn cancel(store: &Arc<Mutex<JobStore>>, params: &Value) -> Value {
    let id = params
        .get("jobID")
        .or_else(|| params.get("jobId"))
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let guard = store.lock().unwrap();
    if let Some(job) = guard.jobs.get(id) {
        let mut state = job.state.lock().unwrap();
        state.cancel = true;
        if state.status == "running" {
            state.status = "cancelled";
        }
    }
    json!({ "cancelled": true })
}
