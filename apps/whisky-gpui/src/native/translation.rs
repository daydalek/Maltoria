//! Translation providers, settings, credentials and the sentence cache for
//! the native backend. Platform-neutral HTTP work lives here so every
//! desktop platform shares one implementation; only the capture injection
//! differs (Wine helper on macOS/Linux, native injector on Windows).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Value, json};

use crate::native::store::Store;

fn document_path(store: &Store) -> PathBuf {
    store.root.join("translation.json")
}

fn credentials_path(store: &Store) -> PathBuf {
    store.root.join("translation-credentials.json")
}

fn load_json(path: &Path) -> Value {
    std::fs::read(path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_json(path: &Path, value: &Value) {
    let _ = std::fs::create_dir_all(store_parent(path));
    if let Ok(data) = serde_json::to_vec_pretty(value) {
        let scratch = path.with_extension("tmp");
        if std::fs::write(&scratch, &data).is_ok() {
            let _ = std::fs::rename(&scratch, path);
        }
    }
}

fn store_parent(path: &Path) -> PathBuf {
    path.parent().map(Path::to_path_buf).unwrap_or_default()
}

fn default_settings() -> Value {
    json!({
        "contextWindowOverride": null,
        "outputTokenLimit": 2048,
        "historyRetentionTurns": 20,
        "contextTurnLimit": 8,
        "includeSourceHistory": false,
        "includeTranslatedHistory": false,
        "customPrompt": "",
        "sourceLanguage": "auto",
        "targetLanguage": "zh-Hans",
        "credentialStorageMode": "localFile",
        "translationDebounceMs": 150,
        "sentenceCacheEnabled": true,
    })
}

/// On Windows there is no OS keychain integration yet; credentials live in a
/// plaintext file under the data root, matching the `localFile` mode.
const CREDENTIAL_STORAGE: &str = "localPlaintextFile";

fn settings_envelope(store: &Store) -> Value {
    let document = load_json(&document_path(store));
    let mut settings = default_settings();
    if let Some(stored) = document.get("settings").and_then(Value::as_object) {
        for (key, value) in stored {
            settings[key.clone()] = value.clone();
        }
    }
    json!({
        "settings": settings,
        "remoteProvidersEnabled": document
            .get("remoteProvidersEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        "credentialStorage": CREDENTIAL_STORAGE,
    })
}

pub(crate) fn settings(store: &Store, method: &str, params: &Value) -> Result<Value, (String, String)> {
    if method == "translation.settings/save" {
        let mut document = load_json(&document_path(store));
        if let Some(settings) = params.get("settings") {
            document["settings"] = settings.clone();
        }
        if let Some(enabled) = params.get("remoteProvidersEnabled") {
            document["remoteProvidersEnabled"] = enabled.clone();
        }
        write_json(&document_path(store), &document);
    }
    Ok(settings_envelope(store))
}

fn provider_capabilities(kind: &str) -> Value {
    let (discovery, streaming, prompt, context) = match kind {
        "openAIChat" => (true, true, true, true),
        "openAICompletions" => (false, true, true, true),
        "anthropic" => (true, true, true, true),
        "gemini" => (true, true, true, true),
        "ollama" => (true, true, true, true),
        "deepL" => (false, false, false, false),
        "localBuiltIn" => (false, false, true, true),
        _ => (false, false, false, false),
    };
    json!({
        "modelDiscovery": discovery,
        "contextDiscovery": false,
        "streaming": streaming,
        "customPrompt": prompt,
        "context": context,
    })
}

fn credentials_map(store: &Store) -> BTreeMap<String, String> {
    load_json(&credentials_path(store))
        .as_object()
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| value.as_str().map(|text| (key.clone(), text.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

fn present_provider(provider: &Value, credentials: &BTreeMap<String, String>) -> Value {
    let mut shown = provider.clone();
    let kind = shown.get("kind").and_then(Value::as_str).unwrap_or("").to_owned();
    shown["capabilities"] = provider_capabilities(&kind);
    shown["hasCredential"] = json!(shown
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| credentials.contains_key(id)));
    // The write shape is a subset of the read shape; fill what the read adds.
    if shown.get("modelMetadata").is_none() {
        shown["modelMetadata"] = Value::Null;
    }
    if shown.get("thinkingLevel").is_none() {
        shown["thinkingLevel"] = json!("default");
    }
    shown
}

fn providers_list(store: &Store) -> Vec<Value> {
    let document = load_json(&document_path(store));
    let credentials = credentials_map(store);
    // One malformed stored entry must not poison the whole list: the frontend
    // decodes the array strictly, so drop anything missing required fields.
    const REQUIRED: &[&str] = &[
        "id", "kind", "name", "baseURL", "defaultSourceLanguage",
        "defaultTargetLanguage", "openAIChatTokenLimitField",
    ];
    document
        .get("providers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|provider| REQUIRED.iter().all(|field| provider.get(field).is_some()))
        .map(|provider| present_provider(provider, &credentials))
        .collect()
}

pub(crate) fn providers(store: &Store, method: &str, params: &Value) -> Result<Value, (String, String)> {
    match method {
        "translation.providers/list" => Ok(json!(providers_list(store))),
        "translation.providers/save" => {
            // The write payload arrives either top-level or under "provider".
            let write = params
                .get("provider")
                .filter(|value| value.is_object())
                .cloned()
                .unwrap_or_else(|| params.clone());
            let mut document = load_json(&document_path(store));
            let mut providers = document
                .get("providers")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut id = write
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            if id.is_empty() {
                id = format!("provider-{}", now_millis());
            }
            // Persist the provider verbatim: the frontend serializes its full
            // DTO and expects every field back on reload. Computed fields
            // (capabilities, hasCredential) are stripped — they are re-derived.
            let mut record = write.clone();
            if let Some(map) = record.as_object_mut() {
                map.insert("id".into(), json!(id));
                map.remove("capabilities");
                map.remove("hasCredential");
            }
            // Preserve any extra stored fields (credentials never live here).
            if let Some(existing) = providers.iter().find(|p| p.get("id").and_then(Value::as_str) == Some(id.as_str())) {
                if let (Some(existing), Some(record_map)) = (existing.as_object(), record.as_object_mut()) {
                    for (key, value) in existing {
                        record_map.entry(key.clone()).or_insert_with(|| value.clone());
                    }
                }
                providers.retain(|p| p.get("id").and_then(Value::as_str) != Some(id.as_str()));
            }
            providers.push(record.clone());
            document["providers"] = json!(providers);
            write_json(&document_path(store), &document);
            let credentials = credentials_map(store);
            Ok(present_provider(&record, &credentials))
        }
        "translation.providers/delete" => {
            let id = params.get("providerID").or_else(|| params.get("id")).and_then(Value::as_str).unwrap_or("");
            let mut document = load_json(&document_path(store));
            let providers = document
                .get("providers")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let remaining: Vec<Value> = providers
                .into_iter()
                .filter(|p| p.get("id").and_then(Value::as_str) != Some(id))
                .collect();
            let deleted = remaining.len() != document
                .get("providers")
                .and_then(Value::as_array)
                .map(|list| list.len())
                .unwrap_or(0);
            document["providers"] = json!(remaining);
            write_json(&document_path(store), &document);
            // Drop the credential with the provider.
            let mut credentials = load_json(&credentials_path(store));
            if let Some(map) = credentials.as_object_mut() {
                map.remove(id);
                write_json(&credentials_path(store), &credentials);
            }
            Ok(json!({ "deleted": deleted }))
        }
        _ => Err(("method_unavailable".into(), format!("{method} is not implemented"))),
    }
}

pub(crate) fn credential(store: &Store, method: &str, params: &Value) -> Result<Value, (String, String)> {
    let id = params
        .get("providerID")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let mut credentials = load_json(&credentials_path(store));
    if method == "translation.credential.set" {
        let key = params
            .get("key")
            .or_else(|| params.get("credential"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if id.is_empty() || key.is_empty() {
            return Err(("invalid_params".into(), "credential.set needs providerID and key".into()));
        }
        credentials[id.clone()] = json!(key);
        write_json(&credentials_path(store), &credentials);
        return Ok(json!({ "storage": CREDENTIAL_STORAGE }));
    }
    let existed = credentials.as_object().is_some_and(|map| map.contains_key(&id));
    if let Some(map) = credentials.as_object_mut() {
        map.remove(&id);
    }
    write_json(&credentials_path(store), &credentials);
    Ok(json!({ "deleted": existed }))
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn now_seconds() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

// ---------------------------------------------------------------------------
// Provider networking: model discovery and translation.
// ---------------------------------------------------------------------------

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .user_agent("Maltoria/0.7 (https://github.com/daydalek/Maltoria)")
        .build()
        .into()
}

fn find_provider(store: &Store, provider_id: &str) -> Result<(Value, Option<String>), (String, String)> {
    let document = load_json(&document_path(store));
    let providers = document
        .get("providers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let provider = providers
        .into_iter()
        .find(|p| p.get("id").and_then(Value::as_str) == Some(provider_id))
        .ok_or_else(|| ("not_found".into(), format!("No provider {provider_id}")))?;
    let key = credentials_map(store).get(provider_id).cloned();
    Ok((provider, key))
}

fn trim_slash(url: &str) -> &str {
    url.trim_end_matches('/')
}

pub(crate) fn models(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let provider_id = params
        .get("providerID")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "translation.models needs providerID".into()))?;
    let (provider, key) = find_provider(store, provider_id)?;
    let kind = provider.get("kind").and_then(Value::as_str).unwrap_or("");
    let base = provider.get("baseURL").and_then(Value::as_str).unwrap_or("");
    let out = match kind {
        "openAIChat" | "openAICompletions" | "openRouter" => {
            let mut request = agent().get(&format!("{}/models", trim_slash(base)));
            if let Some(key) = &key {
                request = request.header("Authorization", &format!("Bearer {key}"));
            }
            let mut response = request.call().map_err(|error| ("network".into(), error.to_string()))?;
            let body: Value = response.body_mut().read_json().map_err(|error| ("network".into(), error.to_string()))?;
            body.get("data")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|model| {
                    model.get("id").and_then(Value::as_str).map(|id| {
                        json!({
                            "id": id,
                            "displayName": id,
                            "contextWindow": null,
                            "contextWindowProvenance": "unknown",
                            "capabilities": [],
                        })
                    })
                })
                .collect::<Vec<_>>()
        }
        "anthropic" => {
            let mut request = agent()
                .get("https://api.anthropic.com/v1/models")
                .header("anthropic-version", "2023-06-01");
            if let Some(key) = &key {
                request = request.header("x-api-key", key);
            }
            let mut response = request.call().map_err(|error| ("network".into(), error.to_string()))?;
            let body: Value = response.body_mut().read_json().map_err(|error| ("network".into(), error.to_string()))?;
            body.get("data")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|model| {
                    model.get("id").and_then(Value::as_str).map(|id| {
                        json!({
                            "id": id,
                            "displayName": model.get("display_name").and_then(Value::as_str).unwrap_or(id),
                            "contextWindow": null,
                            "contextWindowProvenance": "unknown",
                            "capabilities": [],
                        })
                    })
                })
                .collect::<Vec<_>>()
        }
        "gemini" => {
            let key = key.clone().unwrap_or_default();
            let mut response = agent()
                .get(&format!("https://generativelanguage.googleapis.com/v1beta/models?key={key}"))
                .call()
                .map_err(|error| ("network".into(), error.to_string()))?;
            let body: Value = response.body_mut().read_json().map_err(|error| ("network".into(), error.to_string()))?;
            body.get("models")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|model| {
                    let name = model.get("name").and_then(Value::as_str).unwrap_or("");
                    let id = name.strip_prefix("models/").unwrap_or(name);
                    if id.is_empty() {
                        return None;
                    }
                    Some(json!({
                        "id": id,
                        "displayName": model.get("displayName").and_then(Value::as_str).unwrap_or(id),
                        "contextWindow": model.get("inputTokenLimit").cloned(),
                        "contextWindowProvenance": "api",
                        "capabilities": [],
                    }))
                })
                .collect::<Vec<_>>()
        }
        "ollama" => {
            let mut response = agent()
                .get(&format!("{}/api/tags", trim_slash(base)))
                .call()
                .map_err(|error| ("network".into(), error.to_string()))?;
            let body: Value = response.body_mut().read_json().map_err(|error| ("network".into(), error.to_string()))?;
            body.get("models")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|model| {
                    model.get("name").and_then(Value::as_str).map(|id| {
                        json!({
                            "id": id,
                            "displayName": id,
                            "contextWindow": null,
                            "contextWindowProvenance": "unknown",
                            "capabilities": [],
                        })
                    })
                })
                .collect::<Vec<_>>()
        }
        "deepL" | "localBuiltIn" => Vec::new(),
        _ => {
            return Err((
                "unsupported".into(),
                format!("Model discovery is not available for {kind}"),
            ));
        }
    };
    Ok(json!(out))
}

pub(crate) fn model_info(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let provider_id = params
        .get("providerID")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let model_id = params
        .get("modelID")
        .or_else(|| params.get("modelId"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let (provider, _) = find_provider(store, provider_id)?;
    let metadata = provider.get("modelMetadata").cloned().unwrap_or(Value::Null);
    if metadata.is_object() && metadata.get("id").and_then(Value::as_str) == Some(model_id) {
        return Ok(metadata);
    }
    Ok(json!({
        "id": model_id,
        "displayName": model_id,
        "contextWindow": null,
        "contextWindowProvenance": "unknown",
        "capabilities": [],
    }))
}

/// Translate one text through a provider. Shared by testConnection, the
/// capture pipeline and any future batch use.
pub(crate) fn translate_with(
    store: &Store,
    provider: &Value,
    key: Option<&str>,
    text: &str,
    source_language: &str,
    target_language: &str,
    custom_prompt: &str,
) -> Result<String, (String, String)> {
    let kind = provider.get("kind").and_then(Value::as_str).unwrap_or("");
    if kind == "localBuiltIn" {
        return crate::native::local_llm::translate_local(store, text, source_language, target_language, custom_prompt);
    }
    let base = trim_slash(provider.get("baseURL").and_then(Value::as_str).unwrap_or(""));
    let model = provider.get("modelID").and_then(Value::as_str).unwrap_or("");
    let instruction = if custom_prompt.trim().is_empty() {
        format!(
            "Translate the following text from {source_language} to {target_language}. Output only the translation."
        )
    } else {
        format!(
            "{custom_prompt}\n\nTranslate from {source_language} to {target_language}. Output only the translation."
        )
    };
    match kind {
        "openAIChat" | "openRouter" => {
            let body = json!({
                "model": model,
                "messages": [
                    { "role": "system", "content": instruction },
                    { "role": "user", "content": text },
                ],
                "temperature": 0.3,
            });
            let mut request = agent()
                .post(&format!("{base}/chat/completions"))
                .header("Content-Type", "application/json");
            if let Some(key) = key {
                request = request.header("Authorization", &format!("Bearer {key}"));
            }
            let mut response = request.send_json(&body).map_err(|error| ("network".into(), error.to_string()))?;
            let reply: Value = response.body_mut().read_json().map_err(|error| ("network".into(), error.to_string()))?;
            reply
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|choices| choices.first())
                .and_then(|choice| choice.get("message"))
                .and_then(|message| message.get("content"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| ("invalid_reply".into(), "chat completion has no content".into()))
        }
        "anthropic" => {
            let body = json!({
                "model": model,
                "max_tokens": 1024,
                "system": instruction,
                "messages": [ { "role": "user", "content": text } ],
            });
            let mut request = agent()
                .post("https://api.anthropic.com/v1/messages")
                .header("Content-Type", "application/json")
                .header("anthropic-version", "2023-06-01");
            if let Some(key) = key {
                request = request.header("x-api-key", key);
            }
            let mut response = request.send_json(&body).map_err(|error| ("network".into(), error.to_string()))?;
            let reply: Value = response.body_mut().read_json().map_err(|error| ("network".into(), error.to_string()))?;
            reply
                .get("content")
                .and_then(Value::as_array)
                .and_then(|blocks| blocks.first())
                .and_then(|block| block.get("text"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| ("invalid_reply".into(), "anthropic reply has no text".into()))
        }
        "gemini" => {
            let key = key.unwrap_or("");
            let body = json!({
                "systemInstruction": { "parts": [{ "text": instruction }] },
                "contents": [ { "parts": [{ "text": text }] } ],
            });
            let mut response = agent()
                .post(&format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={key}"
                ))
                .header("Content-Type", "application/json")
                .send_json(&body)
                .map_err(|error| ("network".into(), error.to_string()))?;
            let reply: Value = response.body_mut().read_json().map_err(|error| ("network".into(), error.to_string()))?;
            reply
                .get("candidates")
                .and_then(Value::as_array)
                .and_then(|c| c.first())
                .and_then(|c| c.get("content"))
                .and_then(|c| c.get("parts"))
                .and_then(Value::as_array)
                .and_then(|p| p.first())
                .and_then(|p| p.get("text"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| ("invalid_reply".into(), "gemini reply has no text".into()))
        }
        "deepL" => {
            let key = key.unwrap_or("");
            let target = match target_language {
                "zh-Hans" => "ZH-HANS",
                "zh-Hant" => "ZH-HANT",
                other => other,
            };
            let mut response = agent()
                .post("https://api-free.deepl.com/v2/translate")
                .header("Authorization", &format!("DeepL-Auth-Key {key}"))
                .header("Content-Type", "application/json")
                .send_json(&json!({ "text": [text], "target_lang": target }))
                .map_err(|error| ("network".into(), error.to_string()))?;
            let reply: Value = response.body_mut().read_json().map_err(|error| ("network".into(), error.to_string()))?;
            reply
                .get("translations")
                .and_then(Value::as_array)
                .and_then(|t| t.first())
                .and_then(|t| t.get("text"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| ("invalid_reply".into(), "deepl reply has no text".into()))
        }
        "ollama" => {
            let body = json!({
                "model": model,
                "prompt": format!("{instruction}\n\n{text}"),
                "stream": false,
            });
            let mut response = agent()
                .post(&format!("{base}/api/generate"))
                .header("Content-Type", "application/json")
                .send_json(&body)
                .map_err(|error| ("network".into(), error.to_string()))?;
            let reply: Value = response.body_mut().read_json().map_err(|error| ("network".into(), error.to_string()))?;
            reply
                .get("response")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| ("invalid_reply".into(), "ollama reply has no response".into()))
        }
        _ => Err((
            "unsupported".into(),
            format!("Translation is not available for provider kind {kind}"),
        )),
    }
}

pub(crate) fn test_connection(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let provider_id = params
        .get("providerID")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "testConnection needs providerID".into()))?;
    let (provider, key) = find_provider(store, provider_id)?;
    let source = provider.get("defaultSourceLanguage").and_then(Value::as_str).unwrap_or("auto");
    let target = provider.get("defaultTargetLanguage").and_then(Value::as_str).unwrap_or("zh-Hans");
    let sample = if source.starts_with("ja") || source == "auto" {
        "これは接続テストです。"
    } else {
        "This is a connection test."
    };
    let settings = settings_envelope(store);
    let prompt = settings["settings"]["customPrompt"].as_str().unwrap_or("").to_owned();
    let translation = translate_with(store, &provider, key.as_deref(), sample, source, target, &prompt)?;
    Ok(json!({ "translation": translation }))
}

// ---------------------------------------------------------------------------
// Sentence cache.
// ---------------------------------------------------------------------------

fn cache_entries(store: &Store) -> Vec<Value> {
    load_json(&document_path(store))
        .get("sentenceCache")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn record_cache(store: &Store, game_id: &str, source_text: &str, translated: &str, provider_name: &str, provider_kind: &str, model_id: Option<&str>, source_language: &str, target_language: &str) {
    let mut document = load_json(&document_path(store));
    let mut entries = document
        .get("sentenceCache")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let id = format!("{:x}", md5ish(source_text, target_language));
    if let Some(existing) = entries.iter_mut().find(|entry| entry.get("id").and_then(Value::as_str) == Some(id.as_str())) {
        let hits = existing.get("hitCount").and_then(Value::as_u64).unwrap_or(0);
        existing["hitCount"] = json!(hits + 1);
        existing["updatedAt"] = json!(now_seconds());
    } else {
        entries.push(json!({
            "id": id,
            "gameID": game_id,
            "speaker": null,
            "sourceText": source_text,
            "translatedText": translated,
            "providerName": provider_name,
            "providerKind": provider_kind,
            "modelID": model_id,
            "sourceLanguage": source_language,
            "targetLanguage": target_language,
            "hitCount": 1,
            "updatedAt": now_seconds(),
        }));
        if entries.len() > 2000 {
            entries.remove(0);
        }
    }
    document["sentenceCache"] = json!(entries);
    write_json(&document_path(store), &document);
}

fn md5ish(text: &str, target: &str) -> u64 {
    // FNV-1a — stable cache key without pulling in a hash crate.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in format!("{target}|{text}").as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(crate) fn cache_lookup(store: &Store, source_text: &str, target_language: &str) -> Option<String> {
    let id = format!("{:x}", md5ish(source_text, target_language));
    cache_entries(store)
        .into_iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id.as_str()))
        .and_then(|entry| entry.get("translatedText").and_then(Value::as_str).map(str::to_owned))
}

pub(crate) fn cache(store: &Store, method: &str, params: &Value) -> Result<Value, (String, String)> {
    let entries = cache_entries(store);
    match method {
        "translation.cache/list" => {
            let game_filter = params
                .get("gameID")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let query = params
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            let filtered: Vec<Value> = entries
                .into_iter()
                .filter(|entry| {
                    if let Some(game) = &game_filter
                        && entry.get("gameID").and_then(Value::as_str) != Some(game.as_str()) {
                        return false;
                    }
                    if query.is_empty() {
                        return true;
                    }
                    let haystack = format!(
                        "{}{}",
                        entry.get("sourceText").and_then(Value::as_str).unwrap_or(""),
                        entry.get("translatedText").and_then(Value::as_str).unwrap_or("")
                    )
                    .to_lowercase();
                    haystack.contains(&query)
                })
                .collect();
            let total_bytes: u64 = filtered
                .iter()
                .map(|entry| {
                    entry.get("sourceText").and_then(Value::as_str).unwrap_or("").len() as u64
                        + entry.get("translatedText").and_then(Value::as_str).unwrap_or("").len() as u64
                })
                .sum();
            let games: std::collections::BTreeSet<&str> = filtered
                .iter()
                .filter_map(|entry| entry.get("gameID").and_then(Value::as_str))
                .filter(|id| !id.is_empty())
                .collect();
            Ok(json!({
                "totalCount": filtered.len(),
                "gameCount": games.len(),
                "totalBytes": total_bytes,
                "resolvedGameID": game_filter,
                "entries": filtered,
            }))
        }
        "translation.cache/delete" => {
            let id = params.get("id").and_then(Value::as_str).unwrap_or("");
            let mut document = load_json(&document_path(store));
            let before = entries.len();
            let remaining: Vec<Value> = entries
                .into_iter()
                .filter(|entry| entry.get("id").and_then(Value::as_str) != Some(id))
                .collect();
            let removed = remaining.len() != before;
            let total_bytes: u64 = remaining
                .iter()
                .map(|entry| entry.get("sourceText").and_then(Value::as_str).unwrap_or("").len() as u64
                    + entry.get("translatedText").and_then(Value::as_str).unwrap_or("").len() as u64)
                .sum();
            let count = remaining.len();
            document["sentenceCache"] = json!(remaining);
            write_json(&document_path(store), &document);
            Ok(json!({ "removed": removed, "totalCount": count, "totalBytes": total_bytes }))
        }
        "translation.cache.clear" => {
            let game_filter = params.get("gameID").and_then(Value::as_str).map(str::to_owned);
            let mut document = load_json(&document_path(store));
            let remaining: Vec<Value> = match &game_filter {
                Some(game) if !game.is_empty() => entries
                    .into_iter()
                    .filter(|entry| entry.get("gameID").and_then(Value::as_str) != Some(game.as_str()))
                    .collect(),
                _ => Vec::new(),
            };
            let total_bytes: u64 = remaining
                .iter()
                .map(|entry| entry.get("sourceText").and_then(Value::as_str).unwrap_or("").len() as u64
                    + entry.get("translatedText").and_then(Value::as_str).unwrap_or("").len() as u64)
                .sum();
            let count = remaining.len();
            document["sentenceCache"] = json!(remaining);
            write_json(&document_path(store), &document);
            Ok(json!({ "cleared": true, "totalCount": count, "totalBytes": total_bytes }))
        }
        _ => Err(("method_unavailable".into(), format!("{method} is not implemented"))),
    }
}

// ---------------------------------------------------------------------------
// Local (offline) translation — honest empty status until a Windows runtime
// for the bundled models exists.
// ---------------------------------------------------------------------------

pub(crate) fn local(store: &Store, method: &str, _params: &Value) -> Result<Value, (String, String)> {
    let _ = store;
    match method {
        "translation.local.status" => Ok(json!({
            "runtime": {
                "installed": false,
                "version": null,
                "arch": std::env::consts::ARCH,
                "loadable": false,
                "lastError": null,
            },
            "models": [],
            "config": {
                "selectedModelID": null,
                "loadedModelID": null,
            },
        })),
        "translation.local.setConfig" => Ok(json!({ "applied": true })),
        "translation.local.download" => Err((
            "unsupported".into(),
            "Offline model downloads are not yet available on Windows".into(),
        )),
        "translation.local.cancelDownload" | "translation.local.delete" | "translation.local.unload" => {
            Ok(json!({ "done": true }))
        }
        _ => Err(("method_unavailable".into(), format!("{method} is not implemented"))),
    }
}

/// Shared state for the translation pipeline behind capture: which provider
/// is active, resolved lazily from the stored catalog.
pub(crate) struct Pipeline {
    store_path: PathBuf,
    cache_hits: Mutex<u64>,
}

impl Pipeline {
    pub(crate) fn new(store_root: &Path) -> Self {
        Self {
            store_path: store_root.to_path_buf(),
            cache_hits: Mutex::new(0),
        }
    }
}

pub(crate) fn find_provider_pub(store: &Store, provider_id: &str) -> Result<(Value, Option<String>), (String, String)> {
    if provider_id.is_empty() {
        let list = providers_list(store);
        let credentials = credentials_map(store);
        if let Some(first) = list.first() {
            let id = first.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
            return Ok((first.clone(), credentials.get(&id).cloned()));
        }
        return Err(("no_provider".into(), "no translation provider configured".into()));
    }
    find_provider(store, provider_id)
}
