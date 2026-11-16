//! Typed provider/settings state for the local translation bridge.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{AppContext, Context, Entity, Window};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::bridge::BridgeSupervisor;
use crate::state::WhiskyApp;

static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_request_id() -> u64 {
    NEXT_REQUEST.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderCapabilities {
    pub(crate) model_discovery: bool,
    pub(crate) context_discovery: bool,
    pub(crate) streaming: bool,
    pub(crate) custom_prompt: bool,
    pub(crate) context: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelDescriptorDto {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) display_name: String,
    pub(crate) context_window: Option<u64>,
    #[serde(default)]
    pub(crate) context_window_provenance: String,
    #[serde(default)]
    pub(crate) capabilities: Vec<String>,
}

pub(crate) const THINKING_LEVELS: [(&str, &str); 8] = [
    ("default", "模型默认"),
    ("off", "关闭"),
    ("low", "Low"),
    ("medium", "Medium"),
    ("high", "High"),
    ("xhigh", "XHigh"),
    ("max", "Max"),
    ("ultra", "Ultra"),
];

pub(crate) fn default_thinking_level() -> String {
    "default".into()
}

pub(crate) fn normalize_thinking_level(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    THINKING_LEVELS
        .iter()
        .find(|(code, _)| *code == normalized)
        .map(|(code, _)| (*code).to_owned())
        .unwrap_or_else(default_thinking_level)
}

pub(crate) fn supports_thinking_level(kind: &str) -> bool {
    matches!(kind, "openAIChat" | "anthropic" | "gemini" | "ollama")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderDto {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    #[serde(rename = "baseURL")]
    pub(crate) base_url: String,
    #[serde(rename = "modelID")]
    pub(crate) model_id: Option<String>,
    pub(crate) default_source_language: String,
    pub(crate) default_target_language: String,
    pub(crate) model_metadata: Option<ModelDescriptorDto>,
    #[serde(rename = "openAIChatTokenLimitField")]
    pub(crate) open_ai_chat_token_limit_field: String,
    #[serde(default = "default_thinking_level")]
    pub(crate) thinking_level: String,
    pub(crate) capabilities: ProviderCapabilities,
    pub(crate) has_credential: bool,
}

pub(crate) fn default_translation_debounce_ms() -> u64 {
    150
}

pub(crate) fn default_sentence_cache_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationSettingsDto {
    pub(crate) context_window_override: Option<u64>,
    pub(crate) output_token_limit: u64,
    pub(crate) history_retention_turns: u64,
    pub(crate) context_turn_limit: u64,
    pub(crate) include_source_history: bool,
    pub(crate) include_translated_history: bool,
    pub(crate) custom_prompt: String,
    pub(crate) source_language: String,
    pub(crate) target_language: String,
    #[serde(
        rename = "credentialStorageMode",
        default = "default_credential_storage_mode"
    )]
    pub(crate) credential_storage_mode: String,
    #[serde(
        rename = "translationDebounceMs",
        default = "default_translation_debounce_ms"
    )]
    pub(crate) translation_debounce_ms: u64,
    #[serde(
        rename = "sentenceCacheEnabled",
        default = "default_sentence_cache_enabled"
    )]
    pub(crate) sentence_cache_enabled: bool,
}

pub(crate) fn default_credential_storage_mode() -> String {
    "keychain".into()
}

pub(crate) fn normalize_credential_storage_mode(mode: &str) -> String {
    match mode {
        "keychain" | "localFile" | "sessionOnly" => mode.to_owned(),
        _ => default_credential_storage_mode(),
    }
}

pub(crate) fn credential_storage_label(storage: &str) -> String {
    match storage {
        "persistentKeychain" => "macOS 钥匙串".to_owned(),
        "persistentSecretService" => "系统密钥环（Secret Service，加密）".to_owned(),
        "unavailable" => "系统凭据存储不可用（请安装 libsecret-tools）".to_owned(),
        "localPlaintextFile" => "本地文件（明文存储，免弹窗）".to_owned(),
        "sessionOnly" => "仅当前会话内存（关闭后清除）".to_owned(),
        "" => "正在读取".to_owned(),
        other => other.to_owned(),
    }
}

fn default_translation_settings() -> TranslationSettingsDto {
    TranslationSettingsDto {
        context_window_override: None,
        output_token_limit: 2048,
        history_retention_turns: 20,
        context_turn_limit: 8,
        include_source_history: false,
        include_translated_history: false,
        custom_prompt: String::new(),
        source_language: "auto".into(),
        target_language: "en".into(),
        credential_storage_mode: default_credential_storage_mode(),
        translation_debounce_ms: default_translation_debounce_ms(),
        sentence_cache_enabled: default_sentence_cache_enabled(),
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CacheEntryDto {
    pub(crate) id: String,
    #[serde(rename = "gameID", default)]
    pub(crate) game_id: String,
    #[serde(default)]
    pub(crate) speaker: Option<String>,
    #[serde(default)]
    pub(crate) source_text: String,
    #[serde(default)]
    pub(crate) translated_text: String,
    #[serde(default)]
    pub(crate) provider_name: String,
    #[serde(default)]
    pub(crate) provider_kind: String,
    #[serde(rename = "modelID", default)]
    pub(crate) model_id: Option<String>,
    #[serde(default)]
    pub(crate) source_language: String,
    #[serde(default)]
    pub(crate) target_language: String,
    #[serde(default)]
    pub(crate) hit_count: u64,
    #[serde(default)]
    pub(crate) updated_at: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CacheListResponseDto {
    #[serde(default)]
    pub(crate) total_count: u64,
    #[serde(default)]
    pub(crate) game_count: u64,
    #[serde(default)]
    pub(crate) total_bytes: u64,
    #[serde(rename = "resolvedGameID", default)]
    pub(crate) resolved_game_id: Option<String>,
    #[serde(default)]
    pub(crate) entries: Vec<CacheEntryDto>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CacheDeleteResponseDto {
    #[serde(default)]
    removed: bool,
    #[serde(default)]
    total_count: u64,
    #[serde(default)]
    total_bytes: u64,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CacheClearResponseDto {
    #[serde(default)]
    cleared: bool,
    #[serde(default)]
    total_count: u64,
    #[serde(default)]
    total_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TranslationCacheState {
    pub(crate) entries: Vec<CacheEntryDto>,
    pub(crate) total_count: u64,
    pub(crate) game_count: u64,
    pub(crate) total_bytes: u64,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) scope_game_id: Option<String>,
    pub(crate) scope_context: Option<(String, String)>,
    pub(crate) only_current_game: bool,
    pub(crate) query: String,
    pub(crate) loaded_once: bool,
    pub(crate) expanded_in_capture: bool,
}

impl TranslationCacheState {
    pub(crate) fn has_scope(&self) -> bool {
        self.scope_game_id
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty())
            || self.scope_context.is_some()
    }

    pub(crate) fn effective_game_id(&self) -> Option<String> {
        if self.only_current_game {
            self.scope_game_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        } else {
            None
        }
    }
}

fn insert_cache_scope_params(
    params: &mut serde_json::Map<String, Value>,
    game_id: Option<&str>,
    scope_context: Option<&(String, String)>,
) {
    if let Some(gid) = game_id.map(str::trim).filter(|s| !s.is_empty()) {
        params.insert("gameID".into(), Value::String(gid.to_owned()));
    } else if let Some((container, entry_id)) = scope_context {
        params.insert("container".into(), Value::String(container.clone()));
        params.insert("entryID".into(), Value::String(entry_id.clone()));
    }
}

#[derive(Clone, Default)]
struct CachePendingMeta {
    generation: u64,
    deleted_id: Option<String>,
    cleared_game_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsEnvelopeDto {
    pub(crate) settings: TranslationSettingsDto,
    #[serde(default)]
    pub(crate) remote_providers_enabled: bool,
    #[serde(default)]
    pub(crate) credential_storage: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderWriteDto {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    #[serde(rename = "baseURL")]
    pub(crate) base_url: String,
    #[serde(rename = "modelID")]
    pub(crate) model_id: Option<String>,
    pub(crate) default_source_language: String,
    pub(crate) default_target_language: String,
    pub(crate) model_metadata: Option<ModelDescriptorDto>,
    #[serde(rename = "openAIChatTokenLimitField")]
    pub(crate) open_ai_chat_token_limit_field: String,
    pub(crate) thinking_level: String,
}

#[derive(Clone)]
pub(crate) struct ProviderDraft {
    pub(crate) id: Option<String>,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) base_url: String,
    pub(crate) model_id: String,
    pub(crate) source_language: String,
    pub(crate) target_language: String,
    pub(crate) token_limit_field: String,
    pub(crate) thinking_level: String,
    pub(crate) model_metadata: Option<ModelDescriptorDto>,
}

impl Default for ProviderDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            kind: String::new(),
            base_url: String::new(),
            model_id: String::new(),
            source_language: String::new(),
            target_language: String::new(),
            token_limit_field: String::new(),
            thinking_level: default_thinking_level(),
            model_metadata: None,
        }
    }
}

impl ProviderDraft {
    pub(crate) fn set_thinking_level(&mut self, level: &str) {
        self.thinking_level = normalize_thinking_level(level);
    }
}

pub(crate) fn draft_from_provider(profile: &ProviderDto) -> ProviderDraft {
    ProviderDraft {
        id: Some(profile.id.clone()),
        name: profile.name.clone(),
        kind: profile.kind.clone(),
        base_url: profile.base_url.clone(),
        model_id: profile.model_id.clone().unwrap_or_default(),
        source_language: profile.default_source_language.clone(),
        target_language: profile.default_target_language.clone(),
        token_limit_field: profile.open_ai_chat_token_limit_field.clone(),
        thinking_level: normalize_thinking_level(&profile.thinking_level),
        model_metadata: profile.model_metadata.clone(),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TranslationTab {
    #[default]
    Providers,
    Settings,
}

#[derive(Clone)]
pub(crate) struct TranslationPage {
    pub(crate) active: bool,
    pub(crate) return_to_capture: bool,
    pub(crate) tab: TranslationTab,
    pub(crate) context: Option<(String, String, String)>,
    pub(crate) providers: Vec<ProviderDto>,
    pub(crate) providers_loaded: bool,
    pub(crate) selected_id: Option<String>,
    pub(crate) draft: ProviderDraft,
    pub(crate) settings: Option<TranslationSettingsDto>,
    pub(crate) remote_enabled: bool,
    pub(crate) credential_storage: String,
    pub(crate) credential_storage_mode: String,
    pub(crate) models: Vec<ModelDescriptorDto>,
    pub(crate) model_info: Option<ModelDescriptorDto>,
    pub(crate) secret: SecretValue,
    pub(crate) pending_credential: SecretValue,
    pub(crate) inputs: BTreeMap<&'static str, Entity<InputState>>,
    pub(crate) prompt_input: Option<Entity<TextareaState>>,
    pub(crate) pending: BTreeMap<u64, PendingTranslationRequest>,
    pub(crate) cache: TranslationCacheState,
    cache_generation: u64,
    cache_pending: BTreeMap<u64, CachePendingMeta>,
    selection_generation: u64,
    draft_generation: u64,
    settings_edit_generation: u64,
    pub(crate) error: Option<String>,
    pub(crate) message: Option<String>,
}

impl Default for TranslationPage {
    fn default() -> Self {
        Self {
            active: false,
            return_to_capture: false,
            tab: TranslationTab::default(),
            context: None,
            providers: Vec::new(),
            providers_loaded: false,
            selected_id: None,
            draft: ProviderDraft::default(),
            settings: None,
            remote_enabled: false,
            credential_storage: String::new(),
            credential_storage_mode: default_credential_storage_mode(),
            models: Vec::new(),
            model_info: None,
            secret: SecretValue::default(),
            pending_credential: SecretValue::default(),
            inputs: BTreeMap::new(),
            prompt_input: None,
            pending: BTreeMap::new(),
            cache: TranslationCacheState::default(),
            cache_generation: 0,
            cache_pending: BTreeMap::new(),
            selection_generation: 0,
            draft_generation: 0,
            settings_edit_generation: 0,
            error: None,
            message: None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct PendingTranslationRequest {
    method: String,
    provider_id: Option<String>,
    selection_generation: u64,
    draft_generation: u64,
    settings_edit_generation: u64,
}

impl TranslationPage {
    pub(crate) fn is_pending(&self, method: &str) -> bool {
        self.pending.values().any(|request| request.method == method)
    }

    pub(crate) fn provider_probe_pending(&self) -> bool {
        self.pending.values().any(|request| {
            matches!(
                request.method.as_str(),
                "translation.models" | "translation.modelInfo" | "translation.testConnection"
            )
        })
    }

    pub(crate) fn open(&mut self, bridge: &BridgeSupervisor) {
        self.active = true;
        self.remote_enabled = true;
        let defaults = self
            .settings
            .get_or_insert_with(default_translation_settings);
        if self.credential_storage_mode.is_empty() {
            self.credential_storage_mode =
                normalize_credential_storage_mode(&defaults.credential_storage_mode);
        }
        self.cache.scope_context = self
            .context
            .as_ref()
            .map(|(container, entry_id, _)| (container.clone(), entry_id.clone()));
        if self.cache.scope_context.is_none() {
            self.cache.scope_game_id = None;
            self.cache.only_current_game = false;
        }
        self.request(bridge, "translation.providers/list", json!({}));
        self.request(bridge, "translation.settings/get", json!({}));
        let cache_game_id = self.cache.effective_game_id();
        self.refresh_cache(bridge, cache_game_id);
    }

    pub(crate) fn bridge_disconnected(&mut self, reason: Option<&str>) {
        let had_pending = !self.pending.is_empty();
        self.pending.clear();
        self.cache_pending.clear();
        self.cache.loading = false;
        self.pending_credential.clear();
        if had_pending {
            let msg = match reason {
                Some(reason) => format!("桥接连接中断：{reason}"),
                None => "桥接连接中断；未完成的翻译请求已取消。".into(),
            };
            self.error = Some(msg);
        }
    }

    pub(crate) fn reply(
        &mut self,
        request: u64,
        method: &str,
        result: Result<Value, String>,
        bridge: &BridgeSupervisor,
        cx: &mut Context<WhiskyApp>,
    ) {
        let Some(pending) = self.pending.remove(&request) else {
            return;
        };
        if pending.method != method {
            self.cache_pending.remove(&request);
            return;
        }
        let cache_meta = self.cache_pending.remove(&request);
        let selection_current = pending.selection_generation == self.selection_generation;
        let draft_current = pending.draft_generation == self.draft_generation;
        let settings_current = self.settings_response_is_current(&pending);
        let provider_current = pending.provider_id.as_deref() == self.selected_id.as_deref();
        let current_provider_request = self.request_matches_provider(&pending);
        match result {
            Err(error) => {
                if method == "translation.providers/save" {
                    self.pending_credential.clear();
                }
                if method == "translation.providers/list" {
                    self.providers_loaded = true;
                }
                if matches!(
                    method,
                    "translation.cache/list"
                        | "translation.cache/delete"
                        | "translation.cache.clear"
                ) {
                    if method == "translation.cache/list" {
                        self.cache.loading = self.is_pending("translation.cache/list");
                    }
                    let cache_current = cache_meta
                        .as_ref()
                        .is_none_or(|meta| meta.generation == self.cache_generation);
                    if method != "translation.cache/list" || cache_current {
                        self.cache.error = Some(error);
                    }
                    cx.notify();
                    return;
                }
                let show_error = match method {
                    "translation.providers/save" => selection_current && draft_current,
                    "translation.providers/list" => true,
                    "translation.settings/get" | "translation.settings/save" => settings_current,
                    _ => selection_current && draft_current && provider_current,
                };
                if show_error {
                    self.error = Some(error);
                }
            }
            Ok(value) => match method {
                "translation.providers/list" => {
                    self.providers_loaded = true;
                    match serde_json::from_value::<Vec<ProviderDto>>(value) {
                        Ok(profiles) => {
                            self.providers = profiles;
                            if selection_current
                                && self.selected_id.is_none()
                                && self.draft.kind.is_empty()
                            {
                                if let Some(id) = self.providers.first().map(|p| p.id.clone()) {
                                    self.select(Some(id));
                                }
                            }
                        }
                        Err(error) => self.error = Some(format!("无效的提供商数据：{error}")),
                    }
                }
                "translation.settings/get" | "translation.settings/save" => {
                    if settings_current {
                        match serde_json::from_value::<SettingsEnvelopeDto>(value) {
                            Ok(envelope) => {
                                self.sync_settings_inputs(
                                    envelope.settings,
                                    envelope.remote_providers_enabled,
                                    envelope.credential_storage,
                                );
                                self.message = Some("设置已加载".into());
                            }
                            Err(error) => self.error = Some(format!("无效的翻译设置：{error}")),
                        }
                    }
                }
                "translation.providers/save" => {
                    match serde_json::from_value::<ProviderDto>(value) {
                        Ok(profile) => {
                            let id = profile.id.clone();
                            self.upsert_provider(profile);
                            if selection_current && draft_current {
                                let pending_key = (!self.pending_credential.is_empty())
                                    .then(|| self.pending_credential.expose().to_owned());
                                self.select(Some(id.clone()));
                                if let Some(key) = pending_key {
                                    self.pending_credential.clear();
                                    self.request(
                                        bridge,
                                        "translation.credential.set",
                                        json!({"providerID": id, "key": key}),
                                    );
                                    if self.error.is_none() {
                                        self.message =
                                            Some("提供商已保存，正在安全保存凭证…".into());
                                    }
                                } else {
                                    self.message = Some("提供商已保存".into());
                                }
                            }
                        }
                        Err(error) if selection_current && draft_current => {
                            self.pending_credential.clear();
                            self.error = Some(format!("无效的提供商响应：{error}"))
                        }
                        Err(_) => {}
                    }
                }
                "translation.models" => {
                    match serde_json::from_value::<Vec<ModelDescriptorDto>>(value) {
                        Ok(models) if current_provider_request => {
                            self.update_models(models);
                            let model_id = self.current_model_id(cx);
                            self.sync_model_metadata(model_id);
                        }
                        Ok(_) => {}
                        Err(error) if current_provider_request => {
                            self.error = Some(error.to_string())
                        }
                        Err(_) => {}
                    }
                }
                "translation.modelInfo" => {
                    match serde_json::from_value::<ModelDescriptorDto>(value) {
                        Ok(info)
                            if current_provider_request && info.id == self.current_model_id(cx) =>
                        {
                            self.draft.model_metadata = Some(info.clone());
                            self.model_info = Some(info);
                        }
                        Ok(_) => {}
                        Err(error) if current_provider_request => {
                            self.error = Some(error.to_string())
                        }
                        Err(_) => {}
                    }
                }
                "translation.credential.set" => {
                    self.secret.clear();
                    if let Some(id) = pending.provider_id.as_deref() {
                        self.set_has_credential(id, true);
                    }
                    if selection_current && draft_current && provider_current {
                        self.credential_storage = value
                            .get("storage")
                            .and_then(Value::as_str)
                            .unwrap_or(&self.credential_storage)
                            .to_owned();
                        self.message = Some("凭证已安全保存；桥接不会返回凭证值。".into());
                    }
                }
                "translation.credential.delete" => {
                    if let Some(id) = pending.provider_id.as_deref() {
                        self.set_has_credential(id, false);
                    }
                    if selection_current && draft_current && provider_current {
                        if value.get("deleted").and_then(Value::as_bool) == Some(true) {
                            self.message = Some("已移除凭证".into());
                        } else {
                            self.error = Some("没有可移除的已保存凭证。".into());
                        }
                    }
                }
                "translation.providers/delete" => {
                    let deleted = value.get("deleted").and_then(Value::as_bool) == Some(true);
                    if deleted {
                        if let Some(id) = pending.provider_id.as_deref() {
                            self.remove_provider(id);
                            if selection_current && draft_current && provider_current {
                                self.select(None);
                            }
                        }
                        if selection_current && draft_current && provider_current {
                            self.message = Some("提供商已删除".into());
                        }
                    } else if selection_current && draft_current && provider_current {
                        self.error = Some("提供商未删除或已不存在。".into());
                    }
                }
                "translation.testConnection" => {
                    if selection_current && draft_current && provider_current {
                        self.message = value
                            .get("translation")
                            .and_then(Value::as_str)
                            .map(|s| format!("连接测试成功：{s}"));
                    }
                }
                "translation.cache/list" => {
                    self.cache.loading = self.is_pending("translation.cache/list");
                    let cache_current = cache_meta
                        .as_ref()
                        .is_none_or(|meta| meta.generation == self.cache_generation);
                    if cache_current {
                        match serde_json::from_value::<CacheListResponseDto>(value) {
                            Ok(response) => {
                                self.cache.total_count = response.total_count;
                                self.cache.game_count = response.game_count;
                                self.cache.total_bytes = response.total_bytes;
                                if let Some(resolved) = response
                                    .resolved_game_id
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                {
                                    self.cache.scope_game_id = Some(resolved.to_owned());
                                }
                                self.cache.entries = response.entries;
                                self.cache.loaded_once = true;
                                self.cache.error = None;
                            }
                            Err(error) => {
                                self.cache.error = Some(format!("无效的缓存列表数据：{error}"));
                            }
                        }
                    }
                }
                "translation.cache/delete" => {
                    match serde_json::from_value::<CacheDeleteResponseDto>(value) {
                        Ok(response) => {
                            self.cache.total_count = response.total_count;
                            self.cache.total_bytes = response.total_bytes;
                            if response.total_count == 0 {
                                self.cache.game_count = 0;
                            }
                            if let Some(deleted_id) = cache_meta.and_then(|meta| meta.deleted_id) {
                                self.cache.entries.retain(|entry| entry.id != deleted_id);
                            }
                            self.cache.error = None;
                            if response.removed {
                                self.message = Some("已删除缓存条目".into());
                            }
                        }
                        Err(error) => {
                            self.cache.error = Some(format!("无效的删除缓存响应：{error}"));
                        }
                    }
                }
                "translation.cache.clear" => {
                    match serde_json::from_value::<CacheClearResponseDto>(value) {
                        Ok(response) => {
                            self.cache.total_count = response.total_count;
                            self.cache.total_bytes = response.total_bytes;
                            let cleared_game_id = cache_meta.and_then(|meta| meta.cleared_game_id);
                            if response.total_count == 0 {
                                self.cache.game_count = 0;
                                self.cache.entries.clear();
                            } else if let Some(game_id) = cleared_game_id {
                                self.cache.entries.retain(|entry| entry.game_id != game_id);
                            } else {
                                self.cache.entries.clear();
                            }
                            self.cache.error = None;
                            if response.cleared {
                                self.message = Some("翻译缓存已清空".into());
                            }
                        }
                        Err(error) => {
                            self.cache.error = Some(format!("无效的清空缓存响应：{error}"));
                        }
                    }
                }
                _ => {}
            },
        }
        cx.notify();
    }

    pub(crate) fn discover(&mut self, bridge: &BridgeSupervisor) {
        if let Some(id) = &self.selected_id {
            self.request(bridge, "translation.models", json!({"providerID":id}));
        }
    }
    fn update_models(&mut self, models: Vec<ModelDescriptorDto>) {
        self.models = models;
    }

    fn request_matches_provider(&self, pending: &PendingTranslationRequest) -> bool {
        pending.selection_generation == self.selection_generation
            && pending.draft_generation == self.draft_generation
            && pending.provider_id.as_deref() == self.selected_id.as_deref()
    }

    fn sync_model_metadata(&mut self, model_id: String) {
        self.draft.model_id = model_id.clone();
        self.draft.model_metadata = self.models.iter().find(|item| item.id == model_id).cloned();
    }

    pub(crate) fn choose_model(&mut self, model: &ModelDescriptorDto) {
        self.draft.model_id = model.id.clone();
        self.draft.model_metadata = Some(model.clone());
        self.model_info = None;
        self.draft_generation = self.draft_generation.wrapping_add(1);
    }

    fn current_model_id(&self, cx: &Context<WhiskyApp>) -> String {
        let input_model = self.field("model", cx);
        if input_model.is_empty() {
            self.draft.model_id.clone()
        } else {
            input_model
        }
    }

    fn upsert_provider(&mut self, provider: ProviderDto) {
        self.providers.retain(|item| item.id != provider.id);
        self.providers.push(provider);
    }

    fn remove_provider(&mut self, id: &str) {
        self.providers.retain(|profile| profile.id != id);
    }

    fn set_has_credential(&mut self, id: &str, has_credential: bool) {
        if let Some(profile) = self.providers.iter_mut().find(|profile| profile.id == id) {
            profile.has_credential = has_credential;
        }
    }
    pub(crate) fn test_connection(&mut self, bridge: &BridgeSupervisor) {
        if let Some(id) = &self.selected_id {
            self.request(
                bridge,
                "translation.testConnection",
                json!({"providerID":id}),
            );
        }
    }
    pub(crate) fn inspect_model(&mut self, bridge: &BridgeSupervisor, cx: &Context<WhiskyApp>) {
        if let Some(id) = &self.selected_id {
            self.model_info = None;
            self.request(
                bridge,
                "translation.modelInfo",
                json!({"providerID":id,"modelID":self.field("model", cx)}),
            );
        }
    }

    pub(crate) fn refresh_cache(
        &mut self,
        bridge: &BridgeSupervisor,
        game_id: Option<String>,
    ) {
        self.cache_generation = self.cache_generation.wrapping_add(1);
        self.cache.loading = true;
        self.cache.error = None;
        let mut params = serde_json::Map::new();
        if let Some(gid) = game_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            params.insert("gameID".into(), Value::String(gid.to_owned()));
        } else if self.cache.only_current_game {
            insert_cache_scope_params(
                &mut params,
                self.cache.scope_game_id.as_deref(),
                self.cache.scope_context.as_ref(),
            );
        }
        let query = self.cache.query.trim();
        if !query.is_empty() {
            params.insert("query".into(), Value::String(query.to_owned()));
        }
        params.insert("limit".into(), json!(100));
        let ok = self.request_cache(
            bridge,
            "translation.cache/list",
            Value::Object(params),
            CachePendingMeta {
                generation: self.cache_generation,
                deleted_id: None,
                cleared_game_id: None,
            },
        );
        if !ok {
            self.cache.loading = false;
        }
    }

    pub(crate) fn set_cache_only_current_game(
        &mut self,
        bridge: &BridgeSupervisor,
        only_current_game: bool,
    ) {
        self.cache.only_current_game = only_current_game;
        let game_id = self.cache.effective_game_id();
        self.refresh_cache(bridge, game_id);
    }

    pub(crate) fn delete_cache_entry(
        &mut self,
        bridge: &BridgeSupervisor,
        id: impl Into<String>,
    ) {
        let id = id.into();
        self.cache.error = None;
        self.request_cache(
            bridge,
            "translation.cache/delete",
            json!({ "id": id }),
            CachePendingMeta {
                generation: self.cache_generation,
                deleted_id: Some(id),
                cleared_game_id: None,
            },
        );
    }

    pub(crate) fn clear_scoped_cache(
        &mut self,
        bridge: &BridgeSupervisor,
        game_id: Option<String>,
    ) {
        self.cache.error = None;
        let mut params = serde_json::Map::new();
        insert_cache_scope_params(
            &mut params,
            game_id
                .as_deref()
                .or(self.cache.scope_game_id.as_deref()),
            self.cache.scope_context.as_ref(),
        );
        let cleared_game_id = params
            .get("gameID")
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.request_cache(
            bridge,
            "translation.cache.clear",
            Value::Object(params),
            CachePendingMeta {
                generation: self.cache_generation,
                deleted_id: None,
                cleared_game_id,
            },
        );
    }

    pub(crate) fn clear_cache(&mut self, bridge: &BridgeSupervisor, game_id: Option<String>) {
        self.cache.error = None;
        let scoped_game_id = game_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let params = match &scoped_game_id {
            Some(gid) => json!({ "gameID": gid }),
            None => json!({}),
        };
        self.request_cache(
            bridge,
            "translation.cache.clear",
            params,
            CachePendingMeta {
                generation: self.cache_generation,
                deleted_id: None,
                cleared_game_id: scoped_game_id,
            },
        );
    }

    fn request_cache(
        &mut self,
        bridge: &BridgeSupervisor,
        method: &str,
        params: Value,
        meta: CachePendingMeta,
    ) -> bool {
        let request = next_request_id();
        let pending = PendingTranslationRequest {
            method: method.to_owned(),
            provider_id: None,
            selection_generation: self.selection_generation,
            draft_generation: self.draft_generation,
            settings_edit_generation: self.settings_edit_generation,
        };
        match bridge.translation_request(request, method, params) {
            Ok(()) => {
                self.pending.insert(request, pending);
                self.cache_pending.insert(request, meta);
                true
            }
            Err(error) => {
                self.cache.error = Some(error);
                false
            }
        }
    }

    pub(crate) fn request(&mut self, bridge: &BridgeSupervisor, method: &str, params: Value) {
        let request = next_request_id();
        self.error = None;
        self.message = None;
        let provider_id = if method == "translation.providers/save" {
            params
                .pointer("/provider/id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        } else {
            params
                .get("providerID")
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        let pending = PendingTranslationRequest {
            method: method.to_owned(),
            provider_id,
            selection_generation: self.selection_generation,
            draft_generation: self.draft_generation,
            settings_edit_generation: self.settings_edit_generation,
        };
        match bridge.translation_request(request, method, params) {
            Ok(()) => {
                self.pending.insert(request, pending);
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(crate) fn input(
        &mut self,
        key: &'static str,
        value: &str,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<WhiskyApp>,
    ) -> Entity<InputState> {
        if let Some(input) = self.inputs.get(key) {
            return input.clone();
        }
        let initial = value.to_owned();
        let hint = placeholder.to_owned();
        let input = cx.new(|cx| {
            let state = InputState::new(window, cx)
                .placeholder(hint)
                .default_value(initial);
            if key == "credential" {
                state.masked(true)
            } else {
                state
            }
        });
        cx.subscribe(&input, move |this, input, event: &InputEvent, cx| {
            let value = if matches!(event, InputEvent::Change) && key != "credential" {
                Some(input.read(cx).value().to_string())
            } else {
                None
            };
            if this
                .translation_page
                .input_state_event(key, event, value.as_deref())
            {
                cx.notify();
            }
        })
        .detach();
        self.inputs.insert(key, input.clone());
        input
    }

    pub(crate) fn textarea(
        &mut self,
        value: &str,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<WhiskyApp>,
    ) -> Entity<TextareaState> {
        if let Some(input) = &self.prompt_input {
            return input.clone();
        }
        let initial = value.to_owned();
        let hint = placeholder.to_owned();
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder(hint)
                .default_value(initial)
                .auto_grow(3, 8)
        });
        cx.subscribe(&input, move |this, input, event: &InputEvent, cx| {
            let value =
                matches!(event, InputEvent::Change).then(|| input.read(cx).value().to_string());
            if this
                .translation_page
                .input_state_event("prompt", event, value.as_deref())
            {
                cx.notify();
            }
        })
        .detach();
        self.prompt_input = Some(input.clone());
        input
    }

    pub(crate) fn field(&self, key: &'static str, cx: &Context<WhiskyApp>) -> String {
        if key == "prompt" {
            return self
                .prompt_input
                .as_ref()
                .map(|input| input.read(cx).value().to_string())
                .unwrap_or_default();
        }
        self.inputs
            .get(key)
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    pub(crate) fn select(&mut self, id: Option<String>) {
        self.selection_generation = self.selection_generation.wrapping_add(1);
        self.draft_generation = self.draft_generation.wrapping_add(1);
        self.selected_id = id.clone();
        self.pending_credential.clear();
        self.clear_provider_inputs();
        self.models.clear();
        self.model_info = None;
        if let Some(profile) =
            id.and_then(|id| self.providers.iter().find(|profile| profile.id == id))
        {
            self.draft = draft_from_provider(profile);
        } else {
            self.draft = ProviderDraft::default();
        }
    }

    pub(crate) fn set_kind(&mut self, kind: &str) {
        if self.draft.kind == kind && !self.draft.base_url.trim().is_empty() {
            return;
        }
        let old_kind = std::mem::replace(&mut self.draft.kind, kind.to_owned());
        let trimmed = self.draft.base_url.trim();
        if trimmed.is_empty() || (!old_kind.is_empty() && trimmed == default_endpoint(&old_kind)) {
            self.draft.base_url = default_endpoint(kind).into();
        }
        if !supports_thinking_level(kind) || self.draft.thinking_level.is_empty() {
            self.draft.thinking_level = default_thinking_level();
        }
        self.clear_model_metadata();
        self.models.clear();
        self.draft_generation = self.draft_generation.wrapping_add(1);
    }

    pub(crate) fn switch_kind(
        &mut self,
        kind: &str,
        window: &mut Window,
        cx: &mut Context<WhiskyApp>,
    ) {
        let old_url = self.draft.base_url.clone();
        self.set_kind(kind);
        if self.draft.base_url != old_url {
            if let Some(input) = self.inputs.get("endpoint").cloned() {
                let next_url = self.draft.base_url.clone();
                input.update(cx, |state, cx| state.set_value(next_url, window, cx));
            }
        }
    }

    pub(crate) fn set_language_preset(
        &mut self,
        key: &'static str,
        code: &str,
        window: &mut Window,
        cx: &mut Context<WhiskyApp>,
    ) {
        self.input_state_event(key, &InputEvent::Change, Some(code));
        match key {
            "source-language" => {
                let settings = self
                    .settings
                    .get_or_insert_with(default_translation_settings);
                settings.source_language = code.to_owned();
            }
            "target-language" => {
                let settings = self
                    .settings
                    .get_or_insert_with(default_translation_settings);
                settings.target_language = code.to_owned();
            }
            _ => {}
        }
        if let Some(input) = self.inputs.get(key).cloned() {
            let value = code.to_owned();
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
    }

    pub(crate) fn set_token_field(&mut self, field: &str) {
        self.draft.token_limit_field = field.to_owned();
        self.draft_generation = self.draft_generation.wrapping_add(1);
    }

    pub(crate) fn set_thinking_level(&mut self, level: &str) {
        self.draft.set_thinking_level(level);
        self.draft_generation = self.draft_generation.wrapping_add(1);
    }

    pub(crate) fn toggle_source_history(&mut self) {
        let settings = self
            .settings
            .get_or_insert_with(default_translation_settings);
        settings.include_source_history = !settings.include_source_history;
        self.settings_edit_generation = self.settings_edit_generation.wrapping_add(1);
    }

    pub(crate) fn toggle_translated_history(&mut self) {
        let settings = self
            .settings
            .get_or_insert_with(default_translation_settings);
        settings.include_translated_history = !settings.include_translated_history;
        self.settings_edit_generation = self.settings_edit_generation.wrapping_add(1);
    }

    pub(crate) fn toggle_remote_enabled(&mut self) {
        self.remote_enabled = !self.remote_enabled;
        self.settings_edit_generation = self.settings_edit_generation.wrapping_add(1);
    }

    pub(crate) fn toggle_sentence_cache_enabled(&mut self) {
        let settings = self
            .settings
            .get_or_insert_with(default_translation_settings);
        settings.sentence_cache_enabled = !settings.sentence_cache_enabled;
        self.settings_edit_generation = self.settings_edit_generation.wrapping_add(1);
    }

    pub(crate) fn set_translation_debounce_ms(&mut self, ms: u64) {
        let clamped = ms.min(5_000);
        let settings = self
            .settings
            .get_or_insert_with(default_translation_settings);
        settings.translation_debounce_ms = clamped;
        self.settings_edit_generation = self.settings_edit_generation.wrapping_add(1);
        self.inputs.remove("debounce-ms");
    }

    pub(crate) fn apply_translation_debounce_preset(
        &mut self,
        ms: u64,
        window: &mut Window,
        cx: &mut Context<WhiskyApp>,
    ) {
        let input = self.inputs.get("debounce-ms").cloned();
        self.set_translation_debounce_ms(ms);
        if let Some(input) = input {
            let value = ms.min(5_000).to_string();
            input.update(cx, |state, cx| state.set_value(value, window, cx));
            self.inputs.insert("debounce-ms", input);
        }
    }

    pub(crate) fn set_credential_storage_mode(&mut self, mode: &str) {
        let normalized = normalize_credential_storage_mode(mode);
        self.credential_storage_mode = normalized.clone();
        let settings = self
            .settings
            .get_or_insert_with(default_translation_settings);
        settings.credential_storage_mode = normalized;
        self.settings_edit_generation = self.settings_edit_generation.wrapping_add(1);
    }

    pub(crate) fn sync_settings_inputs(
        &mut self,
        mut settings: TranslationSettingsDto,
        remote_providers_enabled: bool,
        credential_storage: String,
    ) {
        let mode = normalize_credential_storage_mode(&settings.credential_storage_mode);
        settings.credential_storage_mode = mode.clone();
        self.credential_storage_mode = mode;
        self.settings = Some(settings);
        self.remote_enabled = remote_providers_enabled;
        self.credential_storage = credential_storage;
        self.inputs.retain(|key, _| {
            !matches!(
                *key,
                "context-window"
                    | "output-limit"
                    | "retention"
                    | "context-turns"
                    | "debounce-ms"
                    | "source-language"
                    | "target-language"
            )
        });
        self.prompt_input = None;
    }

    fn settings_response_is_current(&self, pending: &PendingTranslationRequest) -> bool {
        pending.settings_edit_generation == self.settings_edit_generation
    }

    fn input_state_event(
        &mut self,
        key: &'static str,
        event: &InputEvent,
        value: Option<&str>,
    ) -> bool {
        if !matches!(event, InputEvent::Change) {
            return false;
        }
        let value = value.unwrap_or_default();
        match key {
            "name" => {
                self.draft_generation = self.draft_generation.wrapping_add(1);
                self.draft.name = value.to_owned();
            }
            "endpoint" => {
                self.draft_generation = self.draft_generation.wrapping_add(1);
                self.draft.base_url = value.to_owned();
                self.clear_model_metadata();
                self.models.clear();
            }
            "model" => {
                self.draft_generation = self.draft_generation.wrapping_add(1);
                self.sync_model_metadata(value.to_owned());
                self.model_info = None;
            }
            "provider-source-language" => {
                self.draft_generation = self.draft_generation.wrapping_add(1);
                self.draft.source_language = value.to_owned();
            }
            "provider-target-language" => {
                self.draft_generation = self.draft_generation.wrapping_add(1);
                self.draft.target_language = value.to_owned();
            }
            "credential" => {}
            "context-window" | "output-limit" | "retention" | "context-turns" | "prompt"
            | "source-language" | "target-language" => {
                self.settings_edit_generation = self.settings_edit_generation.wrapping_add(1);
            }
            "debounce-ms" => {
                self.settings_edit_generation = self.settings_edit_generation.wrapping_add(1);
                if let Ok(ms) = parse_setting_range(value.trim(), "翻译防抖延迟", 0, 5_000) {
                    self.settings
                        .get_or_insert_with(default_translation_settings)
                        .translation_debounce_ms = ms;
                }
            }
            "cache-query" => {
                self.cache.query = value.to_owned();
            }
            _ => {}
        }
        true
    }

    fn clear_model_metadata(&mut self) {
        self.draft.model_metadata = None;
        self.model_info = None;
    }

    fn clear_provider_inputs(&mut self) {
        self.inputs.retain(|key, _| {
            !matches!(
                *key,
                "name"
                    | "endpoint"
                    | "model"
                    | "credential"
                    | "model-search"
                    | "provider-source-language"
                    | "provider-target-language"
            )
        });
    }

    pub(crate) fn save_provider(
        &mut self,
        bridge: &BridgeSupervisor,
        window: &mut Window,
        cx: &mut Context<WhiskyApp>,
    ) {
        let model_id = if self.draft.kind == "deepL" {
            String::new()
        } else {
            self.field("model", cx)
        };
        let name = self.field("name", cx);
        let endpoint = self.field("endpoint", cx);
        if name.trim().is_empty() || endpoint.trim().is_empty() {
            self.error = Some("提供商名称和 API 端点不能为空。".into());
            return;
        }
        let credential = self.field("credential", cx);
        if !credential.is_empty() {
            self.pending_credential.set(credential);
            if let Some(input) = self.inputs.remove("credential") {
                input.update(cx, |state, cx| state.set_value(String::new(), window, cx));
            }
        }
        let thinking_level = if supports_thinking_level(&self.draft.kind) {
            normalize_thinking_level(&self.draft.thinking_level)
        } else {
            default_thinking_level()
        };
        let draft = ProviderWriteDto {
            id: self.draft.id.clone().unwrap_or_else(new_profile_id),
            name: name.trim().to_owned(),
            kind: self.draft.kind.clone(),
            base_url: endpoint.trim().to_owned(),
            model_id: (!model_id.trim().is_empty()).then(|| model_id.trim().to_owned()),
            default_source_language: self.field("provider-source-language", cx),
            default_target_language: self.field("provider-target-language", cx),
            model_metadata: self.draft.model_metadata.clone(),
            open_ai_chat_token_limit_field: self.draft.token_limit_field.clone(),
            thinking_level,
        };
        match serde_json::to_value(draft) {
            Ok(provider) => {
                self.request(
                    bridge,
                    "translation.providers/save",
                    json!({"provider": provider}),
                );
                if self.error.is_some() {
                    self.pending_credential.clear();
                }
            }
            Err(error) => {
                self.pending_credential.clear();
                self.error = Some(error.to_string());
            }
        }
    }

    pub(crate) fn delete_provider(&mut self, bridge: &BridgeSupervisor) {
        if let Some(id) = &self.selected_id {
            self.request(
                bridge,
                "translation.providers/delete",
                json!({"providerID":id}),
            );
        }
    }

    pub(crate) fn save_credential(
        &mut self,
        bridge: &BridgeSupervisor,
        window: &mut Window,
        cx: &mut Context<WhiskyApp>,
    ) {
        let Some(id) = self.selected_id.clone() else {
            self.error = Some("先保存并选择一个提供商，再设置凭证。".into());
            return;
        };
        self.secret.set(self.field("credential", cx));
        if self.secret.is_empty() {
            self.error = Some("请输入凭证，空值不会覆盖已保存的凭证。".into());
            return;
        }
        let key = self.secret.expose().to_owned();
        self.secret.clear();
        if let Some(input) = self.inputs.remove("credential") {
            input.update(cx, |state, cx| state.set_value(String::new(), window, cx));
        }
        self.request(
            bridge,
            "translation.credential.set",
            json!({"providerID":id,"key":key}),
        );
    }

    pub(crate) fn remove_credential(&mut self, bridge: &BridgeSupervisor) {
        if let Some(id) = &self.selected_id {
            self.request(
                bridge,
                "translation.credential.delete",
                json!({"providerID":id}),
            );
        }
    }

    pub(crate) fn new_provider(&mut self, kind: &str) {
        self.select(None);
        self.pending_credential.clear();
        self.draft = ProviderDraft {
            kind: kind.to_owned(),
            base_url: default_endpoint(kind).into(),
            source_language: "auto".into(),
            target_language: "en".into(),
            token_limit_field: "max_completion_tokens".into(),
            thinking_level: default_thinking_level(),
            ..ProviderDraft::default()
        };
        self.draft_generation = self.draft_generation.wrapping_add(1);
        self.clear_provider_inputs();
    }

    pub(crate) fn dismiss(&mut self) {
        self.active = false;
        self.return_to_capture = false;
        self.secret.clear();
        self.pending_credential.clear();
        self.inputs.remove("credential");
    }

    pub(crate) fn close(&mut self, window: &mut Window, cx: &mut Context<WhiskyApp>) {
        let input = self.inputs.remove("credential");
        self.dismiss();
        if let Some(input) = input {
            input.update(cx, |state, cx| state.set_value(String::new(), window, cx));
        }
    }

    pub(crate) fn save_settings(&mut self, bridge: &BridgeSupervisor, cx: &Context<WhiskyApp>) {
        let settings = TranslationSettingsDto {
            context_window_override: match parse_context_window_override(
                self.field("context-window", cx).trim(),
            ) {
                Ok(value) => value,
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            },
            output_token_limit: match parse_setting_range(
                self.field("output-limit", cx).trim(),
                "输出 token 上限",
                1,
                32_768,
            ) {
                Ok(v) => v,
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            },
            history_retention_turns: match parse_setting_range(
                self.field("retention", cx).trim(),
                "历史保留轮数",
                0,
                1_000,
            ) {
                Ok(v) => v,
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            },
            context_turn_limit: match parse_setting_range(
                self.field("context-turns", cx).trim(),
                "上下文轮数",
                0,
                1_000,
            ) {
                Ok(v) => v,
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            },
            include_source_history: self
                .settings
                .as_ref()
                .is_some_and(|s| s.include_source_history),
            include_translated_history: self
                .settings
                .as_ref()
                .is_some_and(|s| s.include_translated_history),
            custom_prompt: self.field("prompt", cx),
            source_language: self.field("source-language", cx),
            target_language: self.field("target-language", cx),
            credential_storage_mode: normalize_credential_storage_mode(
                if self.credential_storage_mode.is_empty() {
                    self.settings
                        .as_ref()
                        .map(|s| s.credential_storage_mode.as_str())
                        .unwrap_or("keychain")
                } else {
                    self.credential_storage_mode.as_str()
                },
            ),
            translation_debounce_ms: if self.inputs.contains_key("debounce-ms") {
                match parse_setting_range(
                    self.field("debounce-ms", cx).trim(),
                    "翻译防抖延迟",
                    0,
                    5_000,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        self.error = Some(e);
                        return;
                    }
                }
            } else {
                let stored = self
                    .settings
                    .as_ref()
                    .map(|s| s.translation_debounce_ms)
                    .unwrap_or_else(default_translation_debounce_ms);
                if stored > 5_000 {
                    self.error = Some("翻译防抖延迟 必须在 0 到 5000 之间".into());
                    return;
                }
                stored
            },
            sentence_cache_enabled: self
                .settings
                .as_ref()
                .map(|s| s.sentence_cache_enabled)
                .unwrap_or_else(default_sentence_cache_enabled),
        };
        if settings.custom_prompt.len() > 16_384 {
            self.error = Some("自定义提示最多 16384 UTF-8 字节。".into());
            return;
        }
        let params = json!({"settings": settings, "remoteProvidersEnabled": self.remote_enabled});
        self.request(bridge, "translation.settings/save", params);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_debug_is_redacted_and_models_keep_manual_selection() {
        let mut secret = SecretValue::default();
        secret.set("do-not-log-this".into());
        assert!(!format!("{secret:?}").contains("do-not-log-this"));
        let mut page = TranslationPage {
            draft: ProviderDraft {
                model_id: "manual-choice".into(),
                ..ProviderDraft::default()
            },
            ..TranslationPage::default()
        };
        page.update_models(vec![ModelDescriptorDto {
            id: "discovered".into(),
            display_name: "".into(),
            context_window: None,
            context_window_provenance: "unknown".into(),
            capabilities: vec![],
        }]);
        assert_eq!(page.draft.model_id, "manual-choice");
        page.update_models(vec![]);
        assert_eq!(page.draft.model_id, "manual-choice");
    }

    #[test]
    fn provider_kind_defaults_keep_local_ollama_available() {
        assert_eq!(default_endpoint("ollama"), "http://localhost:11434");
        assert_eq!(default_endpoint("deepL"), "https://api.deepl.com");
    }

    #[test]
    fn model_info_can_use_openai_compatible_model_discovery() {
        assert!(can_inspect_model(false, true));
        assert!(can_inspect_model(true, false));
        assert!(!can_inspect_model(false, false));
    }

    #[test]
    fn provider_language_defaults_and_detected_model_metadata_are_core_compatible() {
        let mut page = TranslationPage::default();
        page.new_provider("deepL");
        assert_eq!(page.draft.source_language, "auto");
        assert_eq!(page.draft.target_language, "en");

        page.models = vec![
            ModelDescriptorDto {
                id: "detected-model".into(),
                display_name: "Detected".into(),
                context_window: Some(16_384),
                context_window_provenance: "detected".into(),
                capabilities: vec![],
            },
            ModelDescriptorDto {
                id: "unknown-model".into(),
                display_name: "Unknown".into(),
                context_window: None,
                context_window_provenance: "unknown".into(),
                capabilities: vec![],
            },
        ];
        let detected_model = page.models[0].clone();
        page.choose_model(&detected_model);
        assert_eq!(page.draft.model_id, "detected-model");
        assert_eq!(
            page.draft
                .model_metadata
                .as_ref()
                .map(|model| model.context_window),
            Some(Some(16_384))
        );
        page.sync_model_metadata("detected-model".into());
        assert_eq!(
            page.draft
                .model_metadata
                .as_ref()
                .map(|model| model.context_window),
            Some(Some(16_384))
        );
        page.sync_model_metadata("unknown-model".into());
        assert_eq!(
            page.draft
                .model_metadata
                .as_ref()
                .map(|model| model.context_window_provenance.as_str()),
            Some("unknown")
        );
        page.sync_model_metadata("manual-model-not-in-list".into());
        assert_eq!(page.draft.model_id, "manual-model-not-in-list");
        assert!(page.draft.model_metadata.is_none());
    }

    #[test]
    fn stale_provider_requests_do_not_match_after_selection_or_new_draft() {
        let mut page = TranslationPage::default();
        page.providers = vec![test_provider("provider-a"), test_provider("provider-b")];
        page.select(Some("provider-a".into()));
        let pending = PendingTranslationRequest {
            method: "translation.models".into(),
            provider_id: Some("provider-a".into()),
            selection_generation: page.selection_generation,
            draft_generation: page.draft_generation,
            settings_edit_generation: page.settings_edit_generation,
        };
        assert!(page.request_matches_provider(&pending));

        page.select(Some("provider-b".into()));
        assert!(!page.request_matches_provider(&pending));

        let selected_b = PendingTranslationRequest {
            method: "translation.modelInfo".into(),
            provider_id: Some("provider-b".into()),
            selection_generation: page.selection_generation,
            draft_generation: page.draft_generation,
            settings_edit_generation: page.settings_edit_generation,
        };
        page.draft_generation = page.draft_generation.wrapping_add(1);
        assert!(!page.request_matches_provider(&selected_b));

        let current_b = PendingTranslationRequest {
            draft_generation: page.draft_generation,
            ..selected_b
        };
        page.new_provider("ollama");
        assert!(!page.request_matches_provider(&current_b));
    }

    #[gpui_kit::test]
    fn input_state_subscription_ignores_focus_and_handles_value_changes(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });
        let (input, pending) = window
            .update(cx, |this, window, cx| {
                this.translation_page.providers = vec![test_provider("provider-a")];
                this.translation_page.select(Some("provider-a".into()));
                this.translation_page.models = vec![ModelDescriptorDto {
                    id: "current-model".into(),
                    display_name: String::new(),
                    context_window: Some(16_384),
                    context_window_provenance: "detected".into(),
                    capabilities: vec![],
                }];
                this.translation_page.draft.model_metadata =
                    this.translation_page.models.first().cloned();
                let pending = PendingTranslationRequest {
                    method: "translation.models".into(),
                    provider_id: Some("provider-a".into()),
                    selection_generation: this.translation_page.selection_generation,
                    draft_generation: this.translation_page.draft_generation,
                    settings_edit_generation: this.translation_page.settings_edit_generation,
                };
                let input = this.translation_page.input(
                    "endpoint",
                    "https://old.example",
                    "Endpoint",
                    window,
                    cx,
                );
                (input, pending)
            })
            .unwrap();

        input.update(cx, |_, cx| cx.emit(InputEvent::Focus));
        input.update(cx, |_, cx| cx.emit(InputEvent::Blur));
        window
            .read_with(cx, |this, _| {
                assert!(this.translation_page.request_matches_provider(&pending));
                assert_eq!(this.translation_page.models.len(), 1);
                assert!(this.translation_page.draft.model_metadata.is_some());
            })
            .unwrap();

        window
            .update(cx, |_, window, cx| {
                input.update(cx, |state, input_cx| {
                    state.set_value("https://new.example", window, input_cx);
                    input_cx.emit(InputEvent::Change);
                });
            })
            .unwrap();
        window
            .read_with(cx, |this, _| {
                assert!(!this.translation_page.request_matches_provider(&pending));
                assert!(this.translation_page.models.is_empty());
                assert!(this.translation_page.draft.model_metadata.is_none());
                assert_eq!(this.translation_page.draft.base_url, "https://new.example");
            })
            .unwrap();
    }

    #[test]
    fn settings_request_generation_is_independent_of_provider_selection() {
        let mut page = TranslationPage::default();
        let pending = PendingTranslationRequest {
            method: "translation.settings/get".into(),
            provider_id: None,
            selection_generation: 0,
            draft_generation: 0,
            settings_edit_generation: page.settings_edit_generation,
        };

        page.select(Some("provider-a".into()));
        assert!(page.settings_response_is_current(&pending));

        page.input_state_event("target-language", &InputEvent::Change, Some("ja"));
        assert!(!page.settings_response_is_current(&pending));
    }

    #[test]
    fn disconnect_clears_pending_translation_requests() {
        let mut page = TranslationPage::default();
        page.pending.insert(
            9,
            PendingTranslationRequest {
                method: "translation.settings/get".into(),
                provider_id: None,
                selection_generation: 0,
                draft_generation: 0,
                settings_edit_generation: 0,
            },
        );

        page.bridge_disconnected(Some("helper exited"));

        assert!(page.pending.is_empty());
        assert!(page.error.as_deref().unwrap().contains("helper exited"));
    }

    #[test]
    fn credential_input_change_does_not_stale_credential_reply() {
        let mut page = TranslationPage::default();
        page.providers = vec![test_provider("provider-a")];
        page.select(Some("provider-a".into()));
        let pending = PendingTranslationRequest {
            method: "translation.credential.set".into(),
            provider_id: Some("provider-a".into()),
            selection_generation: page.selection_generation,
            draft_generation: page.draft_generation,
            settings_edit_generation: page.settings_edit_generation,
        };
        assert!(page.input_state_event("credential", &InputEvent::Change, None));
        assert!(page.request_matches_provider(&pending));
    }

    #[test]
    fn provider_mutations_target_the_requested_row_not_the_current_draft() {
        let mut page = TranslationPage::default();
        page.providers = vec![test_provider("provider-a"), test_provider("provider-b")];
        page.select(Some("provider-b".into()));

        page.set_has_credential("provider-a", true);
        assert!(page.providers[0].has_credential);
        assert!(!page.providers[1].has_credential);
        page.remove_provider("provider-a");
        assert_eq!(page.providers.len(), 1);
        assert_eq!(page.providers[0].id, "provider-b");
        assert_eq!(page.selected_id.as_deref(), Some("provider-b"));
        assert_eq!(page.draft.id.as_deref(), Some("provider-b"));
    }

    #[test]
    fn disconnected_supervisor_returns_an_error_without_leaving_pending_requests() {
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let mut page = TranslationPage::default();
        page.request(
            &bridge,
            "translation.models",
            serde_json::json!({"providerID":"p"}),
        );
        assert!(page.pending.is_empty());
        assert!(
            page.error
                .as_deref()
                .is_some_and(|error| error.contains("disconnected"))
        );
    }

    #[test]
    fn dismiss_clears_the_credential_secret_state() {
        let mut page = TranslationPage::default();
        page.active = true;
        page.secret.set("secret-value".into());
        page.dismiss();
        assert!(!page.active);
        assert!(page.secret.is_empty());
        assert!(!page.inputs.contains_key("credential"));
    }

    #[test]
    fn generated_provider_ids_are_valid_distinct_uuids() {
        let first = new_profile_id();
        let second = new_profile_id();
        assert!(Uuid::parse_str(&first).is_ok());
        assert!(Uuid::parse_str(&second).is_ok());
        assert_ne!(first, second);
    }

    #[test]
    fn empty_profiles_and_discovery_results_are_valid() {
        let profiles: Vec<ProviderDto> = serde_json::from_value(serde_json::json!([])).unwrap();
        assert!(profiles.is_empty());

        let mut page = TranslationPage::default();
        page.update_models(vec![]);
        assert!(page.models.is_empty());
    }

    #[test]
    fn settings_validation_accepts_empty_override_and_zero_history_but_rejects_invalid_ranges() {
        assert_eq!(parse_context_window_override(""), Ok(None));
        assert_eq!(parse_context_window_override("8192"), Ok(Some(8192)));
        assert_eq!(parse_setting_range("0", "历史保留轮数", 0, 1_000), Ok(0));
        assert!(parse_context_window_override("0").is_err());
        assert!(parse_context_window_override("1000001").is_err());
        assert!(parse_setting_range("3.5", "输出 token 上限", 1, 32_768).is_err());
        assert!(parse_setting_range("32769", "输出 token 上限", 1, 32_768).is_err());
    }

    #[test]
    fn provider_write_uses_the_swift_camel_case_contract() {
        let provider = ProviderWriteDto {
            id: "00000000-0000-4000-8000-000000000001".into(),
            name: "Local".into(),
            kind: "ollama".into(),
            base_url: "http://localhost:11434".into(),
            model_id: Some("qwen".into()),
            default_source_language: "auto".into(),
            default_target_language: "en".into(),
            model_metadata: Some(ModelDescriptorDto {
                id: "qwen".into(),
                display_name: "Qwen".into(),
                context_window: Some(8192),
                context_window_provenance: "detected".into(),
                capabilities: vec!["chatCompletion".into()],
            }),
            open_ai_chat_token_limit_field: "max_completion_tokens".into(),
            thinking_level: "off".into(),
        };
        let value = serde_json::to_value(provider).unwrap();
        assert_eq!(value["defaultSourceLanguage"], "auto");
        assert_eq!(value["defaultTargetLanguage"], "en");
        assert_eq!(value["modelMetadata"]["contextWindow"], 8192);
        assert!(value.get("default_source_language").is_none());
        assert!(value.get("model_metadata").is_none());
        assert_eq!(value["baseURL"], "http://localhost:11434");
        assert_eq!(value["modelID"], "qwen");
        assert_eq!(value["openAIChatTokenLimitField"], "max_completion_tokens");
        assert_eq!(value["thinkingLevel"], "off");
        assert!(value.get("thinking_level").is_none());
    }

    #[test]
    fn provider_dto_defaults_thinking_level_and_preserves_explicit_values() {
        let minimal = serde_json::json!({
            "id": "p1",
            "name": "Chat",
            "kind": "openAIChat",
            "baseURL": "https://api.openai.com/v1",
            "modelID": "gpt-5",
            "defaultSourceLanguage": "ja",
            "defaultTargetLanguage": "zh-Hans",
            "modelMetadata": null,
            "openAIChatTokenLimitField": "max_completion_tokens",
            "capabilities": {
                "modelDiscovery": true,
                "contextDiscovery": false,
                "streaming": true,
                "customPrompt": true,
                "context": true
            },
            "hasCredential": true
        });
        let dto: ProviderDto = serde_json::from_value(minimal).unwrap();
        assert_eq!(dto.thinking_level, "default");
        assert_eq!(draft_from_provider(&dto).thinking_level, "default");

        let with_thinking = serde_json::json!({
            "id": "p2",
            "name": "Claude",
            "kind": "anthropic",
            "baseURL": "https://api.anthropic.com/v1",
            "modelID": "claude-sonnet-4-5",
            "defaultSourceLanguage": "ja",
            "defaultTargetLanguage": "zh-Hans",
            "modelMetadata": null,
            "openAIChatTokenLimitField": "max_completion_tokens",
            "thinkingLevel": "xhigh",
            "capabilities": {
                "modelDiscovery": true,
                "contextDiscovery": true,
                "streaming": true,
                "customPrompt": true,
                "context": true
            },
            "hasCredential": false
        });
        let dto_explicit: ProviderDto = serde_json::from_value(with_thinking).unwrap();
        assert_eq!(dto_explicit.thinking_level, "xhigh");
        let mut draft = draft_from_provider(&dto_explicit);
        assert_eq!(draft.thinking_level, "xhigh");

        draft.set_thinking_level("ULTRA");
        assert_eq!(draft.thinking_level, "ultra");
        draft.set_thinking_level("unknown-value");
        assert_eq!(draft.thinking_level, "default");
    }

    #[gpui_kit::test]
    fn switching_between_thinking_and_non_thinking_kinds_updates_thinking_level(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, window, cx| {
                this.translation_page.new_provider("openAIChat");
                assert_eq!(this.translation_page.draft.thinking_level, "default");
                this.translation_page.set_thinking_level("off");
                assert_eq!(this.translation_page.draft.thinking_level, "off");

                // Switching to another reasoning-capable kind preserves thinking level.
                this.translation_page.switch_kind("gemini", window, cx);
                assert_eq!(this.translation_page.draft.thinking_level, "off");

                // Switching to DeepL / openAICompletions resets thinking level to default.
                this.translation_page.switch_kind("deepL", window, cx);
                assert_eq!(this.translation_page.draft.thinking_level, "default");
            })
            .unwrap();
    }

    #[test]
    fn dismiss_and_selection_clear_return_to_capture_and_pending_credential() {
        let mut page = TranslationPage::default();
        page.providers = vec![test_provider("provider-a")];
        page.active = true;
        page.return_to_capture = true;
        page.pending_credential.set("sk-temp".into());
        page.dismiss();
        assert!(!page.active);
        assert!(!page.return_to_capture);
        assert!(page.pending_credential.is_empty());

        page.pending_credential.set("sk-temp".into());
        page.select(Some("provider-a".into()));
        assert!(page.pending_credential.is_empty());

        page.pending_credential.set("sk-temp".into());
        page.new_provider("deepL");
        assert!(page.pending_credential.is_empty());
    }

    #[gpui_kit::test]
    fn return_to_capture_navigation_restores_capture_panel_only_when_opened_from_capture(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, window, cx| {
                // Opened from capture panel -> return_to_capture = true
                this.capture.panel_open = true;
                this.capture.context = Some((
                    "container-1".into(),
                    "game-1".into(),
                    "Demo Game".into(),
                ));
                this.open_translation_settings(cx);
                assert!(!this.capture.panel_open);
                assert!(this.settings);
                assert!(this.translation_page.active);
                assert!(this.translation_page.return_to_capture);
                assert_eq!(
                    this.translation_page.context.as_ref().map(|c| c.2.as_str()),
                    Some("Demo Game")
                );

                this.close_translation_settings(window, cx);
                assert!(!this.settings);
                assert!(!this.translation_page.active);
                assert!(!this.translation_page.return_to_capture);
                assert!(this.capture.panel_open);

                // Opened from application settings -> return_to_capture = false
                this.capture.panel_open = false;
                this.open_translation_settings(cx);
                assert!(this.settings);
                assert!(this.translation_page.active);
                assert!(!this.translation_page.return_to_capture);
                assert!(this.translation_page.context.is_none());

                this.close_translation_settings(window, cx);
                assert!(this.settings);
                assert!(!this.translation_page.active);
                assert!(!this.capture.panel_open);
            })
            .unwrap();
    }

    #[gpui_kit::test]
    fn switch_kind_updates_default_endpoint_and_preserves_custom_endpoint(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, window, cx| {
                this.translation_page.new_provider("ollama");
                let endpoint_input = this.translation_page.input(
                    "endpoint",
                    &this.translation_page.draft.base_url.clone(),
                    "Endpoint",
                    window,
                    cx,
                );
                assert_eq!(
                    endpoint_input.read(cx).value().to_string(),
                    "http://localhost:11434"
                );

                // Switching from default ollama endpoint updates both draft and InputState.
                this.translation_page.switch_kind("deepL", window, cx);
                assert_eq!(this.translation_page.draft.kind, "deepL");
                assert_eq!(
                    this.translation_page.draft.base_url,
                    "https://api.deepl.com"
                );
                assert_eq!(
                    endpoint_input.read(cx).value().to_string(),
                    "https://api.deepl.com"
                );

                // Custom endpoint is preserved when switching kinds.
                this.translation_page.draft.base_url = "https://custom.proxy/v1".into();
                endpoint_input.update(cx, |state, input_cx| {
                    state.set_value("https://custom.proxy/v1", window, input_cx);
                });
                this.translation_page.switch_kind("openAIChat", window, cx);
                assert_eq!(this.translation_page.draft.kind, "openAIChat");
                assert_eq!(
                    this.translation_page.draft.base_url,
                    "https://custom.proxy/v1"
                );
                assert_eq!(
                    endpoint_input.read(cx).value().to_string(),
                    "https://custom.proxy/v1"
                );
            })
            .unwrap();
    }

    #[gpui_kit::test]
    fn pending_credential_chains_after_provider_save_and_clears_on_error(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, window, cx| {
                this.translation_page.new_provider("openAIChat");
                let _name = this.translation_page.input("name", "My OpenAI", "Name", window, cx);
                let _endpoint = this.translation_page.input(
                    "endpoint",
                    "https://api.openai.com/v1",
                    "Endpoint",
                    window,
                    cx,
                );
                let cred_input =
                    this.translation_page
                        .input("credential", "sk-secret-key", "Cred", window, cx);

                // Disconnected bridge clears pending_credential when request fails immediately.
                let disconnected = BridgeSupervisor::disconnected_for_tests();
                this.translation_page.save_provider(&disconnected, window, cx);
                assert!(cred_input.read(cx).value().is_empty());
                assert!(this.translation_page.pending_credential.is_empty());
                assert!(this.translation_page.error.is_some());

                // Error reply for translation.providers/save clears pending_credential.
                this.translation_page.error = None;
                this.translation_page.pending_credential.set("sk-retry".into());
                let err_req = PendingTranslationRequest {
                    method: "translation.providers/save".into(),
                    provider_id: Some("p-err".into()),
                    selection_generation: this.translation_page.selection_generation,
                    draft_generation: this.translation_page.draft_generation,
                    settings_edit_generation: this.translation_page.settings_edit_generation,
                };
                this.translation_page.pending.insert(101, err_req);
                this.translation_page.reply(
                    101,
                    "translation.providers/save",
                    Err("save failed".into()),
                    &disconnected,
                    cx,
                );
                assert!(this.translation_page.pending_credential.is_empty());
                assert_eq!(this.translation_page.error.as_deref(), Some("save failed"));

                // Ok reply for translation.providers/save extracts & clears pending_credential
                // and immediately dispatches translation.credential.set for the saved provider ID.
                this.translation_page.error = None;
                this.translation_page.pending_credential.set("sk-chained".into());
                let ok_req = PendingTranslationRequest {
                    method: "translation.providers/save".into(),
                    provider_id: Some("saved-id".into()),
                    selection_generation: this.translation_page.selection_generation,
                    draft_generation: this.translation_page.draft_generation,
                    settings_edit_generation: this.translation_page.settings_edit_generation,
                };
                this.translation_page.pending.insert(102, ok_req);
                let saved_profile = serde_json::to_value(test_provider("saved-id")).unwrap();
                this.translation_page.reply(
                    102,
                    "translation.providers/save",
                    Ok(saved_profile),
                    &disconnected,
                    cx,
                );
                assert_eq!(
                    this.translation_page.selected_id.as_deref(),
                    Some("saved-id")
                );
                assert!(this.translation_page.pending_credential.is_empty());
                // Disconnected test supervisor proves `translation.credential.set` was dispatched.
                assert!(
                    this.translation_page
                        .error
                        .as_deref()
                        .is_some_and(|e| e.contains("disconnected"))
                );
            })
            .unwrap();
    }

    #[gpui_kit::test]
    fn sync_capture_provider_selection_auto_selects_sole_and_prunes_deleted(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, window, cx| {
                // Sole provider is auto-selected when capture.provider_id is None.
                this.translation_page.providers = vec![test_provider("provider-a")];
                this.capture.provider_id = None;
                this.sync_capture_provider_selection();
                assert_eq!(this.capture.provider_id.as_deref(), Some("provider-a"));

                // With 2+ providers, an unset selection stays None rather than guessing.
                this.translation_page.providers =
                    vec![test_provider("provider-a"), test_provider("provider-b")];
                this.capture.provider_id = None;
                this.sync_capture_provider_selection();
                assert_eq!(this.capture.provider_id, None);

                // Deleting the previously selected provider clears it when 2+ remain.
                this.capture.provider_id = Some("deleted-provider".into());
                this.sync_capture_provider_selection();
                assert_eq!(this.capture.provider_id, None);

                // Deleting down to 1 remaining provider clears the deleted ID and auto-selects the sole survivor.
                this.translation_page.providers = vec![test_provider("provider-b")];
                this.capture.provider_id = Some("provider-a".into());
                this.sync_capture_provider_selection();
                assert_eq!(this.capture.provider_id.as_deref(), Some("provider-b"));

                // Closing translation settings back to capture syncs the sole provider too.
                this.capture.panel_open = true;
                this.capture.provider_id = None;
                this.open_translation_settings(cx);
                this.translation_page.providers = vec![test_provider("provider-b")];
                this.close_translation_settings(window, cx);
                assert!(this.capture.panel_open);
                assert_eq!(this.capture.provider_id.as_deref(), Some("provider-b"));
            })
            .unwrap();
    }

    #[test]
    fn translation_settings_credential_storage_mode_round_trips_and_defaults() {
        let minimal = serde_json::json!({
            "contextWindowOverride": null,
            "outputTokenLimit": 2048,
            "historyRetentionTurns": 20,
            "contextTurnLimit": 8,
            "includeSourceHistory": false,
            "includeTranslatedHistory": false,
            "customPrompt": "",
            "sourceLanguage": "auto",
            "targetLanguage": "zh-Hans"
        });
        let dto: TranslationSettingsDto = serde_json::from_value(minimal).unwrap();
        assert_eq!(dto.credential_storage_mode, "keychain");

        let local_file = TranslationSettingsDto {
            credential_storage_mode: "localFile".into(),
            ..dto.clone()
        };
        let encoded = serde_json::to_value(&local_file).unwrap();
        assert_eq!(encoded["credentialStorageMode"], "localFile");
        assert!(encoded.get("credential_storage_mode").is_none());
        let decoded: TranslationSettingsDto = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.credential_storage_mode, "localFile");

        let session_only: TranslationSettingsDto = serde_json::from_value(serde_json::json!({
            "contextWindowOverride": null,
            "outputTokenLimit": 1024,
            "historyRetentionTurns": 10,
            "contextTurnLimit": 4,
            "includeSourceHistory": true,
            "includeTranslatedHistory": true,
            "customPrompt": "Glossary",
            "sourceLanguage": "ja",
            "targetLanguage": "en",
            "credentialStorageMode": "sessionOnly"
        }))
        .unwrap();
        assert_eq!(session_only.credential_storage_mode, "sessionOnly");

        assert_eq!(
            credential_storage_label("persistentKeychain"),
            "macOS 钥匙串"
        );
        assert_eq!(credential_storage_label("persistentSecretService"), "系统密钥环（Secret Service，加密）");
        assert_eq!(credential_storage_label("unavailable"), "系统凭据存储不可用（请安装 libsecret-tools）");
        assert_eq!(
            credential_storage_label("localPlaintextFile"),
            "本地文件（明文存储，免弹窗）"
        );
        assert_eq!(
            credential_storage_label("sessionOnly"),
            "仅当前会话内存（关闭后清除）"
        );
    }

    #[gpui_kit::test]
    fn translation_page_tracks_and_saves_credential_storage_mode(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let (bridge, drain) = BridgeSupervisor::recording_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, window, cx| {
                this.translation_page.open(&this.bridge);
                let initial_calls = drain();
                assert!(initial_calls.iter().any(|(m, _)| m == "translation.settings/get"));
                let get_id = this
                    .translation_page
                    .pending
                    .iter()
                    .find(|(_, req)| req.method == "translation.settings/get")
                    .map(|(id, _)| *id)
                    .unwrap();

                // Mount required settings inputs before editing/saving.
                this.translation_page
                    .input("output-limit", "2048", "", window, cx);
                this.translation_page
                    .input("retention", "20", "", window, cx);
                this.translation_page
                    .input("context-turns", "8", "", window, cx);
                this.translation_page
                    .input("source-language", "auto", "", window, cx);
                this.translation_page
                    .input("target-language", "zh-Hans", "", window, cx);

                // Selecting localFile bumps settings_edit_generation so stale settings/get replies are ignored.
                this.translation_page.set_credential_storage_mode("localFile");
                assert_eq!(this.translation_page.credential_storage_mode, "localFile");
                assert_eq!(
                    this.translation_page
                        .settings
                        .as_ref()
                        .unwrap()
                        .credential_storage_mode,
                    "localFile"
                );

                this.translation_page.reply(
                    get_id,
                    "translation.settings/get",
                    Ok(serde_json::json!({
                        "settings": {
                            "contextWindowOverride": null,
                            "outputTokenLimit": 2048,
                            "historyRetentionTurns": 20,
                            "contextTurnLimit": 8,
                            "includeSourceHistory": false,
                            "includeTranslatedHistory": false,
                            "customPrompt": "",
                            "sourceLanguage": "auto",
                            "targetLanguage": "en",
                            "credentialStorageMode": "keychain"
                        },
                        "remoteProvidersEnabled": true,
                        "credentialStorage": "persistentKeychain"
                    })),
                    &this.bridge,
                    cx,
                );
                assert_eq!(this.translation_page.credential_storage_mode, "localFile");

                // Save settings sends credentialStorageMode in settings envelope.
                this.translation_page.save_settings(&this.bridge, cx);
                let calls = drain();
                let save_call = calls
                    .iter()
                    .find(|(m, _)| m == "translation.settings/save")
                    .expect("translation.settings/save should be sent");
                assert_eq!(save_call.1["settings"]["credentialStorageMode"], "localFile");
                assert_eq!(save_call.1["remoteProvidersEnabled"], true);

                let save_id = this
                    .translation_page
                    .pending
                    .iter()
                    .find(|(_, req)| req.method == "translation.settings/save")
                    .map(|(id, _)| *id)
                    .unwrap();
                this.translation_page.reply(
                    save_id,
                    "translation.settings/save",
                    Ok(serde_json::json!({
                        "settings": save_call.1["settings"],
                        "remoteProvidersEnabled": true,
                        "credentialStorage": "localPlaintextFile"
                    })),
                    &this.bridge,
                    cx,
                );
                assert_eq!(this.translation_page.credential_storage, "localPlaintextFile");
                assert_eq!(this.translation_page.credential_storage_mode, "localFile");

                // Unknown mode falls back to keychain.
                this.translation_page.set_credential_storage_mode("unexpected");
                assert_eq!(this.translation_page.credential_storage_mode, "keychain");
            })
            .unwrap();
    }

    #[test]
    fn translation_settings_debounce_and_sentence_cache_round_trip_and_default() {
        let minimal = serde_json::json!({
            "contextWindowOverride": null,
            "outputTokenLimit": 2048,
            "historyRetentionTurns": 20,
            "contextTurnLimit": 8,
            "includeSourceHistory": false,
            "includeTranslatedHistory": false,
            "customPrompt": "",
            "sourceLanguage": "auto",
            "targetLanguage": "zh-Hans"
        });
        let dto: TranslationSettingsDto = serde_json::from_value(minimal).unwrap();
        assert_eq!(dto.translation_debounce_ms, 150);
        assert!(dto.sentence_cache_enabled);

        let custom = TranslationSettingsDto {
            translation_debounce_ms: 500,
            sentence_cache_enabled: false,
            ..dto.clone()
        };
        let encoded = serde_json::to_value(&custom).unwrap();
        assert_eq!(encoded["translationDebounceMs"], 500);
        assert_eq!(encoded["sentenceCacheEnabled"], false);
        assert!(encoded.get("translation_debounce_ms").is_none());
        assert!(encoded.get("sentence_cache_enabled").is_none());

        let decoded: TranslationSettingsDto = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.translation_debounce_ms, 500);
        assert!(!decoded.sentence_cache_enabled);
    }

    #[gpui_kit::test]
    fn translation_page_validates_and_saves_debounce_and_sentence_cache(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let (bridge, drain) = BridgeSupervisor::recording_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, window, cx| {
                this.translation_page.open(&this.bridge);
                let initial_calls = drain();
                assert!(initial_calls.iter().any(|(m, _)| m == "translation.cache/list"));

                this.translation_page
                    .input("output-limit", "2048", "", window, cx);
                this.translation_page
                    .input("retention", "20", "", window, cx);
                this.translation_page
                    .input("context-turns", "8", "", window, cx);
                this.translation_page
                    .input("source-language", "auto", "", window, cx);
                this.translation_page
                    .input("target-language", "zh-Hans", "", window, cx);
                this.translation_page
                    .input("debounce-ms", "150", "", window, cx);

                // Toggle sentence cache off and set debounce via helper.
                this.translation_page.toggle_sentence_cache_enabled();
                assert!(!this.translation_page.settings.as_ref().unwrap().sentence_cache_enabled);

                this.translation_page.set_translation_debounce_ms(300);
                assert_eq!(
                    this.translation_page
                        .settings
                        .as_ref()
                        .unwrap()
                        .translation_debounce_ms,
                    300
                );
                assert!(!this.translation_page.inputs.contains_key("debounce-ms"));

                // Save sends updated translationDebounceMs and sentenceCacheEnabled.
                this.translation_page.save_settings(&this.bridge, cx);
                let calls = drain();
                let save_call = calls
                    .iter()
                    .find(|(m, _)| m == "translation.settings/save")
                    .expect("translation.settings/save should be sent");
                assert_eq!(save_call.1["settings"]["translationDebounceMs"], 300);
                assert_eq!(save_call.1["settings"]["sentenceCacheEnabled"], false);

                // Remount debounce input and test out-of-range validation (0..=5000).
                let invalid_input = this
                    .translation_page
                    .input("debounce-ms", "300", "", window, cx);
                invalid_input.update(cx, |state, input_cx| {
                    state.set_value("5001", window, input_cx);
                    input_cx.emit(InputEvent::Change);
                });
                this.translation_page.error = None;
                this.translation_page.save_settings(&this.bridge, cx);
                assert!(this.translation_page.error.is_some());
                assert!(!drain().iter().any(|(m, _)| m == "translation.settings/save"));

                // Valid boundary 0 saves cleanly.
                invalid_input.update(cx, |state, input_cx| {
                    state.set_value("0", window, input_cx);
                    input_cx.emit(InputEvent::Change);
                });
                this.translation_page.error = None;
                this.translation_page.save_settings(&this.bridge, cx);
                let calls = drain();
                let zero_save = calls
                    .iter()
                    .find(|(m, _)| m == "translation.settings/save")
                    .expect("0 ms debounce should be valid");
                assert_eq!(zero_save.1["settings"]["translationDebounceMs"], 0);
            })
            .unwrap();
    }

    #[gpui_kit::test]
    fn translation_page_handles_cache_list_delete_and_clear_replies(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let (bridge, drain) = BridgeSupervisor::recording_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, _, cx| {
                this.translation_page.cache.scope_game_id = Some("key".into());
                this.translation_page.cache.query = "瑚太朗".into();
                this.translation_page
                    .set_cache_only_current_game(&this.bridge, true);
                assert!(this.translation_page.cache.loading);

                let calls = drain();
                let list_call = calls
                    .iter()
                    .find(|(m, _)| m == "translation.cache/list")
                    .expect("translation.cache/list should be sent");
                assert_eq!(list_call.1["gameID"], "key");
                assert_eq!(list_call.1["query"], "瑚太朗");

                let list_id = this
                    .translation_page
                    .pending
                    .iter()
                    .find(|(_, req)| req.method == "translation.cache/list")
                    .map(|(id, _)| *id)
                    .unwrap();
                this.translation_page.reply(
                    list_id,
                    "translation.cache/list",
                    Ok(serde_json::json!({
                        "totalCount": 2,
                        "gameCount": 2,
                        "totalBytes": 4096,
                        "entries": [
                            {
                                "id": "sha-1",
                                "gameID": "key",
                                "speaker": "瑚太朗",
                                "sourceText": "おはよう",
                                "translatedText": "早上好",
                                "providerName": "OpenAI",
                                "providerKind": "openAIChat",
                                "modelID": "gpt-5",
                                "sourceLanguage": "ja",
                                "targetLanguage": "zh-Hans",
                                "hitCount": 3,
                                "updatedAt": 1763200000.0
                            },
                            {
                                "id": "sha-2",
                                "gameID": "other-game",
                                "speaker": null,
                                "sourceText": "さようなら",
                                "translatedText": "再见",
                                "providerName": "DeepL",
                                "providerKind": "deepL",
                                "modelID": null,
                                "sourceLanguage": "ja",
                                "targetLanguage": "zh-Hans",
                                "hitCount": 1,
                                "updatedAt": 1763200100.0
                            }
                        ]
                    })),
                    &this.bridge,
                    cx,
                );
                assert!(!this.translation_page.cache.loading);
                assert!(this.translation_page.cache.loaded_once);
                assert_eq!(this.translation_page.cache.total_count, 2);
                assert_eq!(this.translation_page.cache.game_count, 2);
                assert_eq!(this.translation_page.cache.total_bytes, 4096);
                assert_eq!(this.translation_page.cache.entries.len(), 2);
                assert_eq!(
                    this.translation_page.cache.entries[0].speaker.as_deref(),
                    Some("瑚太朗")
                );

                // Delete entry sha-1.
                this.translation_page.delete_cache_entry(&this.bridge, "sha-1");
                let del_calls = drain();
                let del_call = del_calls
                    .iter()
                    .find(|(m, _)| m == "translation.cache/delete")
                    .unwrap();
                assert_eq!(del_call.1["id"], "sha-1");

                let del_id = this
                    .translation_page
                    .pending
                    .iter()
                    .find(|(_, req)| req.method == "translation.cache/delete")
                    .map(|(id, _)| *id)
                    .unwrap();
                this.translation_page.reply(
                    del_id,
                    "translation.cache/delete",
                    Ok(serde_json::json!({
                        "removed": true,
                        "totalCount": 1,
                        "totalBytes": 2048
                    })),
                    &this.bridge,
                    cx,
                );
                assert_eq!(this.translation_page.cache.total_count, 1);
                assert_eq!(this.translation_page.cache.total_bytes, 2048);
                assert_eq!(this.translation_page.cache.entries.len(), 1);
                assert_eq!(this.translation_page.cache.entries[0].id, "sha-2");

                // Scoped clear for "other-game" removes sha-2.
                this.translation_page
                    .clear_cache(&this.bridge, Some("other-game".into()));
                let clear_scoped_calls = drain();
                let clear_scoped_call = clear_scoped_calls
                    .iter()
                    .find(|(m, _)| m == "translation.cache.clear")
                    .unwrap();
                assert_eq!(clear_scoped_call.1["gameID"], "other-game");

                let clear_scoped_id = this
                    .translation_page
                    .pending
                    .iter()
                    .find(|(_, req)| req.method == "translation.cache.clear")
                    .map(|(id, _)| *id)
                    .unwrap();
                this.translation_page.reply(
                    clear_scoped_id,
                    "translation.cache.clear",
                    Ok(serde_json::json!({
                        "cleared": true,
                        "totalCount": 0,
                        "totalBytes": 0
                    })),
                    &this.bridge,
                    cx,
                );
                assert_eq!(this.translation_page.cache.total_count, 0);
                assert_eq!(this.translation_page.cache.game_count, 0);
                assert_eq!(this.translation_page.cache.total_bytes, 0);
                assert!(this.translation_page.cache.entries.is_empty());

                // Unscoped clear omits gameID.
                this.translation_page.clear_cache(&this.bridge, None);
                let clear_all_calls = drain();
                let clear_all_call = clear_all_calls
                    .iter()
                    .find(|(m, _)| m == "translation.cache.clear")
                    .unwrap();
                assert!(clear_all_call.1.get("gameID").is_none());
            })
            .unwrap();
    }

    fn test_provider(id: &str) -> ProviderDto {
        ProviderDto {
            id: id.into(),
            name: id.into(),
            kind: "ollama".into(),
            base_url: "http://localhost:11434".into(),
            model_id: Some("qwen".into()),
            default_source_language: "auto".into(),
            default_target_language: "en".into(),
            model_metadata: None,
            open_ai_chat_token_limit_field: "max_completion_tokens".into(),
            thinking_level: default_thinking_level(),
            capabilities: ProviderCapabilities {
                model_discovery: true,
                context_discovery: true,
                streaming: true,
                custom_prompt: true,
                context: true,
            },
            has_credential: false,
        }
    }
}

pub(crate) fn default_endpoint(kind: &str) -> &'static str {
    match kind {
        "ollama" => "http://localhost:11434",
        "anthropic" => "https://api.anthropic.com/v1",
        "gemini" => "https://generativelanguage.googleapis.com/v1beta",
        "deepL" => "https://api.deepl.com",
        _ => "https://api.openai.com/v1",
    }
}

pub(crate) fn provider_kind_label(kind: &str) -> &str {
    match kind {
        "openAIChat" => "OpenAI Chat Completions",
        "openAICompletions" => "OpenAI Legacy Completions",
        "ollama" => "Ollama（本地）",
        "deepL" => "DeepL",
        "anthropic" => "Anthropic",
        "gemini" => "Gemini",
        "" => "未选择",
        other => other,
    }
}

fn new_profile_id() -> String {
    Uuid::new_v4().to_string()
}

fn parse_setting_range(raw: &str, label: &str, minimum: u64, maximum: u64) -> Result<u64, String> {
    raw.parse::<u64>()
        .ok()
        .filter(|value| (minimum..=maximum).contains(value))
        .ok_or_else(|| format!("{label} 必须在 {minimum} 到 {maximum} 之间"))
}

fn parse_context_window_override(raw: &str) -> Result<Option<u64>, String> {
    if raw.is_empty() {
        Ok(None)
    } else {
        parse_setting_range(raw, "上下文容量", 1, 1_000_000).map(Some)
    }
}

pub(crate) fn can_inspect_model(context_discovery: bool, model_discovery: bool) -> bool {
    context_discovery || model_discovery
}

#[derive(Clone, Default)]
pub(crate) struct SecretValue(String);

impl SecretValue {
    pub(crate) fn set(&mut self, value: String) {
        self.0 = value;
    }
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretValue([REDACTED])")
    }
}
