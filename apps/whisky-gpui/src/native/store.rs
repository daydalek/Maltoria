//! Persistent state for the native backend: one JSON document plus per-entry
//! metadata cache files, under the platform data directory.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Value, json};

/// The single virtual container every native entry belongs to. The UI keys
/// entries by container path; one row keeps the bottle-centric navigation
/// working while the sidebar chrome is being removed.
pub(crate) const NATIVE_CONTAINER_PATH: &str = "native:";
pub(crate) const NATIVE_CONTAINER_NAME: &str = "Native";

pub(crate) fn data_root() -> PathBuf {
    if let Ok(root) = std::env::var("WHISKY_NATIVE_DATA") {
        return PathBuf::from(root);
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("Maltoria");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local/share/whisky-native");
        }
    }
    std::env::temp_dir().join("maltoria-native")
}

#[derive(Default)]
pub(crate) struct Document {
    /// Folder roots the scanner walks.
    pub(crate) roots: Vec<String>,
    pub(crate) assigned_folders: Vec<String>,
    pub(crate) excluded_folders: Vec<String>,
    /// folder path → chosen executable (or explicit null).
    pub(crate) executables: BTreeMap<String, Option<String>>,
    pub(crate) known_folders: Vec<String>,
    pub(crate) automatic_metadata: bool,
    /// Whether the user explicitly changed automatic metadata. Legacy Windows
    /// stores wrote `false` before scraping existed; those unmigrated values
    /// are treated as the macOS default (`true`) rather than as consent.
    pub(crate) automatic_metadata_touched: bool,
    pub(crate) include_pinned_and_steam: bool,
    /// Folders the user hid from the grid.
    pub(crate) hidden_folders: Vec<String>,
    /// Pinned executables (pinned:<path> entries).
    pub(crate) pinned: Vec<String>,
    pub(crate) blocked: Vec<String>,
    /// Preferences keys → values.
    pub(crate) preferences: BTreeMap<String, Value>,
    /// entry id → activity/overrides: rating, hidden, tags, play_time,
    /// daily_play_time, last_played, last_session_ended_at, custom name.
    pub(crate) entries: BTreeMap<String, Value>,
    /// entry id → launch option overrides.
    pub(crate) launch_options: BTreeMap<String, Value>,
    /// executable path → per-program run settings (locale, arguments,
    /// environment, program name).
    pub(crate) program_settings: BTreeMap<String, Value>,
    /// executable path → finished run entries, newest last, capped at 50.
    pub(crate) run_history: BTreeMap<String, Vec<Value>>,
}

impl Document {
    fn load(root: &Path) -> Self {
        let Ok(data) = fs::read(root.join("library.json")) else {
            return Self {
                automatic_metadata: true,
                automatic_metadata_touched: true,
                include_pinned_and_steam: true,
                ..Self::default()
            };
        };
        let Ok(value) = serde_json::from_slice::<Value>(&data) else {
            return Self {
                automatic_metadata: true,
                automatic_metadata_touched: true,
                include_pinned_and_steam: true,
                ..Self::default()
            };
        };
        let strings = |key: &str| -> Vec<String> {
            value
                .get(key)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        let executables = value
            .get("executables")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            if v.is_null() {
                                None
                            } else {
                                v.as_str().map(str::to_owned)
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let preferences = value
            .get("preferences")
            .and_then(Value::as_object)
            .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let entries = value
            .get("entries")
            .and_then(Value::as_object)
            .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let launch_options = value
            .get("launchOptions")
            .and_then(Value::as_object)
            .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let program_settings = value
            .get("programSettings")
            .and_then(Value::as_object)
            .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let run_history = value
            .get("runHistory")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(k, v)| {
                        v.as_array().map(|rows| (k.clone(), rows.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            roots: strings("roots"),
            assigned_folders: strings("assignedFolders"),
            excluded_folders: strings("excludedFolders"),
            executables,
            known_folders: strings("knownFolders"),
            automatic_metadata: if value
                .get("automaticMetadataTouched")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                value
                    .get("automaticMetadata")
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
            } else {
                true
            },
            automatic_metadata_touched: value
                .get("automaticMetadataTouched")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            include_pinned_and_steam: value
                .get("includePinnedAndSteam")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            hidden_folders: strings("hiddenFolders"),
            pinned: strings("pinned"),
            blocked: strings("blocked"),
            preferences,
            entries,
            launch_options,
            program_settings,
            run_history,
        }
    }

    fn save(&self, root: &Path) {
        let _ = fs::create_dir_all(root);
        let value = json!({
            "roots": self.roots,
            "assignedFolders": self.assigned_folders,
            "excludedFolders": self.excluded_folders,
            "executables": self.executables,
            "knownFolders": self.known_folders,
            "automaticMetadata": self.automatic_metadata,
            "automaticMetadataTouched": self.automatic_metadata_touched,
            "includePinnedAndSteam": self.include_pinned_and_steam,
            "hiddenFolders": self.hidden_folders,
            "pinned": self.pinned,
            "blocked": self.blocked,
            "preferences": self.preferences,
            "entries": self.entries,
            "launchOptions": self.launch_options,
            "programSettings": self.program_settings,
            "runHistory": self.run_history,
        });
        if let Ok(data) = serde_json::to_vec_pretty(&value) {
            let target = root.join("library.json");
            let scratch = root.join("library.json.tmp");
            if fs::write(&scratch, &data).is_ok() {
                let _ = fs::rename(&scratch, &target);
            }
        }
    }
}

pub(crate) struct Store {
    pub(crate) root: PathBuf,
    pub(crate) document: Mutex<Document>,
}

impl Store {
    pub(crate) fn load() -> Self {
        Self::load_at(&data_root())
    }

    pub(crate) fn load_at(root: &Path) -> Self {
        let document = Document::load(root);
        Self {
            root: root.to_path_buf(),
            document: Mutex::new(document),
        }
    }

    pub(crate) fn save(&self, document: &Document) {
        document.save(&self.root);
    }

    pub(crate) fn containers(&self) -> Vec<Value> {
        vec![json!({
            "name": NATIVE_CONTAINER_NAME,
            "path": NATIVE_CONTAINER_PATH,
            "available": true,
        })]
    }

    /// `Game Metadata` cache directory for one entry id.
    pub(crate) fn metadata_dir(&self, id: &str) -> PathBuf {
        let safe: String = id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        self.root.join("Game Metadata").join(safe)
    }

    pub(crate) fn preferences_schema(&self) -> Value {
        json!({
            "booleans": {
                "offlineMode": false,
                "killOnTerminate": true,
                "showMenuBarExtra": false,
                "libraryOnboardingCompleted": false,
                "checkWhiskyWineUpdates": true,
                "checkMaltoriaAppUpdates": false,
                "libraryRefreshAfterExit": true,
                "libraryDetailBlurredBackground": true,
                "librarySingleLineTitles": false,
                "libraryCenterWideCoverText": false,
                "libraryMixedCoverLayout": true,
                "libraryBlurredCoverFill": true,
                "libraryMetadataScrapeCharacters": true,
                "libraryBackdropFill": true,
            },
            "strings": {
                "preferredTerminal": "terminal",
                "preferredLinuxTerminal": "auto",
                "libraryCoverFillStyle": "blurred",
                "libraryDisplayMode": "covers",
                "librarySort": "recent",
                "libraryDetailLayoutStyle": "hero",
                "libraryCoverUpscale": "off",
                "libraryCoverUpscaleEngine": "realcugan",
                "libraryMetadataEnabledSources": "bangumi,vndb,steam,ymgal,nextmoe",
                "libraryMetadataDefaultSource": "bangumi",
                "libraryMetadataBasicSource": "inherit",
                "libraryMetadataSummarySource": "inherit",
                "libraryMetadataCoverSource": "inherit",
                "libraryMetadataCharactersSource": "inherit",
                "librarySidebarMode": "expanded",
                "librarySidebarCollapsedStyle": "icons",
                "telemetryConsent": "undecided",
            },
            "numbers": {
                "libraryCoverWidth": 0,
                "libraryBackdropBlur": 2,
            },
            "paths": ["defaultBottleLocation"]
        })
    }

    pub(crate) fn preferences_read(&self) -> Value {
        let mut values = self.preferences_schema();
        for section in ["booleans", "strings", "numbers"] {
            let Some(defaults) = values
                .get(section)
                .cloned()
                .and_then(|value| value.as_object().cloned())
            else {
                continue;
            };
            for (key, fallback) in defaults {
                values.as_object_mut().unwrap().insert(key, fallback);
            }
            values.as_object_mut().unwrap().remove(section);
        }
        values.as_object_mut().unwrap().remove("paths");
        let document = self.document.lock().unwrap();
        for (key, value) in &document.preferences {
            values[key] = value.clone();
        }
        if !document.preferences.contains_key("defaultBottleLocation") {
            let default = std::env::var("USERPROFILE")
                .map(|home| format!("{}\\Bottles", home))
                .unwrap_or_else(|_| "C:\\Bottles".into());
            values["defaultBottleLocation"] = json!(default);
        }
        values
    }

    pub(crate) fn preferences_set(&self, params: &Value) -> Result<Value, (String, String)> {
        let key = params
            .get("key")
            .and_then(Value::as_str)
            .ok_or_else(|| ("invalid_params".into(), "preferences.set needs key".into()))?;
        let value = params.get("value").cloned().unwrap_or(Value::Null);
        let mut document = self.document.lock().unwrap();
        if value.is_null() {
            document.preferences.remove(key);
        } else {
            document.preferences.insert(key.to_owned(), value);
        }
        self.save(&document);
        drop(document);
        Ok(self.preferences_read())
    }

    pub(crate) fn preferences_set_many(&self, params: &Value) -> Result<Value, (String, String)> {
        let mut document = self.document.lock().unwrap();
        if let Some(values) = params.get("values").and_then(Value::as_object) {
            for (key, value) in values {
                if value.is_null() {
                    document.preferences.remove(key);
                } else {
                    document.preferences.insert(key.clone(), value.clone());
                }
            }
        }
        self.save(&document);
        drop(document);
        Ok(self.preferences_read())
    }

    pub(crate) fn folders_read(&self) -> Value {
        let document = self.document.lock().unwrap();
        folders_dto(&document)
    }

    fn mutate_folders<R>(
        &self,
        mutate: impl FnOnce(&mut Document) -> R,
    ) -> Result<Value, (String, String)> {
        let mut document = self.document.lock().unwrap();
        mutate(&mut document);
        let dto = folders_dto(&document);
        self.save(&document);
        Ok(dto)
    }

    pub(crate) fn folders_add_roots(&self, params: &Value) -> Result<Value, (String, String)> {
        let mut paths: Vec<String> = params
            .get("paths")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(path) = params.get("path").and_then(Value::as_str) {
            paths.push(path.to_owned());
        }
        if let Some(root) = params.get("root").and_then(Value::as_str) {
            paths.push(root.to_owned());
        }
        self.mutate_folders(move |document| {
            for path in paths {
                let normalized = normalize_path(&path);
                if !document.roots.contains(&normalized) {
                    document.roots.push(normalized);
                }
            }
        })
    }

    pub(crate) fn folders_remove_root(&self, params: &Value) -> Result<Value, (String, String)> {
        let path = string_param(params, &["path", "root"])?;
        self.mutate_folders(move |document| {
            document.roots.retain(|root| *root != path);
        })
    }

    pub(crate) fn folders_move_root(&self, params: &Value) -> Result<Value, (String, String)> {
        let source = string_param(params, &["source", "path", "root"])?;
        let destination = string_param(params, &["destination"])?;
        self.mutate_folders(move |document| {
            for root in &mut document.roots {
                if *root == source {
                    *root = destination.clone();
                }
            }
        })
    }

    pub(crate) fn folders_assign(&self, params: &Value) -> Result<Value, (String, String)> {
        let paths = string_list_param(params);
        self.mutate_folders(move |document| {
            for path in paths {
                if !document.assigned_folders.contains(&path) {
                    document.assigned_folders.push(path.clone());
                }
                document.excluded_folders.retain(|item| *item != path);
                if !document.known_folders.contains(&path) {
                    document.known_folders.push(path);
                }
            }
        })
    }

    pub(crate) fn folders_exclude(&self, params: &Value) -> Result<Value, (String, String)> {
        let paths = string_list_param(params);
        self.mutate_folders(move |document| {
            for path in paths {
                if !document.excluded_folders.contains(&path) {
                    document.excluded_folders.push(path.clone());
                }
                document.assigned_folders.retain(|item| *item != path);
                if !document.known_folders.contains(&path) {
                    document.known_folders.push(path);
                }
            }
        })
    }

    pub(crate) fn folders_executable(&self, params: &Value) -> Result<Value, (String, String)> {
        let folder = string_param(params, &["folder", "path"])?;
        let executable = params
            .get("executable")
            .and_then(|value| {
                if value.is_null() {
                    Some(None)
                } else {
                    value.as_str().map(|s| Some(s.to_owned()))
                }
            });
        self.mutate_folders(move |document| {
            document.executables.insert(folder, executable.flatten());
        })
    }

    pub(crate) fn folders_edit(&self, params: &Value) -> Result<Value, (String, String)> {
        self.mutate_folders(move |document| {
            if let Some(name) = params.get("automaticMetadata").and_then(Value::as_bool) {
                document.automatic_metadata = name;
                document.automatic_metadata_touched = true;
            }
            if let Some(include) = params.get("includePinnedAndSteam").and_then(Value::as_bool) {
                document.include_pinned_and_steam = include;
            }
        })
    }

    pub(crate) fn folders_visibility(&self, params: &Value) -> Result<Value, (String, String)> {
        let folder = string_param(params, &["folder", "path"])?;
        let hidden = params
            .get("hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        self.mutate_folders(move |document| {
            if hidden {
                if !document.hidden_folders.contains(&folder) {
                    document.hidden_folders.push(folder);
                }
            } else {
                document.hidden_folders.retain(|item| *item != folder);
            }
        })
    }

    pub(crate) fn folders_automatic_metadata(
        &self,
        params: &Value,
    ) -> Result<Value, (String, String)> {
        self.mutate_folders(move |document| {
            if let Some(enabled) = params
                .get("enabled")
                .or_else(|| params.get("automaticMetadata"))
                .and_then(Value::as_bool)
            {
                document.automatic_metadata = enabled;
                document.automatic_metadata_touched = true;
            }
        })
    }

    fn entry_mutate(
        &self,
        params: &Value,
        mutate: impl FnOnce(&mut Value, &Value),
    ) -> Result<Value, (String, String)> {
        let id = string_param(params, &["id", "entry", "entryID"])?;
        let mut document = self.document.lock().unwrap();
        {
            let entry = document
                .entries
                .entry(id.clone())
                .or_insert_with(|| json!({}));
            mutate(entry, params);
        }
        let reply = document.entries.get(&id).cloned().unwrap_or_else(|| json!({}));
        self.save(&document);
        Ok(reply)
    }

    pub(crate) fn entry_rating(&self, params: &Value) -> Result<Value, (String, String)> {
        self.entry_mutate(params, |entry, params| {
            match params.get("rating") {
                Some(Value::Null) | None => {
                    entry.as_object_mut().map(|map| map.remove("rating"));
                }
                Some(value) => {
                    entry["rating"] = value.clone();
                }
            }
        })
    }

    pub(crate) fn entry_hidden(&self, params: &Value) -> Result<Value, (String, String)> {
        self.entry_mutate(params, |entry, params| {
            entry["hidden"] = params
                .get("hidden")
                .and_then(Value::as_bool)
                .map(Value::from)
                .unwrap_or(Value::Bool(false));
        })
    }

    pub(crate) fn entry_tags(&self, params: &Value) -> Result<Value, (String, String)> {
        self.entry_mutate(params, |entry, params| {
            if let Some(tags) = params.get("tags").and_then(Value::as_array) {
                entry["tags"] = Value::Array(tags.clone());
            }
        })
    }

    pub(crate) fn entry_launch_options(&self, params: &Value) -> Result<Value, (String, String)> {
        let id = string_param(params, &["id", "entry", "entryID"])?;
        let mut document = self.document.lock().unwrap();
        let options = document
            .launch_options
            .entry(id.clone())
            .or_insert_with(|| json!({}));
        if let Some(source) = params.get("options").and_then(Value::as_object) {
            for (key, value) in source {
                options[key] = value.clone();
            }
        }
        for key in [
            "arguments",
            "environment",
            "workingDirectory",
            "multiExecutable",
            "executables",
            "lastExecutable",
        ] {
            if let Some(value) = params.get(key) {
                options[key] = value.clone();
            }
        }
        let multi_executable = options
            .get("multiExecutable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let executables = options
            .get("executables")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let last_executable = options
            .get("lastExecutable")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let reply = json!({
            "entryID": id,
            "multiExecutable": multi_executable,
            "executables": executables,
            "lastExecutable": last_executable,
        });
        self.save(&document);
        Ok(reply)
    }

    pub(crate) fn library_edit(&self, params: &Value) -> Result<Value, (String, String)> {
        // Renames and per-entry display edits land in the entry overrides.
        self.entry_mutate(params, |entry, params| {
            if let Some(name) = params.get("name").and_then(Value::as_str) {
                entry["customName"] = Value::from(name);
            }
        })
    }

    pub(crate) fn library_tags(&self, method: &str, params: &Value) -> Result<Value, (String, String)> {
        let old_tag = params
            .get("oldTag")
            .or_else(|| params.get("tag"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let new_tag = params
            .get("newTag")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let mut document = self.document.lock().unwrap();
        let mut updated = 0_u64;
        for entry in document.entries.values_mut() {
            let Some(tags) = entry.get_mut("tags").and_then(Value::as_array_mut) else {
                continue;
            };
            for tag in tags.iter_mut() {
                if tag.as_str() == Some(old_tag.as_str()) {
                    updated += 1;
                    if method == "library.tags.rename" {
                        *tag = Value::from(new_tag.clone());
                    } else {
                        *tag = Value::Null;
                    }
                }
            }
            tags.retain(|tag| !tag.is_null());
        }
        self.save(&document);
        if method == "library.tags.rename" {
            Ok(json!({ "updated": updated, "oldTag": old_tag, "newTag": new_tag }))
        } else {
            Ok(json!({ "updated": updated, "tag": old_tag }))
        }
    }

    pub(crate) fn programs_pin(&self, params: &Value) -> Result<Value, (String, String)> {
        let path = string_param(params, &["path", "program"])?;
        let pinned = params
            .get("pinned")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let mut document = self.document.lock().unwrap();
        if pinned {
            if !document.pinned.contains(&path) {
                document.pinned.push(path.clone());
            }
            document.blocked.retain(|item| *item != path);
        } else {
            document.pinned.retain(|item| *item != path);
        }
        self.save(&document);
        Ok(json!({}))
    }

    pub(crate) fn programs_block(&self, params: &Value) -> Result<Value, (String, String)> {
        let path = string_param(params, &["path", "program"])?;
        let blocked = params
            .get("blocked")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let mut document = self.document.lock().unwrap();
        if blocked {
            if !document.blocked.contains(&path) {
                document.blocked.push(path.clone());
            }
        } else {
            document.blocked.retain(|item| *item != path);
        }
        self.save(&document);
        Ok(json!({}))
    }

    pub(crate) fn programs_edit(&self, _params: &Value) -> Result<Value, (String, String)> {
        Ok(json!({}))
    }

    /// Read one entry override record.
    pub(crate) fn entry_override(&self, id: &str) -> Value {
        let document = self.document.lock().unwrap();
        document.entries.get(id).cloned().unwrap_or_else(|| json!({}))
    }

    /// Merge one field into an entry override without taking the lock twice.
    pub(crate) fn update_entry(&self, id: &str, key: &str, value: Value) {
        let mut document = self.document.lock().unwrap();
        let entry = document
            .entries
            .entry(id.to_owned())
            .or_insert_with(|| json!({}));
        entry[key] = value;
        self.save(&document);
    }

    /// Accumulate play time for an entry when a session ends.
    pub(crate) fn record_session(&self, id: &str, started: f64, ended: f64) {
        if ended <= started {
            return;
        }
        let duration = ended - started;
        let mut document = self.document.lock().unwrap();
        let entry = document
            .entries
            .entry(id.to_owned())
            .or_insert_with(|| json!({}));
        let total = entry
            .get("playTime")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            + duration;
        entry["playTime"] = json!(total);
        entry["lastPlayed"] = json!(ended);
        entry["lastSessionEndedAt"] = json!(ended);
        let day = crate::native::library::day_key(ended);
        let mut daily = entry
            .get("dailyPlayTime")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let today = daily
            .get(&day)
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            + duration;
        daily.insert(day, json!(today));
        entry["dailyPlayTime"] = Value::Object(daily);
        self.save(&document);
    }
}

pub(crate) fn folders_dto(document: &Document) -> Value {
    json!({
        "roots": document.roots,
        "assignedFolders": document.assigned_folders,
        "excludedFolders": document.excluded_folders,
        "executables": document.executables,
        "knownFolders": document.known_folders,
        "automaticMetadata": document.automatic_metadata,
        "includePinnedAndSteam": document.include_pinned_and_steam,
    })
}

pub(crate) fn normalize_path(path: &str) -> String {
    let expanded = if path == "~" {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|home| home.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_owned())
    } else if let Some(rest) = path.strip_prefix("~/") {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|home| format!("{}/{rest}", home.to_string_lossy()))
            .unwrap_or_else(|| path.to_owned())
    } else {
        path.to_owned()
    };
    expanded.replace('\\', "/").trim_end_matches('/').to_owned()
}

pub(crate) fn string_param(params: &Value, keys: &[&str]) -> Result<String, (String, String)> {
    for key in keys {
        if let Some(value) = params.get(key).and_then(Value::as_str) {
            return Ok(normalize_path(value));
        }
    }
    Err((
        "invalid_params".into(),
        format!("missing string parameter: {}", keys.join("/")),
    ))
}

fn string_list_param(params: &Value) -> Vec<String> {
    let mut out: Vec<String> = params
        .get("paths")
        .or_else(|| params.get("folders"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(normalize_path)
                .collect()
        })
        .unwrap_or_default();
    if let Some(one) = params
        .get("path")
        .or_else(|| params.get("folder"))
        .and_then(Value::as_str)
    {
        out.push(normalize_path(one));
    }
    out
}
