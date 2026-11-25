//! Modal sheets: run settings, scan folders, winetricks, create bottle, and diagnostics.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Sizable};
use gpui_kit::{prelude::*, *};

use super::Command;
use crate::bridge::dto::FoldersDto;
use crate::bridge::{BridgeStatus, FolderEdit};
use crate::controls::*;
use crate::program_settings::{self, ProgramSettingsDto, RunEntryDto};
use crate::runtime::ClipboardDecision;
use crate::scan::{self, JobStatus};
use crate::state::{AboutTab, BottleDialogKind, PreviewSheet, ScanState, WhiskyApp};
use crate::theme::RADIUS_LG;
use crate::winetricks::{category_label, filtered_verbs};

fn program_locales() -> Vec<(&'static str, &'static str)> {
    vec![
        ("", crate::i18n::s().program_settings.locale_auto),
        ("ja_JP.UTF-8", "日本語"),
        ("zh_CN.UTF-8", "简体中文"),
        ("zh_TW.UTF-8", "繁體中文"),
        ("en_US.UTF-8", "English"),
        ("de_DE.UTF-8", "Deutsch"),
        ("es_ES.UTF-8", "Español"),
        ("fr_FR.UTF-8", "Français"),
        ("it_IT.UTF-8", "Italiano"),
        ("ko_KR.UTF-8", "한국어"),
        ("ru_RU.UTF-8", "Русский"),
        ("uk_UA.UTF-8", "Українська"),
        ("th_TH.UTF-8", "ไทย"),
    ]
}

fn graphics_backends() -> Vec<(&'static str, &'static str)> {
    vec![
        ("recommended", crate::i18n::s().program_settings.gfx_recommended),
        #[cfg(not(target_os = "linux"))]
        ("d3dMetal", "D3DMetal"),
        ("dxvk", "DXVK"),
        #[cfg(not(target_os = "linux"))]
        ("dxmt", "DXMT"),
        ("wined3d", "WineD3D"),
    ]
}

fn dxvk_huds() -> Vec<(&'static str, &'static str)> {
    let p = &crate::i18n::s().program_settings;
    vec![
        ("full", p.hud_full),
        ("partial", p.hud_partial),
        ("fps", p.hud_fps),
        ("off", crate::i18n::s().common.off),
    ]
}

fn sync_modes() -> Vec<(&'static str, &'static str)> {
    vec![
        ("none", crate::i18n::s().common.off),
        ("esync", "ESync"),
        #[cfg(target_os = "linux")]
        ("fsync", "FSync"),
        #[cfg(not(target_os = "linux"))]
        ("msync", "MSync"),
    ]
}

fn gdi_upscalers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("off", crate::i18n::s().common.off),
        ("metalfx", "MetalFX Spatial"),
        ("anime4k", "Anime4K (CNN x2 S)"),
        ("realcugan", "Real-CUGAN (models-se x2)"),
        ("adaptive", crate::i18n::s().dialogs.dynamic_metalfx_static_real_cugan),
        ("realesrgan", "Real-ESRGAN AnimeVideo-v3 (x2)"),
        ("adaptive-realesrgan", crate::i18n::s().dialogs.dynamic_metalfx_static_animevideo_v3),
        ("adaptive-anime4k", crate::i18n::s().dialogs.dynamic_anime4k_static_real_cugan),
        ("adaptive-anime4k-realesrgan", crate::i18n::s().dialogs.dynamic_anime4k_static_animevideo_v3),
    ]
}

fn gdi_caches() -> Vec<(&'static str, &'static str)> {
    vec![
        ("off", crate::i18n::s().dialogs.cross_scene_cache_off),
        ("memory", crate::i18n::s().dialogs.memory_cache_this_session),
        ("persistent", crate::i18n::s().dialogs.cache_and_save_to_disk),
    ]
}

fn resolutions() -> Vec<(&'static str, &'static str)> {
    vec![
        ("r1280x720", "1280 × 720"),
        ("r1600x900", "1600 × 900"),
        ("r1920x1080", "1920 × 1080"),
        ("r2560x1440", "2560 × 1440"),
        ("r3840x2160", "3840 × 2160"),
        #[cfg(not(target_os = "linux"))]
        ("matchDisplay", crate::i18n::s().dialogs.match_display),
        ("custom", crate::i18n::s().common.custom),
    ]
}

fn dll_modes() -> Vec<(&'static str, &'static str)> {
    let p = &crate::i18n::s().program_settings;
    vec![
        ("b", p.dll_builtin_only),
        ("n", p.dll_native_only),
        ("n,b", p.dll_native_first),
        ("b,n", p.dll_builtin_first),
        ("", p.dll_disabled),
    ]
}

pub(crate) const GPL_LICENSE_TEXT: &str = include_str!("../../../../LICENSE");
pub(crate) const THIRD_PARTY_NOTICES_TEXT: &str =
    include_str!("../../../../THIRD-PARTY-NOTICES.md");

fn about_tabs() -> [(AboutTab, &'static str, &'static str); 4] {
    let d = &crate::i18n::s().dialogs;
    [
        (AboutTab::Overview, "about-tab-overview", crate::i18n::s().dialogs.about_tab_overview),
        (AboutTab::Acknowledgments, "about-tab-ack", d.about_view_ack),
        (AboutTab::License, "about-tab-license", crate::i18n::s().dialogs.about_tab_license),
        (AboutTab::Notices, "about-tab-notices", crate::i18n::s().dialogs.third_party_notices),
    ]
}

struct AcknowledgmentItem {
    #[allow(dead_code)]
    name: &'static str,
    badge: &'static str,
    highlighted: bool,
}

impl AcknowledgmentItem {
    fn display_name(&self) -> &'static str {
        match self.badge {
            "Apache-2.0" => crate::i18n::s().dialogs.gpui_kit_zed_gpui_rust_ecosystem,
            "GoldenPotato137/PotatoVN · Apache-2.0" => {
                crate::i18n::s().dialogs.potatovn_public_metadata_apis
            }
            _ => self.name,
        }
    }

    fn display_description(&self) -> &'static str {
        match self.badge {
            "HIllya51/LunaTranslator · GPL-3.0" => crate::i18n::s().dialogs.ack_lunahook_desc,
            "whisky-app/whisky / frankea/Whisky · GPL-3.0" => crate::i18n::s().dialogs.ack_whisky_desc,
            "LGPL-2.1+" => crate::i18n::s().dialogs.ack_wine_desc,
            "LGPL-2.1+ · GPL-2.0+ / LGPL-2.1+" => crate::i18n::s().dialogs.ack_winetricks_desc,
            "Apache-2.0" => crate::i18n::s().dialogs.ack_gpui_desc,
            "bilibili/ailab & nihui · MIT / bloc97 · MIT" => crate::i18n::s().dialogs.ack_realcugan_desc,
            _ => crate::i18n::s().dialogs.ack_potatovn_desc,
        }
    }
}

const ACKNOWLEDGMENTS: &[AcknowledgmentItem] = &[
    AcknowledgmentItem {
        name: "LunaHook / LunaTranslator",
        badge: "HIllya51/LunaTranslator · GPL-3.0",
        highlighted: true,
    },
    AcknowledgmentItem {
        name: "Whisky & Contributors",
        badge: "whisky-app/whisky / frankea/Whisky · GPL-3.0",
        highlighted: false,
    },
    AcknowledgmentItem {
        name: "Wine / CrossOver Wine & GE-Proton",
        badge: "LGPL-2.1+",
        highlighted: false,
    },
    AcknowledgmentItem {
        name: "Winetricks & cabextract / libmspack",
        badge: "LGPL-2.1+ · GPL-2.0+ / LGPL-2.1+",
        highlighted: false,
    },
    AcknowledgmentItem {
        name: "gpui-kit / Zed GPUI & Rust",
        badge: "Apache-2.0",
        highlighted: false,
    },
    AcknowledgmentItem {
        name: "Real-CUGAN & Anime4K",
        badge: "bilibili/ailab & nihui · MIT / bloc97 · MIT",
        highlighted: false,
    },
    AcknowledgmentItem {
        name: "PotatoVN",
        badge: "GoldenPotato137/PotatoVN · Apache-2.0",
        highlighted: false,
    },
];

impl WhiskyApp {
    fn demo_run_settings(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        self.form(
            "run-settings-scroll",
            vec![
                self.section(
                    "run-config",
                    crate::i18n::s().dialogs.sheet_run_settings,
                    vec![
                        self.info(
                            crate::i18n::s().program_settings.program_label,
                            self.game(index)
                                .map(|game| game.title.clone())
                                .unwrap_or_default(),
                        ),
                        self.choice(
                            "locale",
                            crate::i18n::s().program_settings.locale_label,
                            &[
                                crate::i18n::s().common.inherit_bottle,
                                crate::i18n::s().dialogs.japanese_japan,
                                crate::i18n::s().dialogs.simplified_chinese_china,
                                crate::i18n::s().dialogs.english_us,
                            ],
                            cx,
                        ),
                        self.field("arguments", crate::i18n::s().program_settings.launch_arguments),
                    ],
                    cx,
                ),
                self.section(
                    "env-presets",
                    crate::i18n::s().program_settings.presets_section,
                    vec![self.choice(
                        "environment-preset",
                        crate::i18n::s().common.preset,
                        &crate::i18n::s().program_settings.demo_env_presets,
                        cx,
                    )],
                    cx,
                ),
                self.section(
                    "environment",
                    crate::i18n::s().bottle.env_section,
                    vec![
                        row()
                            .gap(px(10.))
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.preview.inputs["env-key"])),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.preview.inputs["env-value"])),
                            )
                            .into_any_element(),
                        self.action("add-env", crate::i18n::s().program_settings.add_env_btn, Command::Notice, cx),
                    ],
                    cx,
                ),
                self.section(
                    "overrides",
                    crate::i18n::s().dialogs.program_overrides,
                    vec![
                        self.toggle(
                            "override-graphics",
                            crate::i18n::s().program_settings.override_graphics,
                            crate::i18n::s().dialogs.inherits_bottle_settings_when_off,
                            cx,
                        ),
                        self.choice(
                            "program-backend",
                            crate::i18n::s().dialogs.graphics_backend,
                            #[cfg(not(target_os = "linux"))]
                            &[crate::i18n::s().common.inherit_bottle, "D3DMetal", "DXVK", "WineD3D"],
                            #[cfg(target_os = "linux")]
                            &[crate::i18n::s().common.inherit_bottle, "DXVK", "WineD3D"],
                            cx,
                        ),
                        self.toggle("override-sync", crate::i18n::s().program_settings.override_sync, "", cx),
                        self.toggle("override-input", crate::i18n::s().program_settings.override_input, "", cx),
                        self.toggle("override-display", crate::i18n::s().program_settings.override_display, "", cx),
                        self.action(
                            "program-dll",
                            crate::i18n::s().program_settings.override_dll,
                            Command::Sheet(PreviewSheet::Dll),
                            cx,
                        ),
                    ],
                    cx,
                ),
                self.section(
                    "run-diagnostics",
                    crate::i18n::s().dialogs.diagnostics,
                    vec![
                        self.info(
                            crate::i18n::s().dialogs.last_run,
                            crate::i18n::s().program_settings.demo_last_run_value,
                        ),
                        self.action(
                            "run-troubleshoot",
                            crate::i18n::s().dialogs.troubleshoot,
                            Command::Sheet(PreviewSheet::Diagnostics),
                            cx,
                        ),
                    ],
                    cx,
                ),
                self.section(
                    "console",
                    crate::i18n::s().dialogs.console_run_history,
                    vec![
                        self.info(crate::i18n::s().program_settings.just_now, crate::i18n::s().program_settings.demo_exit_ok),
                        muted("[wine] Program started\n[wine] Process exited successfully")
                            .font_family("monospace")
                            .p(px(12.))
                            .into_any_element(),
                    ],
                    cx,
                ),
            ],
        )
    }
    /// Real run settings: the draft lives in `program_settings`, and the
    /// footer's save sends one `programs.settings.set` patch.
    fn run_settings(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.demo {
            return self.demo_run_settings(index, cx);
        }
        let mut sections = Vec::new();
        if let Some(launch_section) = self.launch_entry_section(index, window, cx) {
            sections.push(launch_section);
        }
        if self.program_settings.loading && self.program_settings.settings.is_none() {
            sections.push(self.section(
                "program-loading",
                crate::i18n::config::section_title("program"),
                vec![
                    row()
                        .gap(px(8.))
                        .py(px(12.))
                        .child(Spinner::new().with_size(px(14.)))
                        .child(muted(crate::i18n::s().program_settings.reading_program_settings))
                        .into_any_element(),
                ],
                cx,
            ));
            return self.form("run-settings-scroll", sections);
        }
        let Some(settings) = self.program_settings.settings.clone() else {
            if let Some(reason) = self.program_settings.error.clone() {
                sections.push(self.section(
                    "program-load-error",
                    crate::i18n::config::section_title("program"),
                    vec![
                        muted(reason).py(px(10.)).into_any_element(),
                        Button::new("program-settings-retry")
                            .label(crate::i18n::s().common.retry)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.retry_program_settings(cx)),
                            )
                            .into_any_element(),
                    ],
                    cx,
                ));
            } else {
                let supports_pick = self
                    .game(index)
                    .is_some_and(crate::model::Game::supports_multi_executable);
                let multi_exe = self.game(index).is_some_and(|g| g.multi_executable);
                let mut rows: Vec<AnyElement> = vec![
                    muted(crate::i18n::s().program_settings.no_exe_hint)
                        .text_size(px(12.))
                        .line_height(relative(1.45))
                        .py(px(4.))
                        .into_any_element(),
                ];
                if supports_pick {
                    rows.push(
                        row()
                            .py(px(6.))
                            .gap(px(8.))
                            .child(
                                Button::new("run-settings-pick-exe")
                                    .label(if multi_exe {
                                        crate::i18n::s().program_settings.pick_multi_entries_now
                                    } else {
                                        crate::i18n::s().program_settings.multi_exe_pick_ellipsis
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if multi_exe {
                                            this.open_multi_executable_picker(
                                                index, false, true, false, cx,
                                            );
                                        } else {
                                            this.open_executable_picker(index, false, cx);
                                        }
                                    })),
                            )
                            .into_any_element(),
                    );
                }
                sections.push(self.section("program-no-exe", crate::i18n::config::section_title("program"), rows, cx));
            }
            return self.form("run-settings-scroll", sections);
        };

        if let Some(error) = &self.program_settings.error {
            sections.push(self.section(
                "program-settings-error",
                crate::i18n::s().dialogs.save_failed,
                vec![muted(error.clone()).into_any_element()],
                cx,
            ));
        }
        sections.push(self.program_config_section(index, &settings, window, cx));
        if let Some(translation_section) = self.program_translation_section(index, cx) {
            sections.push(translation_section);
        }
        if cfg!(target_os = "macos") {
            let p = &crate::i18n::s().program_settings;
            let mode = self.program_settings.region_picker_mode();
            let status = if mode == "off" { p.upscale_region_off }
                else if mode == "auto" { p.upscale_region_auto }
                else { p.upscale_region_saved };
            sections.push(self.section("program-upscale-region", p.upscale_region_section, vec![
                muted(status).into_any_element(),
                muted(p.upscale_region_note).into_any_element(),
                row().gap(px(8.)).flex_wrap()
                    .child(Button::new("region-auto").label(p.use_saved_region)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.program_settings.set_region_picker_mode("auto".into()); cx.notify();
                        })))
                    .child(Button::new("region-redraw").label(p.redraw_next_launch)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let token = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default().as_nanos().to_string();
                            this.program_settings.set_region_picker_mode(token); cx.notify();
                        })))
                    .child(Button::new("region-off").label(p.disable_exclusion)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.program_settings.set_region_picker_mode("off".into()); cx.notify();
                        }))).into_any_element(),
            ], cx));
        }
        sections.push(self.program_env_section(window, cx));
        sections.extend(self.program_override_sections(&settings, window, cx));
        sections.push(self.program_history_section(cx));
        sections.push(self.program_shortcut_section(window, cx));
        self.form("run-settings-scroll", sections)
    }

    fn program_translation_section(
        &self,
        index: usize,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let game = self.game(index)?;
        if game.container.is_empty() {
            return None;
        }
        let container = game.container.clone();
        let entry_id = game.id.clone();
        let cached_preset = self.capture.preset_for_game(&container, &entry_id);
        let preset = cached_preset.cloned().unwrap_or_default();
        let preset_mutating = self.capture.is_pending("translation.capture.preset/save")
            || self.capture.is_pending("translation.capture.preset/clear");
        let preset_loading = self.capture.is_pending("translation.capture.preset/get");
        let remembered_process = preset
            .target_exe_path
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(crate::capture::exe_basename);
        let p = &crate::i18n::s().program_settings;
        let status_summary = if preset_loading && cached_preset.is_none() {
            p.reading_translation_preset.to_owned()
        } else {
            let base = if preset.translation_enabled {
                p.live_translation_enabled
            } else {
                p.live_translation_disabled
            };
            match remembered_process {
                Some(exe) => crate::i18n::s().dialogs.remembered_process(base, exe),
                None => crate::i18n::s().dialogs.no_remembered_process(base),
            }
        };
        let enabled = preset.translation_enabled;
        let prompt_on_launch = preset.prompt_attach_on_launch;
        let container_for_enable = container.clone();
        let entry_for_enable = entry_id.clone();
        let container_for_prompt = container.clone();
        let entry_for_prompt = entry_id.clone();
        let hud_available = self.capture_hud_available();
        let hud_pending = self.capture.is_pending("translation.capture.hud/show");

        Some(self.section(
            "program-translation",
            p.live_translation_section,
            vec![
                col()
                    .gap(px(10.))
                    .py(px(6.))
                    .child(muted(status_summary).line_height(relative(1.4)))
                    .child(
                        row()
                            .gap(px(8.))
                            .flex_wrap()
                            .child(selectable_pill(
                                "run-settings-translation-enabled",
                                crate::i18n::s().capture.enable_for_game,
                                enabled,
                                preset_mutating,
                                cx.listener(move |this, _, _, cx| {
                                    this.sync_capture_provider_selection();
                                    this.capture.toggle_preset_translation_enabled(
                                        &this.bridge,
                                        &container_for_enable,
                                        &entry_for_enable,
                                        !enabled,
                                    );
                                    cx.notify();
                                }),
                            ))
                            .child(selectable_pill(
                                "run-settings-prompt-on-launch",
                                crate::i18n::s().capture.popup_hud_on_launch,
                                prompt_on_launch,
                                preset_mutating || !enabled,
                                cx.listener(move |this, _, _, cx| {
                                    this.capture.toggle_preset_prompt_on_launch(
                                        &this.bridge,
                                        &container_for_prompt,
                                        &entry_for_prompt,
                                        !prompt_on_launch,
                                    );
                                    cx.notify();
                                }),
                            )),
                    )
                    .child(
                        row()
                            .gap(px(8.))
                            .flex_wrap()
                            .child(
                                Button::new("run-settings-open-capture")
                                    .label(p.open_capture_panel_ellipsis)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.preview.sheet = None;
                                        this.preview.sheet_history.clear();
                                        this.open_translation_for_game(index, cx);
                                    })),
                            )
                            .when(hud_available, |r| {
                                r.child(
                                    Button::new("run-settings-open-hud")
                                        .label(p.popup_hud_btn)
                                        .disabled(hud_pending)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.capture.open_capture_hud(
                                                &this.bridge,
                                                &container,
                                                &entry_id,
                                            );
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .into_any_element(),
            ],
            cx,
        ))
    }

    fn program_input(
        &mut self,
        key: &str,
        value: &str,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let key = key.to_owned();
        if let Some(input) = self.preview.program_inputs.get(&key) {
            return input.clone();
        }
        let value = value.to_owned();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(value)
        });
        self.preview.program_inputs.insert(key, input.clone());
        input
    }

    /// The reason an engine policy disables `field` on the program page.
    fn program_field_reason(&self, field: &str) -> Option<String> {
        self.program_settings
            .container
            .as_deref()
            .and_then(|container| self.engine_field_policy(container, field))
            .filter(|policy| policy.disabled)
            .and_then(|policy| policy.reason.clone())
    }

    /// An engine-gated program override switch, same rules as the bottle
    /// config page: gated renders visibly disabled with the reason; a saved
    /// ON stays switchable OFF; `unverified` shows an explicit
    /// "确认后尝试启用" action instead of a live switch.
    fn program_engine_switch(
        &self,
        id: String,
        label: &'static str,
        checked: bool,
        field: &'static str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let container = self.program_settings.container.clone();
        let policy = container
            .as_deref()
            .and_then(|container| self.engine_field_policy(container, field))
            .cloned();
        let gated = policy.as_ref().is_some_and(|policy| policy.disabled);
        let confirmable = policy
            .as_ref()
            .is_some_and(|policy| policy.state == "unverified" && policy.can_override);
        let feature = policy
            .as_ref()
            .and_then(|policy| policy.feature.clone())
            .unwrap_or_default();
        let mut column = col().gap(px(4.));
        if let Some(reason) = self.program_field_reason(field) {
            column = column.child(muted(reason).text_size(px(11.)).line_height(relative(1.4)));
        }
        // A saved ON the engine does not honor is an inactive preference —
        // shown as such, still switchable to OFF.
        if gated && checked {
            column = column.child(
                muted(crate::i18n::s().bottle.saved_pref_inactive)
                    .text_size(px(11.))
                    .line_height(relative(1.4)),
            );
        }
        column = column.child(self.program_switch(
            id,
            label,
            checked,
            gated && !checked,
            move |this, on, cx| this.set_program_override_bool(field, on, cx),
            cx,
        ));
        if confirmable {
            let container = container.clone().unwrap_or_default();
            let field_id = field.to_owned();
            column = column.child(
                Button::new(format!("program-confirm:{field_id}"))
                    .label(crate::i18n::s().bottle.confirm_and_try_enable)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request_program_feature_confirm(
                            &container,
                            &feature,
                            &field_id,
                            serde_json::Value::Bool(true),
                            cx,
                        );
                    })),
            );
        }
        column.into_any_element()
    }

    fn program_switch(
        &self,
        id: String,
        label: &'static str,
        checked: bool,
        disabled: bool,
        on_change: impl Fn(&mut WhiskyApp, bool, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> AnyElement {
        Switch::new(id)
            .label(label)
            .checked(checked)
            .disabled(disabled)
            .on_change(cx.listener(move |this, checked, _, cx| {
                on_change(this, *checked, cx);
                cx.notify();
            }))
            .into_any_element()
    }

    fn program_choice(
        &self,
        id: String,
        current: String,
        current_value: &str,
        options: &[(&'static str, &'static str)],
        field: &'static str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let options: Vec<(&'static str, &'static str)> = options.to_vec();
        let container = self.program_settings.container.clone();
        let field_policy = container
            .as_deref()
            .and_then(|container| self.engine_field_policy(container, field))
            .cloned();
        // Field-level gate fallback: fields like `dxvkHud` carry the policy
        // on the FIELD, not per choice — every non-neutral option inherits
        // it; off/none stay selectable so a saved value can be reverted.
        let neutral = |id: &str| id.is_empty() || id == "none" || id == "off";
        let policy_for = move |option_id: &str| -> Option<crate::bridge::dto::EnginePolicyDto> {
            if let Some(policy) = field_policy
                .as_ref()
                .and_then(|policy| policy.choices.get(option_id))
            {
                return Some(policy.clone());
            }
            let policy = field_policy.as_ref()?;
            if policy.disabled && !neutral(option_id) {
                return Some(crate::bridge::dto::EnginePolicyDto {
                    disabled: policy.disabled,
                    reason: policy.reason.clone(),
                    state: policy.state.clone(),
                    can_override: policy.can_override,
                    feature: policy.feature.clone(),
                });
            }
            None
        };
        let selected_gated = policy_for(current_value).is_some_and(|policy| policy.disabled);
        let label = if selected_gated {
            format!("{}  ⌄", crate::i18n::s().program_settings.value_inactive(&current))
        } else {
            format!("{current}  ⌄")
        };
        let view = cx.entity().downgrade();
        Button::new(id)
            .label(label)
            .dropdown_menu(move |mut menu, _, _| {
                menu = menu.scrollable(true);
                for &(raw, display) in &options {
                    let view = view.clone();
                    let choice_policy = policy_for(raw);
                    let confirmable = choice_policy.as_ref().is_some_and(|policy| {
                        policy.disabled && policy.can_override && policy.state == "unverified"
                    });
                    let unavailable = choice_policy.as_ref().is_some_and(|policy| {
                        policy.disabled && !(policy.can_override && policy.state == "unverified")
                    });
                    let title = if confirmable {
                        crate::i18n::s().program_settings.value_requires_confirm(display)
                    } else {
                        display.to_owned()
                    };
                    menu = menu.item(PopupMenuItem::new(title).disabled(unavailable).on_click(
                        move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| {
                                if field == "locale" {
                                    this.set_program_locale(raw.to_owned(), cx);
                                } else {
                                    this.set_program_choice(field, raw, cx);
                                    cx.notify();
                                }
                            });
                        },
                    ));
                }
                menu
            })
            .into_any_element()
    }

    fn program_choice_row(&self, label: &'static str, control: AnyElement) -> AnyElement {
        row()
            .gap(px(16.))
            .py(px(10.))
            .child(div().flex_1().min_w_0().child(label))
            .child(control)
            .into_any_element()
    }

    fn program_config_section(
        &mut self,
        index: usize,
        settings: &ProgramSettingsDto,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let arguments = self.program_input(
            "ps-args",
            &settings.arguments,
            crate::i18n::s().program_settings.args_placeholder,
            window,
            cx,
        );
        let locale = self.program_choice(
            "ps-locale".into(),
            program_settings::locale_label(&settings.locale).to_owned(),
            &settings.locale,
            &program_locales(),
            "locale",
            cx,
        );
        let game = self.game(index);
        let multi_exe = game.is_some_and(|g| g.multi_executable && g.launch_executables.len() >= 2);
        let folder = game
            .and_then(|g| g.game_directory())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let p = &crate::i18n::s().program_settings;
        let mut rows = vec![self.info(p.program_label, settings.program_name.clone())];
        if multi_exe && !folder.is_empty() {
            let (rel_dir, _) =
                crate::bridge::dto::relative_directory_label(&settings.executable, &folder);
            rows.push(self.info(p.relative_path, rel_dir));
        }
        rows.push(self.info(
            p.executable_label,
            if settings.available {
                p.available_label
            } else {
                p.file_missing
            },
        ));
        rows.push(self.program_choice_row(p.locale_label, locale));
        rows.push(
            col()
                .gap(px(7.))
                .py(px(8.))
                .child(p.launch_arguments)
                .child(Input::new(&arguments))
                .into_any_element(),
        );
        if multi_exe {
            rows.push(
                muted(crate::i18n::s().dialogs.per_program_scope_tip(&settings.program_name))
                .text_size(px(11.5))
                .line_height(relative(1.4))
                .py(px(4.))
                .into_any_element(),
            );
        }
        self.section("program-config", crate::i18n::config::section_title("program"), rows, cx)
    }

    fn program_env_section(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows: Vec<_> = self.program_settings.env_rows.iter()
            .filter(|row| row.key != "WINE_GDI_UPSCALER_PICK").cloned().collect();
        // Locale/renderer presets only rewrite Wine variables (LANG,
        // WINE_D3D_CONFIG); native Windows programs ignore them.
        let mut children: Vec<AnyElement> = if cfg!(target_os = "windows") {
            Vec::new()
        } else {
            vec![self.program_env_presets(cx)]
        };
        if rows.is_empty() {
            children.push(muted(crate::i18n::s().program_settings.no_program_env).py(px(8.)).into_any_element());
        }
        for env_row in rows {
            let key = self.program_input(
                &format!("ps-env-key-{}", env_row.id),
                &env_row.key,
                crate::i18n::s().bottle.env_key_placeholder,
                window,
                cx,
            );
            let value = self.program_input(
                &format!("ps-env-value-{}", env_row.id),
                &env_row.value,
                crate::i18n::s().bottle.env_value_placeholder,
                window,
                cx,
            );
            let id = env_row.id;
            children.push(
                row()
                    .gap(px(10.))
                    .py(px(5.))
                    .child(div().flex_1().min_w_0().child(Input::new(&key)))
                    .child(div().flex_1().min_w_0().child(Input::new(&value)))
                    .child(
                        Button::new(format!("ps-env-remove-{id}"))
                            .label("−")
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.remove_program_env_row(id, cx);
                            })),
                    )
                    .into_any_element(),
            );
        }
        children.push(
            row()
                .py(px(6.))
                .child(
                    Button::new("ps-env-add")
                        .label(crate::i18n::s().program_settings.add_env_btn)
                        .on_click(cx.listener(|this, _, _, cx| this.add_program_env_row(cx))),
                )
                .into_any_element(),
        );
        self.section("program-env", crate::i18n::s().bottle.env_section, children, cx)
    }

    /// One selectable pill of the quick-add presets.
    fn preset_chip(
        &self,
        id: String,
        label: &'static str,
        selected: bool,
        on_click: impl Fn(&mut WhiskyApp, &mut Context<WhiskyApp>) + 'static,
        cx: &Context<Self>,
    ) -> AnyElement {
        let element_id = SharedString::from(id);
        row()
            .id(element_id.clone())
            .gap(px(5.))
            .items_center()
            .h(px(26.))
            .px(px(11.))
            .rounded(px(13.))
            .border_1()
            .cursor_pointer()
            .text_size(px(12.))
            .border_color(rgba(if selected { crate::theme::accent_subtle_border() } else { crate::theme::surface_border() }))
            .bg(rgba(if selected { crate::theme::accent_subtle_bg() } else { crate::theme::card_bg() }))
            .text_color(rgb(if selected { crate::theme::pill_selected_text() } else { crate::theme::subtext() }))
            .hover(|s| s.bg(rgba(if selected { crate::theme::accent_subtle_hover() } else { crate::theme::hover_strong() })))
            .when(selected, |d| {
                d.child(icon("check", 11.).text_color(rgb(crate::theme::accent_fg())))
            })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
            .key_focus(element_id)
            .into_any_element()
    }

    /// Quick presets from the SwiftUI editor: locale and WineD3D renderer.
    /// They rewrite LANG/LC_ALL and WINE_D3D_CONFIG in the draft rows below.
    fn program_env_presets(&self, cx: &Context<Self>) -> AnyElement {
        let locale = self
            .program_settings
            .settings
            .as_ref()
            .map(|settings| settings.locale.clone())
            .unwrap_or_default();
        let lang = self.program_env_value("LANG").map(str::to_owned);
        let lc_all = self.program_env_value("LC_ALL").map(str::to_owned);
        let locale_selected = |value: &str| {
            if value.is_empty() {
                locale.is_empty() && lang.is_none() && lc_all.is_none()
            } else {
                locale == value
                    && lang.as_deref() == Some(value)
                    && lc_all.as_deref() == Some(value)
            }
        };
        let mut locales = row().gap(px(8.)).flex_wrap();
        for (value, label) in [
            ("ja_JP.UTF-8", "日本語"),
            ("zh_CN.UTF-8", "简体中文"),
            ("zh_TW.UTF-8", "繁體中文"),
            ("", crate::i18n::s().program_settings.locale_auto),
        ] {
            locales = locales.child(self.preset_chip(
                format!("env-locale-{value}"),
                label,
                locale_selected(value),
                move |this, cx| this.set_program_locale(value.to_owned(), cx),
                cx,
            ));
        }
        let renderer = self.program_d3d_renderer();
        let mut renderers = row().gap(px(8.)).flex_wrap();
        for (value, label) in [
            ("", crate::i18n::s().common.default_label),
            ("gdi", "GDI"),
            ("gl", "OpenGL"),
            ("vulkan", "Vulkan"),
        ] {
            renderers = renderers.child(self.preset_chip(
                format!("env-renderer-{value}"),
                label,
                renderer.as_deref() == Some(value),
                move |this, cx| this.set_program_renderer(value, cx),
                cx,
            ));
        }
        let group = |title: &'static str, chips: Div| {
            col()
                .gap(px(7.))
                .child(muted(title).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD))
                .child(chips)
        };
        col()
            .gap(px(14.))
            .py(px(8.))
            .child(group(crate::i18n::s().program_settings.locale_label, locales))
            .child(
                group(crate::i18n::s().program_settings.wined3d_renderer, renderers)
            )
            .child(
                muted(crate::i18n::s().program_settings.presets_note)
                    .text_size(px(11.))
                    .line_height(relative(1.4)),
            )
            .child(div().h(px(1.)).bg(rgba(crate::theme::surface_border())).mt(px(2.)))
            .into_any_element()
    }

    fn program_override_sections(
        &mut self,
        settings: &ProgramSettingsDto,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        // Every override group is a Wine translation knob (DXVK, esync,
        // virtual desktop, dll overrides); none of it exists for native
        // Windows programs.
        if cfg!(target_os = "windows") {
            let _ = (settings, window);
            return Vec::new();
        }
        vec![
            self.program_graphics_section(settings, cx),
            self.program_sync_section(settings, cx),
            self.program_upscale_section(settings, cx),
            self.program_performance_section(settings, cx),
            self.program_input_section(settings, cx),
            self.program_display_section(settings, window, cx),
            self.program_dll_section(settings, window, cx),
        ]
    }

    fn program_graphics_section(
        &self,
        settings: &ProgramSettingsDto,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled = settings.graphics_enabled();
        let overrides = settings.overrides.as_ref();
        let defaults = &settings.defaults;
        let mut children = vec![self.program_switch(
            "ps-override-graphics".into(),
            crate::i18n::s().program_settings.override_graphics,
            enabled,
            false,
            |this, on, _cx| this.set_program_override_group("graphics", on),
            cx,
        )];
        if enabled {
            let backend = overrides
                .and_then(|value| value.graphics_backend.clone())
                .or_else(|| defaults.graphics_backend.clone())
                .unwrap_or_else(|| "recommended".into());
            children.push(self.program_choice_row(
                crate::i18n::s().dialogs.graphics_backend,
                self.program_choice(
                    "ps-graphics-backend".into(),
                    program_settings::graphics_label(&backend).to_owned(),
                    &backend,
                    &graphics_backends(),
                    "graphicsBackend",
                    cx,
                ),
            ));
            let async_on = overrides
                .and_then(|value| value.dxvk_async)
                .or(defaults.dxvk_async)
                .unwrap_or(true);
            children.push(self.program_engine_switch(
                "ps-dxvk-async".into(),
                "DXVK Async",
                async_on,
                "dxvkAsync",
                cx,
            ));
            let hud = overrides
                .and_then(|value| value.dxvk_hud.clone())
                .or_else(|| defaults.dxvk_hud.clone())
                .unwrap_or_else(|| "off".into());
            children.push(self.program_choice_row(
                "DXVK HUD",
                self.program_choice(
                    "ps-dxvk-hud".into(),
                    program_settings::hud_label(&hud).to_owned(),
                    &hud,
                    &dxvk_huds(),
                    "dxvkHud",
                    cx,
                ),
            ));
            #[cfg(not(target_os = "linux"))]
            {
            let metal4 = overrides
                .and_then(|value| value.metal4_enabled)
                .or(defaults.metal4_enabled)
                .unwrap_or(false);
            children.push(self.program_engine_switch(
                "ps-metal4".into(),
                crate::i18n::s().program_settings.metal4_encoder,
                metal4,
                "metal4Enabled",
                cx,
            ));
            }
        } else {
            let backend = defaults.graphics_backend.as_deref().unwrap_or("recommended");
            let hud = defaults.dxvk_hud.as_deref().unwrap_or("off");
            children.push({
                let g = program_settings::graphics_label(backend);
                let h = program_settings::hud_label(hud);
                muted(crate::i18n::s().dialogs.inherited_hud(g, h))
                .into_any_element()
            });
        }
        self.section("override-graphics", crate::i18n::config::section_title("graphics"), children, cx)
    }

    #[cfg(not(target_os = "linux"))]
    fn program_upscale_section(
        &self,
        settings: &ProgramSettingsDto,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled = settings.upscale_enabled();
        let overrides = settings.overrides.as_ref();
        let defaults = &settings.defaults;
        let mut children = vec![self.program_switch(
            "ps-override-upscale".into(),
            crate::i18n::s().program_settings.override_upscale,
            enabled,
            false,
            |this, on, _cx| this.set_program_override_group("upscale", on),
            cx,
        )];
        if enabled {
            let upscaler = overrides
                .and_then(|value| value.gdi_upscaler.clone())
                .or_else(|| defaults.gdi_upscaler.clone())
                .unwrap_or_else(|| "off".into());
            children.push(self.program_choice_row(
                crate::i18n::s().program_settings.upscale_algo,
                self.program_choice(
                    "ps-gdi-upscaler".into(),
                    program_settings::upscaler_label(&upscaler).to_owned(),
                    &upscaler,
                    &gdi_upscalers(),
                    "gdiUpscaler",
                    cx,
                ),
            ));
            let cache = overrides
                .and_then(|value| value.gdi_upscaler_cache.clone())
                .or_else(|| defaults.gdi_upscaler_cache.clone())
                .unwrap_or_else(|| "memory".into());
            children.push(self.program_choice_row(
                crate::i18n::s().program_settings.upscale_cache,
                self.program_choice(
                    "ps-gdi-cache".into(),
                    program_settings::cache_label(&cache).to_owned(),
                    &cache,
                    &gdi_caches(),
                    "gdiUpscalerCache",
                    cx,
                ),
            ));
            children.push(muted(crate::i18n::s().program_settings.restart_game_to_apply).into_any_element());
        } else {
            let upscaler = defaults.gdi_upscaler.as_deref().unwrap_or("off");
            let cache = defaults.gdi_upscaler_cache.as_deref().unwrap_or("memory");
            children.push({
                let u = program_settings::upscaler_label(upscaler);
                let c = program_settings::cache_label(cache);
                muted(crate::i18n::s().dialogs.inherited(c, u))
                .into_any_element()
            });
        }
        let cache_dir = self.program_settings.gdi_cache_dir();
        let p = &crate::i18n::s().program_settings;
        let status = if self.program_settings.cache_clearing {
            p.clearing.to_owned()
        } else if self.program_settings.cache_pending {
            p.calculating.to_owned()
        } else {
            match self.program_settings.cache_bytes {
                Some(bytes) if bytes > 0 => program_settings::format_size(bytes),
                _ => p.no_cache.to_owned(),
            }
        };
        let clearable = self
            .program_settings
            .cache_bytes
            .is_some_and(|bytes| bytes > 0)
            && !self.program_settings.cache_pending
            && !self.program_settings.cache_clearing;
        children.push(self.info(crate::i18n::s().program_settings.disk_cache, status));
        if let Some(dir) = &cache_dir {
            children.push(
                muted(dir.to_string_lossy().into_owned())
                    .text_size(px(11.))
                    .truncate()
                    .into_any_element(),
            );
        }
        let legacy_cache_dir = self.program_settings.legacy_gdi_cache_dir().filter(|dir| dir.is_dir());
        if let Some(dir) = &legacy_cache_dir {
            children.push(muted(crate::i18n::s().dialogs.legacy_cache(dir.display())).text_size(px(11.)).truncate().into_any_element());
        }
        children.push(
            row()
                .gap(px(8.))
                .flex_wrap()
                .py(px(4.))
                .child(
                    Button::new("gdi-cache-clear")
                        .label(if self.program_settings.cache_clearing {
                            crate::i18n::s().program_settings.clearing
                        } else {
                            crate::i18n::s().program_settings.clear_disk_cache
                        })
                        .disabled(!clearable)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.clear_program_cache(cx)
                        })),
                )
                .child(
                    Button::new("gdi-cache-open")
                        .label(crate::i18n::s().program_settings.open_cache_dir)
                        .disabled(cache_dir.is_none())
                        .on_click(cx.listener(|this, _, _, _| {
                            this.open_program_cache_dir()
                        })),
                )
                .when(legacy_cache_dir.is_some(), |row| row.child(
                    Button::new("gdi-legacy-cache-open")
                        .label(crate::i18n::s().program_settings.open_legacy_cache_dir)
                        .on_click(cx.listener(|this, _, _, _| this.open_legacy_program_cache_dir()))
                ))
                .into_any_element(),
        );
        self.section("override-upscale", crate::i18n::config::section_title("upscaling"), children, cx)
    }

    #[cfg(target_os = "linux")]
    fn program_upscale_section(
        &self,
        settings: &ProgramSettingsDto,
        cx: &Context<Self>,
    ) -> AnyElement {
        let _ = settings;
        self.section("override-upscale", crate::i18n::config::section_title("upscaling"), vec![
            muted(crate::i18n::s().program_settings.gdi_mac_only).into_any_element(),
        ], cx)
    }

    fn program_sync_section(
        &self,
        settings: &ProgramSettingsDto,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled = settings.sync_enabled();
        let defaults = &settings.defaults;
        let mut children = vec![self.program_switch(
            "ps-override-sync".into(),
            crate::i18n::s().program_settings.override_sync,
            enabled,
            false,
            |this, on, _cx| this.set_program_override_group("sync", on),
            cx,
        )];
        if enabled {
            let sync = settings
                .overrides
                .as_ref()
                .and_then(|value| value.enhanced_sync.clone())
                .or_else(|| defaults.enhanced_sync.clone())
                .unwrap_or_else(|| "msync".into());
            children.push(self.program_choice_row(
                crate::i18n::s().program_settings.sync_mode,
                self.program_choice(
                    "ps-sync-mode".into(),
                    program_settings::sync_label(&sync).to_owned(),
                    &sync,
                    &sync_modes(),
                    "enhancedSync",
                    cx,
                ),
            ));
        } else {
            let sync = defaults.enhanced_sync.as_deref().unwrap_or("msync");
            children.push(
                muted(crate::i18n::s().dialogs.inherited_single(program_settings::sync_label(sync))).into_any_element(),
            );
        }
        self.section("override-sync", crate::i18n::s().program_settings.sync_section, children, cx)
    }

    fn program_performance_section(
        &self,
        settings: &ProgramSettingsDto,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled = settings.performance_enabled();
        let defaults = &settings.defaults;
        let mut children = vec![self.program_switch(
            "ps-override-performance".into(),
            crate::i18n::s().program_settings.override_perf,
            enabled,
            false,
            |this, on, _cx| this.set_program_override_group("performance", on),
            cx,
        )];
        if enabled {
            let shader_cache = settings
                .overrides
                .as_ref()
                .and_then(|value| value.shader_cache_enabled)
                .or(defaults.shader_cache_enabled)
                .unwrap_or(true);
            children.push(self.program_engine_switch(
                "ps-shader-cache".into(),
                crate::i18n::config::field_title("config.field.shaderCache", "shaderCacheEnabled", "Shader Cache"),
                shader_cache,
                "shaderCacheEnabled",
                cx,
            ));
        } else {
            children.push(
                {
                    let st = if defaults.shader_cache_enabled.unwrap_or(true) {
                        crate::i18n::s().common.short_on
                    } else {
                        crate::i18n::s().common.short_off
                    };
                    muted(crate::i18n::s().dialogs.inherited_shader_cache(st))
                }
                .into_any_element(),
            );
        }
        self.section("override-performance", crate::i18n::config::section_title("performance"), children, cx)
    }

    fn program_input_section(
        &self,
        settings: &ProgramSettingsDto,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled = settings.input_enabled();
        let defaults = &settings.defaults;
        let overrides = settings.overrides.as_ref();
        let disable_mouse_capture = overrides
            .and_then(|value| value.disable_mouse_capture)
            .or(defaults.disable_mouse_capture)
            .unwrap_or(false);
        let mouse_label = crate::i18n::config::field_title(
            "config.field.disableMouseCapture",
            "disableMouseCapture",
            "disableMouseCapture",
        );
        let mut children = vec![
            self.program_switch(
                "ps-disable-mouse-capture".into(),
                mouse_label,
                disable_mouse_capture,
                false,
                |this, on, cx| this.set_program_override_bool("disableMouseCapture", on, cx),
                cx,
            ),
            muted(crate::i18n::s().program_settings.disable_mouse_capture_desc)
                .text_size(px(11.))
                .line_height(relative(1.4))
                .into_any_element(),
            self.program_switch(
                "ps-override-input".into(),
                crate::i18n::s().program_settings.override_input,
                enabled,
                false,
                |this, on, _cx| this.set_program_override_group("input", on),
                cx,
            ),
        ];
        if enabled {
            for (id, key, field) in [
                ("ps-controller-compat", "config.field.controllerCompatibilityMode", "controllerCompatibilityMode"),
                ("ps-disable-hidapi", "config.field.disableHIDAPI", "disableHIDAPI"),
                ("ps-background-events", "config.field.allowBackgroundEvents", "allowBackgroundEvents"),
                ("ps-button-labels", "config.field.useButtonLabels", "useButtonLabels"),
            ] {
                let label = crate::i18n::config::field_title(key, field, field);
                let checked = match field {
                    "controllerCompatibilityMode" => overrides
                        .and_then(|value| value.controller_compatibility_mode)
                        .or(defaults.controller_compatibility_mode)
                        .unwrap_or(false),
                    "disableHIDAPI" => overrides
                        .and_then(|value| value.disable_hidapi)
                        .or(defaults.disable_hidapi)
                        .unwrap_or(false),
                    "allowBackgroundEvents" => overrides
                        .and_then(|value| value.allow_background_events)
                        .or(defaults.allow_background_events)
                        .unwrap_or(false),
                    _ => overrides
                        .and_then(|value| value.use_button_labels)
                        .or(defaults.use_button_labels)
                        .unwrap_or(false),
                };
                children.push(self.program_switch(
                    id.to_owned(),
                    label,
                    checked,
                    false,
                    move |this, on, cx| this.set_program_override_bool(field, on, cx),
                    cx,
                ));
            }
        } else {
            children.push({
                let c = &crate::i18n::s().common;
                let a = if defaults.controller_compatibility_mode.unwrap_or(false) { c.short_on } else { c.short_off };
                let b = if defaults.use_button_labels.unwrap_or(false) { c.short_on } else { c.short_off };
                muted(crate::i18n::s().dialogs.inherited_controller(a, b))
                .into_any_element()
            });
        }
        self.section("override-input", crate::i18n::config::section_title("input"), children, cx)
    }

    fn program_display_section(
        &mut self,
        settings: &ProgramSettingsDto,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let enabled = settings.display_enabled();
        let defaults = &settings.defaults;
        let overrides = settings.overrides.as_ref();
        let emulate_modeset = overrides
            .and_then(|value| value.emulate_modeset)
            .or(defaults.emulate_modeset)
            .unwrap_or(false);
        let mut children = vec![
            self.program_switch(
                "ps-emulate-modeset".into(),
                crate::i18n::s().dialogs.legacy_fs_label,
                emulate_modeset,
                false,
                |this, on, cx| this.set_program_override_bool("emulateModeset", on, cx),
                cx,
            ),
            muted(crate::i18n::s().program_settings.legacy_fs_desc)
                .text_size(px(11.))
                .line_height(relative(1.4))
                .into_any_element(),
            self.program_switch(
                "ps-override-display".into(),
                crate::i18n::s().program_settings.override_display,
                enabled,
                false,
                |this, on, _cx| this.set_program_override_group("display", on),
                cx,
            ),
        ];
        if enabled {
            let virtual_desktop = overrides
                .and_then(|value| value.virtual_desktop_enabled)
                .or(defaults.virtual_desktop_enabled)
                .unwrap_or(false);
            children.push(self.program_switch(
                "ps-virtual-desktop".into(),
                crate::i18n::s().program_settings.enable_virtual_desktop,
                virtual_desktop,
                false,
                |this, on, cx| this.set_program_override_bool("virtualDesktopEnabled", on, cx),
                cx,
            ));
            if virtual_desktop {
                let preset = overrides
                    .and_then(|value| value.resolution_preset.clone())
                    .or_else(|| defaults.resolution_preset.clone())
                    .unwrap_or_else(|| "r1920x1080".into());
                children.push(self.program_choice_row(
                    crate::i18n::config::field_title("config.field.desktopSize", "desktopSize", "Resolution"),
                    self.program_choice(
                        "ps-resolution".into(),
                        program_settings::resolution_label(&preset).to_owned(),
                        &preset,
                        &resolutions(),
                        "resolutionPreset",
                        cx,
                    ),
                ));
                if preset == "custom" {
                    let width = overrides
                        .and_then(|value| value.custom_resolution_width)
                        .or(defaults.custom_resolution_width)
                        .unwrap_or(1920);
                    let height = overrides
                        .and_then(|value| value.custom_resolution_height)
                        .or(defaults.custom_resolution_height)
                        .unwrap_or(1080);
                    let width_input = self.program_input(
                        "ps-res-width",
                        &width.to_string(),
                        "1920",
                        window,
                        cx,
                    );
                    let height_input = self.program_input(
                        "ps-res-height",
                        &height.to_string(),
                        "1080",
                        window,
                        cx,
                    );
                    children.push(
                        row()
                            .gap(px(10.))
                            .py(px(6.))
                            .child(div().w(px(110.)).child(Input::new(&width_input)))
                            .child(muted("×"))
                            .child(div().w(px(110.)).child(Input::new(&height_input)))
                            .into_any_element(),
                    );
                }
            }
        } else {
            children.push(
                {
                    let vd = if defaults.virtual_desktop_enabled.unwrap_or(false) {
                        crate::i18n::s().program_settings.virtual_desktop_on
                    } else {
                        crate::i18n::s().program_settings.virtual_desktop_off
                    };
                    let res = program_settings::resolution_label(
                        defaults.resolution_preset.as_deref().unwrap_or("r1920x1080"),
                    );
                    muted(crate::i18n::s().dialogs.inherited_display(vd, res))
                }
                .into_any_element(),
            );
        }
        self.section("override-display", crate::i18n::config::section_title("display"), children, cx)
    }

    fn program_dll_section(
        &mut self,
        settings: &ProgramSettingsDto,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let enabled = settings.dll_enabled();
        let mut children = vec![self.program_switch(
            "ps-override-dll".into(),
            crate::i18n::s().program_settings.override_dll,
            enabled,
            false,
            |this, on, _cx| this.set_program_override_group("dll", on),
            cx,
        )];
        if enabled {
            let entries = settings
                .overrides
                .as_ref()
                .and_then(|value| value.dll_overrides.clone())
                .unwrap_or_default();
            if entries.is_empty() {
                children.push(muted(crate::i18n::s().program_settings.no_program_dll).py(px(6.)).into_any_element());
            }
            for (index, entry) in entries.iter().enumerate() {
                let mode = program_settings::dll_mode_label(&entry.mode);
                children.push(
                    row()
                        .gap(px(12.))
                        .py(px(5.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .font_family("monospace")
                                .truncate()
                                .child(entry.dll_name.clone()),
                        )
                        .child(muted(mode.to_owned()))
                        .child(
                            Button::new(format!("ps-dll-remove-{index}"))
                                .label("−")
                                .ghost()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.remove_program_dll_override(index);
                                    cx.notify();
                                })),
                        )
                        .into_any_element(),
                );
            }
            let name = self.program_input("ps-dll-name", "", crate::i18n::s().program_settings.dll_example_placeholder, window, cx);
            let mode = self.program_settings.dll_new_mode.clone();
            let mode_control = self.program_choice(
                "ps-dll-new-mode".into(),
                program_settings::dll_mode_label(&mode).to_owned(),
                &mode,
                &dll_modes(),
                "dllNewMode",
                cx,
            );
            children.push(
                row()
                    .gap(px(10.))
                    .py(px(8.))
                    .child(div().flex_1().min_w_0().child(Input::new(&name)))
                    .child(mode_control)
                    .child(
                        Button::new("ps-dll-add")
                            .label(crate::i18n::s().common.add)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let name = this
                                    .preview
                                    .program_inputs
                                    .get("ps-dll-name")
                                    .map(|input| input.read(cx).value().to_string())
                                    .unwrap_or_default();
                                this.add_program_dll_override(name, cx);
                            })),
                    )
                    .into_any_element(),
            );
        } else {
            let count = settings
                .defaults
                .dll_overrides
                .as_ref()
                .map_or(0, Vec::len);
            children.push(
                muted(crate::i18n::s().dialogs.inherited_bottle_dll_overrides(count)).into_any_element(),
            );
        }
        self.section("override-dll", crate::i18n::s().dialogs.sheet_dll, children, cx)
    }

    fn program_history_section(&self, cx: &Context<Self>) -> AnyElement {
        let mut children: Vec<AnyElement> = Vec::new();
        if self.program_settings.history_loading {
            children.push(
                row()
                    .gap(px(8.))
                    .py(px(10.))
                    .child(Spinner::new().with_size(px(14.)))
                    .child(muted(crate::i18n::s().program_settings.reading_run_history))
                    .into_any_element(),
            );
        } else if let Some(error) = &self.program_settings.history_error {
            children.push(muted(error.clone()).py(px(8.)).into_any_element());
            children.push(
                Button::new("program-history-retry")
                    .label(crate::i18n::s().common.retry)
                    .on_click(cx.listener(|this, _, _, cx| this.reload_program_history(cx)))
                    .into_any_element(),
            );
        } else if self.program_settings.history.is_empty() {
            children.push(muted(crate::i18n::s().program_settings.no_run_history).py(px(8.)).into_any_element());
        } else {
            let container = self.program_settings.container.clone().unwrap_or_default();
            for entry in self
                .program_settings
                .history_entries_newest_first()
                .into_iter()
                .cloned()
                .collect::<Vec<RunEntryDto>>()
            {
                children.push(self.program_history_row(&container, &entry, cx));
            }
            children.push(
                row()
                    .py(px(6.))
                    .child(
                        Button::new("program-history-clear")
                            .label(crate::i18n::s().program_settings.clear_history)
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| this.clear_program_history(cx))),
                    )
                    .into_any_element(),
            );
        }
        self.section("program-history", crate::i18n::s().program_settings.run_history_section, children, cx)
    }

    fn program_history_row(
        &self,
        container: &str,
        entry: &RunEntryDto,
        cx: &Context<Self>,
    ) -> AnyElement {
        let log_path = entry.log_path.clone();
        let container = container.to_owned();
        let run_id = entry.id.clone();
        let exit = entry
            .exit_code
            .map(|code| crate::i18n::s().dialogs.exit(code))
            .unwrap_or_else(|| crate::i18n::s().runtime.running.to_owned());
        row()
            .gap(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(rgba(crate::theme::divider()))
            .child(
                col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(program_settings::relative_time(entry.start_time))
                    .child(
                        muted(format!(
                            "{} · {}",
                            program_settings::duration_label(entry.duration),
                            exit
                        ))
                        .truncate(),
                    ),
            )
            .when(!log_path.is_empty(), |d| {
                d.child(
                    Button::new(format!("program-log-{run_id}"))
                        .label(crate::i18n::s().program_settings.open_log)
                        .ghost()
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.bridge.open(&container, &log_path);
                        })),
                )
            })
            .child(
                Button::new(format!("program-run-delete-{run_id}"))
                    .label(crate::i18n::s().common.delete)
                    .ghost()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.delete_program_run(run_id.clone(), cx);
                    })),
            )
            .into_any_element()
    }

    fn program_shortcut_section(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let destination = self.program_settings.shortcut_destination.clone();
        let input = self.program_input("ps-shortcut-dest", &destination, if cfg!(target_os = "linux") { "~/.local/share/applications" } else { "~/Applications" }, window, cx);
        let mut children: Vec<AnyElement> = vec![
            col()
                .gap(px(7.))
                .py(px(8.))
                .child(crate::i18n::s().program_settings.shortcut_location)
                .child(Input::new(&input))
                .into_any_element(),
            muted(if cfg!(target_os = "linux") { crate::i18n::s().program_settings.shortcut_desc_linux } else if cfg!(target_os = "windows") { crate::i18n::s().program_settings.shortcut_desc_windows } else { crate::i18n::s().program_settings.shortcut_desc_mac })
                .into_any_element(),
        ];
        if let Some(path) = &self.program_settings.shortcut_path {
            children.push(muted(crate::i18n::s().dialogs.created(path)).into_any_element());
        }
        if let Some(error) = &self.program_settings.shortcut_error {
            children.push(muted(error.clone()).into_any_element());
        }
        children.push(
            row()
                .py(px(6.))
                .child(
                    Button::new("ps-shortcut-create")
                        .label(crate::i18n::s().program_settings.create_shortcut)
                        .on_click(cx.listener(|this, _, _, cx| this.create_program_shortcut(cx))),
                )
                .into_any_element(),
        );
        self.section("program-shortcut", crate::i18n::s().program_settings.shortcut_section, children, cx)
    }

    /// Scan folders for one bottle. Roots, assignment, exclusion and the
    /// executable are all `folders.*` calls; the scan itself is a bridge job.
    fn scan_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.demo {
            return self.demo_scan_settings(cx);
        }
        let Some(container) = self.scan_container() else {
            return self.form(
                "scan-settings-scroll",
                vec![self.section(
                    "scan-folders",
                    crate::i18n::s().dialogs.scan_folders,
                    vec![muted(crate::i18n::s().bottle.select_bottle_first).into_any_element()],
                    cx,
                )],
            );
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        match self.folders.get(&container).cloned() {
            Some(folders) => {
                rows.push(
                    row()
                        .py(px(10.))
                        .gap(px(16.))
                        .child(div().flex_1().min_w_0().child(crate::i18n::s().dialogs.fetch_metadata_automatically))
                        .child(
                            Switch::new("scan-automatic-metadata")
                                .checked(folders.automatic_metadata)
                                .on_change(cx.listener(|this, checked, _, cx| {
                                    this.edit_folder(
                                        FolderEdit::AutomaticMetadata(*checked),
                                        cx,
                                    );
                                })),
                        )
                        .into_any_element(),
                );
                rows.push(
                    row()
                        .py(px(10.))
                        .gap(px(16.))
                        .child(
                            col()
                                .flex_1()
                                .min_w_0()
                                .gap(px(3.))
                                .child(crate::i18n::s().dialogs.include_pinned_programs_steam_games)
                                .child(muted(crate::i18n::s().dialogs.scan_folders_only_tip).text_size(px(11.))),
                        )
                        .child(
                            Switch::new("scan-include-pinned-steam")
                                .checked(folders.include_pinned_and_steam)
                                .on_change(cx.listener(|this, checked, _, cx| {
                                    this.edit_folder(FolderEdit::Visibility(*checked), cx);
                                })),
                        )
                        .into_any_element(),
                );
                if folders.roots.is_empty() && folders.assigned_folders.is_empty() {
                    rows.push(muted(crate::i18n::s().dialogs.no_scan_folders_added_yet).into_any_element());
                }
                for root in folders.roots.clone() {
                    rows.push(self.scan_root_row(&container, &root, cx));
                }
                rows.push(
                    row()
                        .gap(px(10.))
                        .py(px(10.))
                        .child(
                            Button::new("scan-add-picker")
                                .label(crate::i18n::s().dialogs.choose_folder)
                                .primary()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.choose_scan_roots(cx);
                                })),
                        )
                        .when(cfg!(not(target_os = "windows")), |d| {
                            d.child(muted(crate::i18n::s().dialogs.multi_select_bottle_tip).text_size(px(11.)))
                        })
                        .into_any_element(),
                );
                rows.push(
                    row()
                        .gap(px(10.))
                        .py(px(10.))
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(&self.preview.inputs["scan-path"])),
                        )
                        .child(
                            Button::new("scan-add")
                                .label(crate::i18n::s().dialogs.add_path)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let path =
                                        this.preview.inputs["scan-path"].read(cx).value().to_string();
                                    this.add_scan_root(&path, cx);
                                })),
                        )
                        .into_any_element(),
                );
                for folder in folders.assigned_folders.clone() {
                    rows.push(self.scan_game_row(&container, &folders, &folder, window, cx));
                }
                rows.push(
                    row()
                        .gap(px(10.))
                        .py(px(10.))
                        .child(
                            Button::new("scan-assign-picker")
                                .label(crate::i18n::s().dialogs.add_single_game_folder)
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.choose_assign_folders(cx);
                                })),
                        )
                        .child(
                            muted(crate::i18n::s().dialogs.folder_as_game_tip)
                                .text_size(px(11.)),
                        )
                        .into_any_element(),
                );
            }
            None if self.folders_pending.contains(&container) => rows.push(
                row()
                    .gap(px(8.))
                    .py(px(10.))
                    .child(Spinner::new().with_size(px(14.)))
                    .child(muted(crate::i18n::s().dialogs.loading_scan_folders))
                    .into_any_element(),
            ),
            None => {
                let reason = self
                    .folders_error
                    .get(&container)
                    .cloned()
                    .unwrap_or_else(|| crate::i18n::s().dialogs.scan_folders_have_not_been_loaded_yet.to_owned());
                rows.push(muted(reason).into_any_element());
                rows.push(
                    row()
                        .py(px(8.))
                        .child(
                            Button::new("scan-folders-retry")
                                .label(crate::i18n::s().common.retry)
                                .on_click(cx.listener(|this, _, _, cx| this.reload_folders(cx))),
                        )
                        .into_any_element(),
                );
            }
        }
        rows.extend(self.scan_controls(&container, cx));
        self.form(
            "scan-settings-scroll",
            vec![self.section("scan-folders", crate::i18n::s().dialogs.scan_folders, rows, cx)],
        )
    }

    /// A scan-root row. Roots are containers of games — their child folders
    /// become entries, the root itself never does — so the row only offers
    /// root-level actions. Game-level controls live on `scan_game_row`.
    fn scan_root_row(
        &mut self,
        container: &str,
        root: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let remove_root = root.to_owned();
        let other_bottles: Vec<(usize, String)> = self
            .bottles
            .iter()
            .enumerate()
            .filter(|(_, bottle)| bottle.available && bottle.path != container)
            .map(|(index, bottle)| (index, bottle.name.clone()))
            .collect();
        let move_button = if other_bottles.is_empty() {
            None
        } else {
            let view = cx.entity().downgrade();
            let menu_root = root.to_owned();
            Some(
                Button::new(format!("scan-move:{root}"))
                    .label(crate::i18n::s().dialogs.move_entry)
                    .ghost()
                    .dropdown_menu(move |mut menu, _, _| {
                        menu = menu.scrollable(true);
                        for (index, name) in other_bottles.clone() {
                            let view = view.clone();
                            let menu_root = menu_root.clone();
                            menu = menu.item(PopupMenuItem::new(name).on_click(
                                move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.move_scan_root(&menu_root, index, cx);
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            )
        };
        col()
            .gap(px(8.))
            .py(px(10.))
            .child(
                row()
                    .gap(px(10.))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(div().truncate().child(root.to_owned()))
                            .child(muted(crate::i18n::s().dialogs.scan_folder).truncate()),
                    )
                    .when_some(move_button, |d, button| d.child(button))
                    .child(
                        Button::new(format!("scan-remove:{root}"))
                            .label("−")
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.edit_folder(
                                    FolderEdit::RemoveRoot(remove_root.clone()),
                                    cx,
                                );
                            })),
                    ),
            )
            .when(cfg!(not(target_os = "windows")), |d| {
                d.child(muted(crate::i18n::s().library.container_prefix(&container)).text_size(px(10.)))
            })
            .into_any_element()
    }

    /// A standalone game folder: the folder itself is the game. It can be
    /// unassigned (游戏库 off), excluded, or given its launching executable.
    fn scan_game_row(
        &mut self,
        container: &str,
        folders: &FoldersDto,
        folder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let excluded = folders.is_excluded(folder);
        let executable = folders.executable_for(folder).unwrap_or_default().to_owned();
        let display = if executable.is_empty() {
            crate::i18n::s().dialogs.no_executable_set.to_owned()
        } else {
            executable.clone()
        };
        let input = self.folder_input(folder, &executable, window, cx);
        let assign_folder = folder.to_owned();
        let exclude_folder = folder.to_owned();
        let remove_folder = folder.to_owned();
        let executable_folder = folder.to_owned();
        let executable_key = format!("scan-exe:{folder}");
        col()
            .gap(px(8.))
            .py(px(10.))
            .child(
                row()
                    .gap(px(10.))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(div().truncate().child(folder.to_owned()))
                            .child(muted(display).truncate()),
                    )
                    .child(
                        Switch::new(format!("scan-assign:{folder}"))
                            .label(crate::i18n::s().library.title)
                            .checked(true)
                            .on_change(cx.listener(move |this, checked, _, cx| {
                                this.edit_folder(
                                    FolderEdit::Assign {
                                        path: assign_folder.clone(),
                                        assigned: *checked,
                                    },
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Switch::new(format!("scan-exclude:{folder}"))
                            .label(crate::i18n::s().dialogs.exclude)
                            .checked(excluded)
                            .on_change(cx.listener(move |this, checked, _, cx| {
                                this.edit_folder(
                                    FolderEdit::Exclude {
                                        path: exclude_folder.clone(),
                                        excluded: *checked,
                                    },
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new(format!("scan-remove:{folder}"))
                            .label("−")
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.edit_folder(
                                    FolderEdit::Assign {
                                        path: remove_folder.clone(),
                                        assigned: false,
                                    },
                                    cx,
                                );
                            })),
                    ),
            )
            .child(
                row()
                    .gap(px(10.))
                    .child(div().flex_1().child(Input::new(&input)))
                    .child(
                        Button::new(format!("scan-exe:{folder}"))
                            .label(crate::i18n::s().dialogs.set_executable)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let text = this
                                    .preview
                                    .folder_inputs
                                    .get(&executable_key)
                                    .map(|input| input.read(cx).value().to_string())
                                    .unwrap_or_default();
                                let executable = (!text.trim().is_empty())
                                    .then(|| text.trim().to_owned());
                                this.edit_folder(
                                    FolderEdit::Executable {
                                        path: executable_folder.clone(),
                                        executable,
                                    },
                                    cx,
                                );
                            })),
                    ),
            )
            .when(cfg!(not(target_os = "windows")), |d| {
                d.child(muted(crate::i18n::s().library.container_prefix(&container)).text_size(px(10.)))
            })
            .into_any_element()
    }

    fn folder_input(
        &mut self,
        root: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let key = format!("scan-exe:{root}");
        if let Some(input) = self.preview.folder_inputs.get(&key) {
            return input.clone();
        }
        let entity = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::i18n::s().dialogs.path_to_game_exe)
                .default_value(value.to_owned())
        });
        self.preview.folder_inputs.insert(key, entity.clone());
        entity
    }

    /// Start / progress / cancel, using the sheet's existing spinner treatment.
    fn scan_controls(&self, container: &str, cx: &Context<Self>) -> Vec<AnyElement> {
        let scan = self.scan_for(container);
        let elsewhere = self
            .scan
            .as_ref()
            .is_some_and(|scan| scan.container != container && scan.running());
        let mut rows: Vec<AnyElement> = Vec::new();
        if let Some(state) = scan {
            let status = state.status.unwrap_or(JobStatus::Queued);
            let label = scan::progress_label(status, state.stage.as_deref());
            let counts = state
                .total
                .map(|total| format!("{} / {total}", state.loaded))
                .unwrap_or_default();
            let mut line = row()
                .gap(px(8.))
                .py(px(10.))
                .child(if state.running() {
                    Spinner::new().with_size(px(14.)).into_any_element()
                } else {
                    div().into_any_element()
                })
                .child(div().child(label));
            if !counts.is_empty() {
                line = line.child(muted(counts));
            }
            rows.push(line.into_any_element());
            if let Some(error) = &state.error {
                rows.push(muted(error.clone()).into_any_element());
            }
        }
        if scan.is_some_and(ScanState::running) {
            rows.push(
                row()
                    .py(px(6.))
                    .child(
                        Button::new("scan-cancel")
                            .label(crate::i18n::s().dialogs.cancel_scan)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_scan(cx))),
                    )
                    .into_any_element(),
            );
        } else {
            rows.push(
                row()
                    .gap(px(10.))
                    .py(px(6.))
                    .child(
                        Button::new("scan-start")
                            .label(if scan.is_some() { crate::i18n::s().dialogs.scan_again } else { crate::i18n::s().dialogs.scan_now })
                            .primary()
                            .disabled(elsewhere)
                            .on_click(cx.listener(|this, _, _, cx| this.start_scan(cx))),
                    )
                    .when(elsewhere, |d| d.child(muted(crate::i18n::s().state.other_bottle_scan_running)))
                    .into_any_element(),
            );
        }
        rows
    }

    fn demo_scan_settings(&self, cx: &Context<Self>) -> AnyElement {
        self.form(
            "scan-settings-scroll",
            vec![self.section(
                "scan-folders",
                crate::i18n::s().dialogs.scan_folders,
                vec![
                    self.info("~/Games", crate::i18n::s().dialogs.recursive),
                    self.field("scan-path", crate::i18n::s().dialogs.add_folder),
                    self.action("scan-add", crate::i18n::s().dialogs.add_folder, Command::Notice, cx),
                ],
                cx,
            )],
        )
    }

    /// Every `.exe` the helper found under the folder, newest section first.
    /// Choosing one records it; Play opened the picker, so it also starts.
    fn executable_picker(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(picker) = self.executable_picker.as_ref() else {
            return self.form(
                "executable-scroll",
                vec![self.section(
                    "executable-empty",
                    crate::i18n::s().dialogs.launch_program,
                    vec![self.info(
                        crate::i18n::s().dialogs.no_launchable_executable_found,
                        crate::i18n::s().dialogs.no_exe_in_this_folder,
                    )],
                    cx,
                )],
            );
        };
        let folder = picker.folder.clone();
        let multi_select = picker.multi_select;
        let selected = picker.selected.clone();
        let game_index = picker.game_index;
        let supports_multi = self
            .game(game_index)
            .is_some_and(crate::model::Game::supports_multi_executable);
        let picker_container_handle = window
            .use_keyed_state(
                ("exe-picker-container", self.executable_picker_generation),
                cx,
                |_, cx| cx.focus_handle(),
            )
            .read(cx)
            .clone();
        let candidate_handles: Vec<FocusHandle> = (0..picker.candidates.len())
            .map(|i| {
                window
                    .use_keyed_state(("exe-candidate", i), cx, |_, cx| {
                        cx.focus_handle().tab_stop(true).tab_index(0)
                    })
                    .read(cx)
                    .clone()
            })
            .collect();
        if picker.loading || picker.candidates.is_empty() {
            let loading_focused = window.use_keyed_state(
                ("exe-picker-loading-autofocus", self.executable_picker_generation),
                cx,
                |_, _| std::cell::Cell::new(false),
            );
            if !loading_focused.read(cx).replace(true) {
                window.focus(&picker_container_handle, cx);
            }
        } else {
            let candidates_focused = window.use_keyed_state(
                (
                    "exe-picker-candidates-autofocus",
                    self.executable_picker_generation,
                ),
                cx,
                |_, _| std::cell::Cell::new(false),
            );
            if !candidates_focused.read(cx).replace(true) {
                if let Some(first) = candidate_handles.first() {
                    window.focus(first, cx);
                }
            }
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        if picker.loading {
            rows.push(
                row()
                    .gap(px(10.))
                    .py(px(14.))
                    .justify_center()
                    .child(Spinner::new().with_size(px(14.)))
                    .child(muted(crate::i18n::s().dialogs.scanning_executables_in_this_folder))
                    .into_any_element(),
            );
        } else if let Some(reason) = picker.error.as_deref() {
            rows.push(self.info(crate::i18n::s().dialogs.scan_failed, reason));
        } else if picker.candidates.is_empty() {
            rows.push(self.info(crate::i18n::s().dialogs.no_launchable_executable_found, crate::i18n::s().dialogs.no_exe_in_this_folder));
        }
        for (i, candidate) in picker.candidates.iter().enumerate() {
            let path = candidate.path.clone();
            let (location, is_root) =
                crate::bridge::dto::relative_directory_label(&candidate.path, &folder);
            let is_checked = multi_select && selected.iter().any(|item| item == &path);
            let row_id = SharedString::from(format!("exe-candidate-{}", i));
            rows.push(
                row()
                    .id(row_id)
                    .track_focus(&candidate_handles[i])
                    .w_full()
                    .p(px(10.))
                    .my(px(3.))
                    .gap(px(12.))
                    .rounded(px(8.))
                    .bg(rgba(if is_checked {
                        crate::theme::accent_subtle_bg()
                    } else {
                        crate::theme::card_bg_subtle()
                    }))
                    .border_1()
                    .border_color(rgba(if is_checked {
                        crate::theme::accent_subtle_border()
                    } else {
                        crate::theme::card_border()
                    }))
                    .focus(|s| s.border_2().border_color(rgb(crate::theme::ACCENT)))
                    .cursor_pointer()
                    .hover(|d| {
                        d.bg(rgba(if is_checked {
                            crate::theme::accent_subtle_hover()
                        } else {
                            crate::theme::hover()
                        }))
                        .border_color(rgba(if is_checked {
                            crate::theme::accent_subtle_border()
                        } else {
                            crate::theme::hover_strong()
                        }))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if multi_select {
                            this.toggle_picker_executable(&path, cx);
                        } else {
                            this.choose_executable(&path, cx);
                            this.restore_preview_sheet_focus(window, cx);
                        }
                    }))
                    .when(multi_select, |d| {
                        d.child(
                            div()
                                .size(px(18.))
                                .rounded(px(5.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .border_1()
                                .border_color(rgba(if is_checked {
                                    crate::theme::accent_subtle_border()
                                } else {
                                    crate::theme::surface_border()
                                }))
                                .bg(rgba(if is_checked {
                                    crate::theme::accent_subtle_hover()
                                } else {
                                    crate::theme::card_bg()
                                }))
                                .when(is_checked, |box_el| {
                                    box_el.child(
                                        icon("check", 12.)
                                            .text_color(rgb(crate::theme::accent_fg())),
                                    )
                                }),
                        )
                    })
                    .child(
                        div()
                            .size(px(36.))
                            .rounded(px(7.))
                            .bg(gradient(crate::theme::placeholder_gradient().0, crate::theme::placeholder_gradient().1))
                            .border_1()
                            .border_color(rgba(crate::theme::card_border()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("package", 16.).text_color(rgb(if is_root || is_checked { crate::theme::accent_fg() } else { crate::theme::muted_color() }))),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                row()
                                    .gap(px(6.))
                                    .items_center()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_size(px(13.))
                                            .truncate()
                                            .child(candidate.name.clone()),
                                    )
                                    .when(is_root, |d| {
                                        d.child(
                                            div()
                                                .px(px(6.))
                                                .py(px(1.))
                                                .rounded(px(4.))
                                                .bg(rgba(crate::theme::accent_subtle_bg()))
                                                .text_size(px(10.))
                                                .text_color(rgb(crate::theme::accent_fg()))
                                                .child(crate::i18n::s().dialogs.primary),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(crate::theme::muted_color()))
                                    .truncate()
                                    .child(location),
                            ),
                    )
                    .child(if multi_select {
                        row()
                            .gap(px(4.))
                            .items_center()
                            .child(muted(if is_checked { crate::i18n::s().dialogs.selected } else { crate::i18n::s().dialogs.click_to_select }))
                            .into_any_element()
                    } else {
                        row()
                            .gap(px(4.))
                            .items_center()
                            .child(muted(crate::i18n::s().dialogs.select))
                            .child(muted("›"))
                            .into_any_element()
                    })
                    .into_any_element(),
            );
        }
        let mut info_rows = vec![self.info(&picker.title, &folder)];
        if supports_multi {
            info_rows.push(
                row()
                    .py(px(8.))
                    .gap(px(14.))
                    .items_center()
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(div().text_size(px(13.)).child(crate::i18n::s().dialogs.multi_exe_entry_mode))
                            .child(
                                muted(if multi_select {
                                    {
                                        let n = selected.len();
                                        crate::i18n::s().dialogs.entries_chooser_tip(n)
                                    }
                                } else {
                                    crate::i18n::s().dialogs.multi_exe_tip.to_owned()
                                })
                                .text_size(px(11.)),
                            ),
                    )
                    .child(
                        Switch::new("picker-multi-exe")
                            .checked(multi_select)
                            .on_change(cx.listener(move |this, checked, _, cx| {
                                this.set_multi_executable(game_index, *checked, cx);
                            })),
                    )
                    .into_any_element(),
            );
        }
        div()
            .id("executable-picker-root")
            .size_full()
            .track_focus(&picker_container_handle)
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let mods = &event.keystroke.modifiers;
                if mods.platform || mods.control {
                    if event.keystroke.key == "enter"
                        && this
                            .executable_picker
                            .as_ref()
                            .is_some_and(|p| p.multi_select && !p.loading && !p.selected.is_empty())
                    {
                        this.confirm_multi_executable_picker(cx);
                        if this.preview.sheet.is_none() && this.launch_chooser.is_none() {
                            this.restore_preview_sheet_focus(window, cx);
                        }
                        cx.stop_propagation();
                    }
                    return;
                }
                if mods.modified() || candidate_handles.is_empty() {
                    return;
                }
                let n = candidate_handles.len();
                let current = candidate_handles.iter().position(|h| h.is_focused(window));
                match event.keystroke.key.as_str() {
                    "down" => {
                        let next = current.map_or(0, |i| (i + 1) % n);
                        candidate_handles[next].focus(window, cx);
                        cx.stop_propagation();
                    }
                    "up" => {
                        let prev = current.map_or(n - 1, |i| (i + n - 1) % n);
                        candidate_handles[prev].focus(window, cx);
                        cx.stop_propagation();
                    }
                    "home" => {
                        candidate_handles[0].focus(window, cx);
                        cx.stop_propagation();
                    }
                    "end" => {
                        candidate_handles[n - 1].focus(window, cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(self.form(
                "executable-scroll",
                vec![
                    self.section("executable-folder", crate::i18n::s().dialogs.game_folder, info_rows, cx),
                    self.section(
                        "executable-list",
                        if multi_select {
                            crate::i18n::s().dialogs.select_executables_multi_select
                        } else {
                            crate::i18n::s().dialogs.select_executable_exe
                        },
                        rows,
                        cx,
                    ),
                ],
            ))
            .into_any_element()
    }

    /// The live Winetricks sheet: catalog search, installed state and one
    /// helper-driven install job. `WHISKY_GPUI_DEMO_DATA=1` keeps the original
    /// static form so the visual baseline is unchanged.
    fn winetricks_sheet(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.demo {
            return self.demo_winetricks(cx);
        }
        let Some(container) = self.selected_container() else {
            return self.form(
                "winetricks-scroll",
                vec![self.section(
                    "tricks",
                    crate::i18n::s().dialogs.available_components,
                    vec![muted(crate::i18n::s().bottle.select_bottle_first).into_any_element()],
                    cx,
                )],
            );
        };

        let mut sections = Vec::new();
        let installed = self.winetricks.installed_for(&container);
        let mut filter_rows: Vec<AnyElement> = vec![
            col()
                .gap(px(7.))
                .py(px(8.))
                .child(crate::i18n::s().dialogs.search_components)
                .child(
                    Input::new(&self.preview.inputs["dependency-search"])
                        .prefix(icon("search", 13.).text_color(rgb(crate::theme::muted_color())))
                        .cleanable(true),
                )
                .into_any_element(),
        ];
        if let Some(catalog) = &self.winetricks.catalog {
            let mut tabs = row().flex_wrap().gap(px(6.)).py(px(6.));
            for category in &catalog.categories {
                let id = category.id.clone();
                let active = self.winetricks.category.as_deref() == Some(id.as_str());
                let view = cx.entity().downgrade();
                let next = id.clone();
                let button = if active {
                    Button::new(format!("trick-category:{id}"))
                        .label(category_label(&id).to_owned())
                        .primary()
                } else {
                    Button::new(format!("trick-category:{id}"))
                        .label(category_label(&id).to_owned())
                };
                tabs = tabs.child(button.on_click(move |_, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.winetricks.category = Some(next.clone());
                        this.winetricks.selected = None;
                        cx.notify();
                    });
                }));
            }
            filter_rows.push(tabs.into_any_element());

            let mut filter_line = row().gap(px(14.)).py(px(6.));
            filter_line = filter_line.child(
                Switch::new("trick-installed-only")
                    .label(crate::i18n::s().dialogs.installed_only)
                    .checked(self.flag("trick-installed-only"))
                    .on_change(cx.listener(|this, checked, _, cx| {
                        this.preview
                            .toggles
                            .insert("trick-installed-only", *checked);
                        cx.notify();
                    })),
            );
            if let Some(installed) = installed {
                filter_line = filter_line.child(muted(crate::i18n::s().winetricks.winetricks_count_summary(installed.len(), catalog.verb_count())));
            } else if self.winetricks.installed_pending.contains(&container) {
                filter_line = filter_line
                    .child(Spinner::new().with_size(px(12.)))
                    .child(muted(crate::i18n::s().dialogs.loading_installed_components));
            } else if let Some(error) = self.winetricks.installed_error.get(&container) {
                filter_line = filter_line.child(muted(error.clone())).child(
                    Button::new("trick-installed-error-retry")
                        .label(crate::i18n::s().common.retry)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.reload_installed_winetricks(cx)),
                        ),
                );
            }
            filter_rows.push(filter_line.into_any_element());
        }
        sections.push(self.section("trick-filter", crate::i18n::s().dialogs.search_filter, filter_rows, cx));

        match &self.winetricks.catalog {
            Some(catalog) => {
                let installed_only = self.flag("trick-installed-only");
                let installed_missing = installed_only && installed.is_none();
                let query = self.preview.inputs["dependency-search"]
                    .read(cx)
                    .value()
                    .to_string();
                let category = self
                    .winetricks
                    .category
                    .as_ref()
                    .and_then(|id| {
                        catalog
                            .categories
                            .iter()
                            .find(|category| &category.id == id)
                    })
                    .or_else(|| catalog.categories.first());

                if let Some(job) = self
                    .winetricks
                    .job
                    .as_ref()
                    .filter(|job| job.container == container)
                {
                    sections.push(self.section(
                        "trick-job",
                        crate::i18n::s().dialogs.install_progress,
                        self.winetricks_job_rows(job, cx),
                        cx,
                    ));
                }

                let mut rows: Vec<AnyElement> = Vec::new();
                if installed_missing && self.winetricks.installed_pending.contains(&container) {
                    rows.push(
                        row()
                            .gap(px(8.))
                            .py(px(12.))
                            .child(Spinner::new().with_size(px(14.)))
                            .child(muted(crate::i18n::s().dialogs.loading_installed_components))
                            .into_any_element(),
                    );
                } else if installed_missing {
                    let reason = self
                        .winetricks
                        .installed_error
                        .get(&container)
                        .cloned()
                        .unwrap_or_else(|| crate::i18n::s().dialogs.components_not_loaded.to_owned());
                    rows.push(muted(reason).py(px(10.)).into_any_element());
                    rows.push(
                        Button::new("trick-installed-retry")
                            .label(crate::i18n::s().common.retry)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.reload_installed_winetricks(cx)),
                            )
                            .into_any_element(),
                    );
                } else if let Some(category) = category {
                    let verbs = filtered_verbs(category, &query, installed_only, installed);
                    if verbs.is_empty() {
                        rows.push(
                            muted(if installed_only {
                                crate::i18n::s().dialogs.no_installed_components_in_this_category
                            } else {
                                crate::i18n::s().dialogs.no_matching_components
                            })
                            .py(px(10.))
                            .into_any_element(),
                        );
                    }
                    for verb in verbs {
                        rows.push(self.winetricks_verb_row(
                            verb,
                            installed.is_some_and(|installed| installed.contains(&verb.name)),
                            cx,
                        ));
                    }
                } else {
                    rows.push(
                        muted(crate::i18n::s().dialogs.no_component_catalog)
                            .py(px(10.))
                            .into_any_element(),
                    );
                }
                sections.push(self.section("tricks", crate::i18n::s().dialogs.available_components, rows, cx));
            }
            None if self.winetricks.catalog_pending => {
                sections.push(self.section(
                    "trick-loading",
                    crate::i18n::s().dialogs.available_components,
                    vec![
                        row()
                            .gap(px(8.))
                            .py(px(12.))
                            .child(Spinner::new().with_size(px(14.)))
                            .child(muted(crate::i18n::s().dialogs.loading_component_catalog))
                            .into_any_element(),
                    ],
                    cx,
                ));
            }
            None => {
                let reason = self
                    .winetricks
                    .catalog_error
                    .clone()
                    .unwrap_or_else(|| crate::i18n::s().dialogs.catalog_not_loaded.to_owned());
                sections.push(self.section(
                    "trick-load-error",
                    crate::i18n::s().dialogs.available_components,
                    vec![
                        muted(reason).py(px(10.)).into_any_element(),
                        Button::new("trick-catalog-retry")
                            .label(crate::i18n::s().common.retry)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.reload_winetricks_catalog(cx)),
                            )
                            .into_any_element(),
                    ],
                    cx,
                ));
            }
        }
        self.form("winetricks-scroll", sections)
    }

    fn winetricks_job_rows(
        &self,
        job: &crate::winetricks::InstallJob,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut line = row().gap(px(8.)).py(px(8.)).child(div().child(job.label()));
        if job.running() {
            line = line.child(Spinner::new().with_size(px(14.)));
        }
        if let Some(count) = job.progress.count_label() {
            line = line.child(muted(count));
        }
        rows.push(line.into_any_element());
        if let Some(output) = &job.progress.line {
            rows.push(
                div()
                    .w_full()
                    .truncate()
                    .font_family("monospace")
                    .text_size(px(11.))
                    .text_color(rgb(crate::theme::muted_color()))
                    .py(px(4.))
                    .child(output.clone())
                    .into_any_element(),
            );
        }
        for failure in &job.failed {
            rows.push(muted(format!("{} · {}", crate::i18n::s().winetricks.install_failed, failure.label())).into_any_element());
        }
        if let Some(error) = &job.error {
            rows.push(muted(error.clone()).into_any_element());
        }
        if job.running() {
            rows.push(
                Button::new("trick-cancel")
                    .label(crate::i18n::s().dialogs.cancel_install)
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_winetricks_install(cx)))
                    .into_any_element(),
            );
        }
        rows
    }

    fn winetricks_verb_row(
        &self,
        verb: &crate::winetricks::VerbDto,
        installed: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let selected = self.winetricks.selected.as_deref() == Some(verb.name.as_str());
        let view = cx.entity().downgrade();
        let name = verb.name.clone();
        row()
            .id(SharedString::from(format!("trick-verb:{}", verb.name)))
            .w_full()
            .p(px(9.))
            .my(px(2.))
            .gap(px(11.))
            .rounded(px(8.))
            .bg(if selected {
                rgba(crate::theme::accent_subtle_bg())
            } else {
                rgba(crate::theme::card_bg_subtle())
            })
            .border_1()
            .border_color(if selected {
                rgba(crate::theme::accent_subtle_border())
            } else {
                rgba(crate::theme::card_border())
            })
            .cursor_pointer()
            .hover(|d| d.bg(rgba(crate::theme::hover())).border_color(rgba(crate::theme::hover_strong())))
            .on_click(move |_, _, cx| {
                let name = name.clone();
                let _ = view.update(cx, |this, cx| {
                    this.winetricks.selected = Some(name);
                    cx.notify();
                });
            })
            .child(
                div()
                    .w(px(16.))
                    .flex()
                    .justify_center()
                    .text_size(px(13.))
                    .text_color(rgb(if installed { crate::theme::installed_fg() } else { crate::theme::muted_color() }))
                    .child(if installed { "✓" } else { "○" }),
            )
            .child(
                col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_size(px(12.5))
                            .truncate()
                            .child(verb.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(crate::theme::muted_color()))
                            .truncate()
                            .child(verb.description.clone()),
                    ),
            )
            .when(installed, |d| {
                d.child(
                    div()
                        .px(px(7.))
                        .py(px(2.))
                        .rounded(px(4.))
                        .bg(rgba(crate::theme::installed_bg()))
                        .text_size(px(10.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(crate::theme::installed_fg()))
                        .child(crate::i18n::s().common.installed),
                )
            })
            .into_any_element()
    }

    fn demo_winetricks(&self, cx: &Context<Self>) -> AnyElement {
        self.form(
            "winetricks-scroll",
            vec![
                self.field("dependency-search", crate::i18n::s().dialogs.search_components),
                self.section(
                    "tricks",
                    crate::i18n::s().dialogs.available_components,
                    [
                        ("vcrun2022", "Visual C++ 2015–2022"),
                        ("d3dx9", "DirectX 9"),
                        ("lavfilters", "LAV Filters"),
                        ("cjkfonts", crate::i18n::s().dialogs.cjk_fonts),
                        ("dotnet48", ".NET Framework 4.8"),
                    ]
                    .into_iter()
                    .map(|(key, label)| self.toggle(key, label, "", cx))
                    .collect(),
                    cx,
                ),
            ],
        )
    }


    fn move_entry_sheet(&self, cx: &Context<Self>) -> AnyElement {
        let Some(state) = self.move_entry.as_ref() else {
            return self.form(
                "move-entry-scroll",
                vec![self.section(
                    "move-entry",
                    crate::i18n::s().dialogs.move_target_title,
                    vec![muted(crate::i18n::s().dialogs.move_no_entry).into_any_element()],
                    cx,
                )],
            );
        };
        let mut rows: Vec<AnyElement> = vec![self.info(crate::i18n::s().dialogs.entry_label, state.title.clone())];
        if state.pending {
            rows.push(
                row()
                    .gap(px(8.))
                    .py(px(8.))
                    .child(Spinner::new().with_size(px(14.)))
                    .child(muted(crate::i18n::s().dialogs.moving_note))
                    .into_any_element(),
            );
        }
        if let Some(error) = &state.error {
            rows.push(muted(error.clone()).py(px(8.)).into_any_element());
        }
        let mut targets = 0;
        for bottle in self
            .bottles
            .iter()
            .filter(|bottle| bottle.path != state.source_container)
        {
            targets += 1;
            let path = bottle.path.clone();
            let selected = state.target_container.as_deref() == Some(path.as_str());
            let button = if selected {
                Button::new(format!("move-target:{path}"))
                    .label(if state.pending { crate::i18n::s().dialogs.moving } else { crate::i18n::s().dialogs.move_btn })
                    .primary()
            } else {
                Button::new(format!("move-target:{path}"))
                    .label(crate::i18n::s().dialogs.move_btn)
            };
            rows.push(
                row()
                    .gap(px(14.))
                    .p(px(10.))
                    .my(px(3.))
                    .rounded(px(8.))
                    .bg(rgba(crate::theme::card_bg_subtle()))
                    .border_1()
                    .border_color(rgba(crate::theme::card_border()))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(3.))
                            .child(div().font_weight(FontWeight::MEDIUM).child(bottle.name.clone()))
                            .child(muted(bottle.path.clone()).truncate()),
                    )
                    .child(button.disabled(state.pending).on_click(cx.listener(
                        move |this, _, _, cx| this.start_move_entry(path.clone(), cx),
                    )))
                    .into_any_element(),
            );
        }
        if targets == 0 {
            rows.push(muted(crate::i18n::s().dialogs.move_no_other_bottles).py(px(8.)).into_any_element());
        }
        self.form(
            "move-entry-scroll",
            vec![self.section("move-entry", crate::i18n::s().dialogs.move_target_title, rows, cx)],
        )
    }

    fn open_file_sheet(&self, cx: &Context<Self>) -> AnyElement {
        let Some(state) = self.open_file.as_ref() else {
            return self.form(
                "open-file-scroll",
                vec![self.section(
                    "open-file",
                    crate::i18n::s().dialogs.open_file_title,
                    vec![muted(crate::i18n::s().dialogs.open_file_empty).into_any_element()],
                    cx,
                )],
            );
        };
        let mut rows: Vec<AnyElement> = vec![
            self.info(crate::i18n::s().dialogs.file_label, state.name.clone()),
            self.info(crate::i18n::s().common.location, state.path.clone()),
        ];
        for (index, bottle) in self.bottles.iter().enumerate() {
            rows.push(
                row()
                    .gap(px(14.))
                    .p(px(10.))
                    .my(px(3.))
                    .rounded(px(8.))
                    .bg(rgba(crate::theme::card_bg_subtle()))
                    .border_1()
                    .border_color(rgba(crate::theme::card_border()))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(3.))
                            .child(div().font_weight(FontWeight::MEDIUM).child(bottle.name.clone()))
                            .child(muted(bottle.path.clone()).truncate()),
                    )
                    .child(
                        Button::new(format!("run-in:{}", bottle.path))
                            .label(if bottle.available { crate::i18n::s().common.run } else { crate::i18n::s().common.unavailable })
                            .primary()
                            .disabled(!bottle.available)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.start_open_file(index, cx);
                            })),
                    )
                    .into_any_element(),
            );
        }
        self.form(
            "open-file-scroll",
            vec![self.section("open-file", crate::i18n::s().dialogs.open_file_title, rows, cx)],
        )
    }

    /// Metadata search, cover choice and local GameDB recommendations for the
    /// card that opened the sheet.
    fn game_info_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(page) = self.game_info.as_ref() else {
            return div().into_any_element();
        };
        let container = page.container.clone();
        let entry_id = page.entry_id.clone();
        let title = page.title.clone();
        let searching = page.searching;
        let applying = page.applying;
        let error = page.error.clone();
        let groups = page.groups.clone();
        let matches = page.matches.clone();
        let config_message = page.config_message.clone();
        let editable = page.editor.loaded && !page.editor.busy;
        let loaded = page.editor.loaded;
        let message = page.editor.message.clone();
        let bindings = page.editor.record["bindings"].clone();
        let snapshots = page.editor.record["snapshots"].clone();
        let default_source = page.editor.source("default").to_owned();
        let basic_source = page.editor.effective_source("basic").to_owned();
        let allowed = self.metadata_sources();
        let dirty = self.metadata_editor_dirty(cx);

        let game_entry = self
            .entries
            .iter()
            .find(|g| g.container == container && g.id == entry_id);
        let game_art = game_entry.and_then(|g| g.art.clone());
        let game_meta_line = game_entry.map(|g| {
            [g.studio.as_deref(), g.release.as_deref()]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
        }).filter(|s| !s.is_empty());

        let input = self.metadata_search_input(window, cx);

        // Hero summary card: brings title, cover, binding status, and unsaved indicator into focus.
        let mut bound_pills: Vec<AnyElement> = crate::metadata_editor::PROVIDERS
            .iter()
            .filter_map(|&source| {
                let label = crate::onboarding::source_label(source);
                let id = bindings[source].as_str().unwrap_or("");
                (!id.is_empty()).then(|| {
                    info_pill(
                        format!("{label} · {id}"),
                        crate::theme::installed_bg(),
                        crate::theme::installed_fg(),
                    )
                })
            })
            .collect();
        if bound_pills.is_empty() && loaded {
            bound_pills.push(info_pill(
                crate::i18n::s().dialogs.not_linked_to_an_online_source,
                crate::theme::badge_bg(),
                crate::theme::warning_fg(),
            ));
        }
        if loaded {
            bound_pills.push(info_pill(
                crate::i18n::s().dialogs.default_source(source_label(&default_source)),
                crate::theme::badge_bg(),
                crate::theme::badge_text(),
            ));
        }

        let hero_warning = if loaded && allowed.is_empty() {
            Some(crate::i18n::s().dialogs.no_metadata_sources.to_owned())
        } else if loaded && bindings[&basic_source].as_str().is_none_or(str::is_empty) {
            Some({
                let src = source_label(&basic_source);
                crate::i18n::s().dialogs.basic_source_unlinked(src)
            })
        } else {
            None
        };

        let hero_cover = match game_art {
            Some(path) => self
                .artwork_image(path)
                .w(px(54.))
                .h(px(76.))
                .flex_shrink_0()
                .rounded(px(7.))
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => div()
                .w(px(54.))
                .h(px(76.))
                .flex_shrink_0()
                .rounded(px(7.))
                .bg(gradient(
                    crate::theme::placeholder_gradient().0,
                    crate::theme::placeholder_gradient().1,
                ))
                .flex()
                .items_center()
                .justify_center()
                .child(icon("package", 20.).text_color(rgb(crate::theme::muted_color())))
                .into_any_element(),
        };

        let mut hero_card = col()
            .flex_shrink_0()
            .gap(px(10.))
            .p(px(16.))
            .rounded(px(10.))
            .bg(rgba(crate::theme::section_bg()))
            .border_1()
            .border_color(rgba(crate::theme::section_border()))
            .child(
                row()
                    .gap(px(14.))
                    .items_start()
                    .child(hero_cover)
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(6.))
                            .child(
                                row()
                                    .justify_between()
                                    .gap(px(8.))
                                    .flex_wrap()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(px(16.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(crate::theme::text()))
                                            .child(title),
                                    )
                                    .when(dirty, |d| {
                                        d.child(info_pill(
                                            crate::i18n::s().dialogs.unsaved_changes,
                                            crate::theme::accent_subtle_bg(),
                                            crate::theme::accent_fg(),
                                        ))
                                    }),
                            )
                            .when_some(game_meta_line, |d, line| {
                                d.child(muted(line).text_size(px(12.)).truncate())
                            })
                            .child(row().gap(px(6.)).flex_wrap().children(bound_pills)),
                    ),
            );
        if let Some(warning) = hero_warning {
            hero_card = hero_card.child(
                row()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(rgba(0xf5a62318))
                    .border_1()
                    .border_color(rgba(0xf5a62344))
                    .child(icon("triangle-alert", 13.).text_color(rgb(crate::theme::warning_fg())))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(11.5))
                            .line_height(relative(1.4))
                            .text_color(rgb(crate::theme::warning_fg()))
                            .child(warning),
                    ),
            );
        }

        // Section 1: Online Search & Bindings
        let search_row = row()
            .id("metadata-search-row")
            .gap(px(8.))
            .py(px(4.))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" {
                    this.command(Command::MetadataSearch, cx);
                }
            }))
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&input)
                        .prefix(icon("search", 13.).text_color(rgb(crate::theme::muted_color())))
                        .cleanable(true),
                ),
            )
            .child(
                Button::new("metadata-bind-id")
                    .label(crate::i18n::s().dialogs.link_by_id_url)
                    .disabled(!editable || searching)
                    .on_click(cx.listener(|this, _, _, cx| this.bind_metadata_id(cx))),
            )
            .child(
                Button::new("metadata-search")
                    .label(if searching { crate::i18n::s().common.searching } else { crate::i18n::s().common.search })
                    .primary()
                    .disabled(
                        searching
                            || !editable
                            || !self.service_available("metadata.search")
                            || self.flag("offline")
                            || self.metadata_sources().is_empty(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.command(Command::MetadataSearch, cx);
                    })),
            )
            .into_any_element();

        let mut search_rows: Vec<AnyElement> = Vec::new();
        let mut bound_cards: Vec<AnyElement> = Vec::new();
        for &source in &crate::metadata_editor::PROVIDERS {
            let label = crate::onboarding::source_label(source);
            let id = bindings[source].as_str().unwrap_or("");
            if !id.is_empty() {
                let snap_title = snapshots[source]["title"].as_str().unwrap_or("");
                let snap_title = if snap_title.is_empty() {
                    crate::i18n::s().dialogs.no_snapshot_save_refresh
                } else {
                    snap_title
                };
                bound_cards.push(
                    row()
                        .gap(px(8.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(8.))
                        .bg(rgba(crate::theme::card_bg_subtle()))
                        .border_1()
                        .border_color(rgba(crate::theme::card_border()))
                        .child(info_pill(
                            label,
                            crate::theme::installed_bg(),
                            crate::theme::installed_fg(),
                        ))
                        .child(info_pill(
                            format!("ID {id}"),
                            crate::theme::badge_bg(),
                            crate::theme::badge_text(),
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(12.))
                                .text_color(rgb(crate::theme::text()))
                                .child(snap_title.to_owned()),
                        )
                        .child(
                            Button::new(format!("metadata-unbind-{source}"))
                                .ghost()
                                .label(crate::i18n::s().dialogs.unlink)
                                .disabled(!editable)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.unbind_metadata_source(source, cx)
                                })),
                        )
                        .into_any_element(),
                );
            }
        }
        if !bound_cards.is_empty() {
            search_rows.push(
                col()
                    .gap(px(6.))
                    .py(px(4.))
                    .child(
                        muted(crate::i18n::s().dialogs.currently_linked_draft)
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD),
                    )
                    .children(bound_cards)
                    .into_any_element(),
            );
            search_rows.push(div().w_full().h(px(1.)).my(px(4.)).bg(rgba(crate::theme::divider())).into_any_element());
        }

        search_rows.push(self.metadata_source_row(cx));
        search_rows.push(search_row);
        if searching {
            search_rows.push(
                row()
                    .gap(px(8.))
                    .py(px(8.))
                    .child(Spinner::new().with_size(px(14.)))
                    .child(muted(crate::i18n::s().dialogs.querying_selected_sources))
                    .into_any_element(),
            );
        }
        if let Some(error) = &error {
            search_rows.push(
                row()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(rgba(0xff5f571a))
                    .border_1()
                    .border_color(rgba(0xff5f574d))
                    .child(icon("triangle-alert", 13.).text_color(rgb(crate::theme::danger_fg())))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(rgb(crate::theme::danger_fg()))
                            .child(error.clone()),
                    )
                    .into_any_element(),
            );
        }
        if let Some(message) = message {
            search_rows.push(
                row()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(rgba(crate::theme::installed_bg()))
                    .border_1()
                    .border_color(rgba(crate::theme::card_border()))
                    .child(icon("check", 13.).text_color(rgb(crate::theme::installed_fg())))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(rgb(crate::theme::installed_fg()))
                            .child(message),
                    )
                    .into_any_element(),
            );
        }
        for (group_index, group) in groups.iter().enumerate() {
            search_rows.push(
                muted(source_label(&group.source))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .mt(px(6.))
                    .into_any_element(),
            );
            if let Some(provider_error) = &group.error {
                search_rows.push(
                    muted(crate::i18n::s().dialogs.query_failed(provider_error))
                        .text_size(px(11.))
                        .into_any_element(),
                );
            }
            for (game_index, candidate) in group.games.iter().enumerate() {
                let candidate_id = candidate.raw["sourceID"].as_str().unwrap_or("");
                let already_bound = !candidate_id.is_empty()
                    && bindings[&group.source].as_str() == Some(candidate_id);
                let mut details = candidate.developer.clone();
                if !candidate_id.is_empty() {
                    details = if details.is_empty() {
                        format!("ID {candidate_id}")
                    } else {
                        format!("ID {candidate_id} · {details}")
                    };
                }
                if !candidate.release_date.is_empty() {
                    if !details.is_empty() {
                        details.push_str(" · ");
                    }
                    details.push_str(&candidate.release_date);
                }
                search_rows.push(
                    row()
                        .gap(px(12.))
                        .p(px(10.))
                        .my(px(3.))
                        .rounded(px(8.))
                        .bg(rgba(if already_bound {
                            crate::theme::accent_subtle_bg()
                        } else {
                            crate::theme::card_bg_subtle()
                        }))
                        .border_1()
                        .border_color(rgba(if already_bound {
                            crate::theme::accent_subtle_border()
                        } else {
                            crate::theme::card_border()
                        }))
                        .child(match &candidate.thumbnail {
                            // Narrow fixed-size thumbnail: the row stays
                            // compact, and a placeholder keeps the height
                            // consistent when a provider has no image.
                            Some(path) => self
                                .artwork_image(path.clone())
                                .w(px(34.))
                                .h(px(48.))
                                .flex_shrink_0()
                                .rounded(px(5.))
                                .object_fit(ObjectFit::Cover)
                                .into_any_element(),
                            None => div()
                                .w(px(34.))
                                .h(px(48.))
                                .flex_shrink_0()
                                .rounded(px(5.))
                                .bg(gradient(
                                    crate::theme::placeholder_gradient().0,
                                    crate::theme::placeholder_gradient().1,
                                ))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    icon("package", 16.)
                                        .text_color(rgb(crate::theme::muted_color())),
                                )
                                .into_any_element(),
                        })
                        .child(
                            col()
                                .flex_1()
                                .min_w_0()
                                .gap(px(3.))
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate()
                                        .child(candidate.title.clone()),
                                )
                                .when(!details.is_empty(), |d| {
                                    d.child(muted(details).text_size(px(11.)))
                                })
                                .when(!candidate.summary.is_empty(), |d| {
                                    d.child(
                                        muted(candidate.summary.clone())
                                            .text_size(px(11.))
                                            .line_clamp(2),
                                    )
                                }),
                        )
                        .child(
                            Button::new(format!("metadata-apply:{group_index}:{game_index}"))
                                .label(if already_bound {
                                    crate::i18n::s().dialogs.in_draft
                                } else {
                                    crate::i18n::s().dialogs.link_to_draft
                                })
                                .disabled(!editable || already_bound)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.command(
                                        Command::MetadataApply(group_index, game_index),
                                        cx,
                                    );
                                })),
                        )
                        .into_any_element(),
                );
            }
        }
        if groups.is_empty() && !searching {
            search_rows.push(
                muted(crate::i18n::s().dialogs.search_or_link_tip)
                    .text_size(px(11.))
                    .line_height(relative(1.4))
                    .into_any_element(),
            );
        }

        // Section 2: Manual Editing & Custom Cover
        let mut edit_rows = self.metadata_manual_rows(window, cx);
        if !edit_rows.is_empty() {
            edit_rows.push(div().w_full().h(px(1.)).my(px(4.)).bg(rgba(crate::theme::divider())).into_any_element());
        }
        edit_rows.push(
            row()
                .gap(px(12.))
                .py(px(6.))
                .items_center()
                .flex_wrap()
                .child(
                    col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.))
                        .child(crate::i18n::s().dialogs.custom_cover)
                        .child(
                            muted(crate::i18n::s().dialogs.local_image_priority_tip)
                                .text_size(px(11.)),
                        ),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .child(self.action(
                            "cover-import",
                            crate::i18n::s().dialogs.choose_local_image,
                            Command::MetadataCover,
                            cx,
                        ))
                        .child(self.action(
                            "cover-reset",
                            crate::i18n::s().dialogs.reset_to_default_cover,
                            Command::MetadataCoverReset,
                            cx,
                        ))
                        .when(applying, |d| d.child(Spinner::new().with_size(px(14.)))),
                )
                .into_any_element(),
        );

        // Section 3: Source Policy & Characters Preview
        let source_rows = self.metadata_policy_and_character_rows(cx);

        let mut config_rows: Vec<AnyElement> = Vec::new();
        match matches {
            None if self.game_info.as_ref().is_some_and(|page| page.config_error.is_some()) => {
                let reason = self.game_info.as_ref().and_then(|page| page.config_error.clone()).unwrap_or_default();
                config_rows.push(row().gap(px(12.)).py(px(10.))
                    .child(div().flex_1().child(muted(reason)))
                    .child(Button::new("gamedb-retry").label(crate::i18n::s().dialogs.reload)
                        .on_click(cx.listener(|this, _, _, cx| this.retry_game_db(cx))))
                    .into_any_element());
            }
            None => {
                config_rows.push(
                    row()
                        .gap(px(8.))
                        .py(px(8.))
                        .child(Spinner::new().with_size(px(14.)))
                        .child(muted(crate::i18n::s().dialogs.matching_local_gamedb))
                        .into_any_element(),
                );
            }
            Some(matches) if matches.is_empty() => {
                config_rows.push(
                    muted(crate::i18n::s().dialogs.no_gamedb_match)
                        .into_any_element(),
                );
            }
            Some(matches) => {
                for (index, matched) in matches.iter().enumerate() {
                    let entry_title = matched
                        .entry
                        .get("title")
                        .and_then(|value| value.as_str())
                        .unwrap_or(crate::i18n::s().dialogs.gamedb_entry);
                    let rating = matched
                        .entry
                        .get("rating")
                        .and_then(|value| value.as_str())
                        .unwrap_or("");
                    let pct = matched.confidence * 100.; let mut detail = crate::i18n::s().dialogs.match_pct(format!("{:.0}", pct));
                    if matched.reference_platform == "macos" {
                        detail.push_str(crate::i18n::s().dialogs.macos_reference);
                    }
                    if !rating.is_empty() {
                        detail.push_str(" · ");
                        detail.push_str(&compat_label(rating));
                    }
                    let mut rows = vec![
                        row()
                            .gap(px(12.))
                            .child(
                                col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(3.))
                                    .child(div().font_weight(FontWeight::MEDIUM).child(entry_title.to_owned()))
                                    .child(muted(detail).text_size(px(11.)))
                                    .when(!matched.explanation.is_empty(), |d| {
                                        d.child(muted(matched.explanation.clone()).text_size(px(11.)))
                                    }),
                            )
                            .child(
                                Button::new(format!("gamedb-apply:{index}"))
                                    .label(if matched.apply_available(self.service_available("gameConfig.apply")) {
                                        crate::i18n::s().dialogs.apply_recommended
                                    } else { crate::i18n::s().dialogs.config_reference })
                                    .primary()
                                    .disabled(applying || !matched.apply_available(self.service_available("gameConfig.apply")))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.command(Command::GameDbApply(index), cx);
                                    })),
                            )
                            .into_any_element(),
                    ];
                    if !matched.availability_reason.is_empty() {
                        rows.push(muted(matched.availability_reason.clone()).text_size(px(11.)).into_any_element());
                    }
                    for change in &matched.changes {
                        rows.push(
                            muted(format!(
                                "{}{} · {}：{} → {}",
                                if change.high_impact { "⚠ " } else { "" },
                                change.category,
                                change.setting,
                                change.current,
                                change.next
                            ))
                            .text_size(px(11.))
                            .into_any_element(),
                        );
                    }
                    config_rows.push(
                        col()
                            .gap(px(6.))
                            .p(px(10.))
                            .my(px(3.))
                            .rounded(px(8.))
                            .bg(rgba(crate::theme::card_bg_subtle()))
                            .border_1()
                            .border_color(rgba(crate::theme::card_border()))
                            .children(rows)
                            .into_any_element(),
                    );
                }
            }
        }
        if let Some(message) = config_message {
            config_rows.push(muted(message).into_any_element());
        }

        self.form(
            "game-info-scroll",
            vec![
                hero_card.into_any_element(),
                self.section("game-metadata-search", crate::i18n::s().dialogs.online_match_link, search_rows, cx),
                self.section("game-metadata-edit", crate::i18n::s().dialogs.manual_edit_cover, edit_rows, cx),
                self.section("game-metadata-sources", crate::i18n::s().dialogs.sources_character_preview, source_rows, cx),
                self.section(
                    "game-gamedb",
                    if self.service_available("gameConfig.apply") {
                        crate::i18n::s().dialogs.gamedb_recommended_config
                    } else {
                        crate::i18n::s().dialogs.gamedb_config_reference
                    },
                    config_rows,
                    cx,
                ),
            ],
        )
    }

    pub(crate) fn preview_sheet(
        &mut self,
        sheet: PreviewSheet,
        width: f32,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !matches!(sheet, PreviewSheet::GameInfo(_)) {
            self.game_info = None;
        }
        if matches!(sheet, PreviewSheet::RunSettings(_) | PreviewSheet::Executable)
            && self.preview_sheet_return_captured_gen != self.preview_sheet_generation
        {
            self.preview_sheet_return_focus = window.focused(cx);
            self.preview_sheet_return_captured_gen = self.preview_sheet_generation;
        }
        let d = &crate::i18n::s().dialogs;
        let title = match sheet {
            PreviewSheet::RunSettings(_) => d.sheet_run_settings,
            PreviewSheet::Scan => d.sheet_scan,
            PreviewSheet::Executable => {
                if self
                    .executable_picker
                    .as_ref()
                    .is_some_and(|picker| picker.multi_select)
                {
                    d.sheet_entries_config
                } else {
                    d.sheet_entries_select
                }
            }
            PreviewSheet::Winetricks => "Winetricks",
            PreviewSheet::CreateBottle => d.sheet_create_bottle,
            PreviewSheet::MoveEntry(_) => d.sheet_move_entry,
            PreviewSheet::OpenFile => d.sheet_open_file,
            PreviewSheet::Diagnostics => {
                if cfg!(target_os = "linux") { d.sheet_diagnostics_linux } else { d.sheet_diagnostics_mac }
            },
            PreviewSheet::Audio => d.sheet_audio,
            PreviewSheet::Dll => d.sheet_dll,
            PreviewSheet::Graphics => d.sheet_graphics,
            PreviewSheet::GameInfo(_) => d.sheet_game_info,
            PreviewSheet::GameTags(_) => d.sheet_game_tags,
            PreviewSheet::ManageTags => d.sheet_manage_tags,
        };
        let content = match sheet {
            PreviewSheet::RunSettings(i) => self.run_settings(i, window, cx),
            PreviewSheet::MoveEntry(_) => self.move_entry_sheet(cx),
            PreviewSheet::OpenFile => self.open_file_sheet(cx),
            PreviewSheet::Scan => self.scan_settings(window, cx),
            PreviewSheet::Executable => self.executable_picker(window, cx),
            PreviewSheet::GameTags(i) => self.game_tags_sheet(i, cx),
            PreviewSheet::ManageTags => self.manage_tags_sheet(cx),
            PreviewSheet::Winetricks => self.winetricks_sheet(cx),
            PreviewSheet::CreateBottle => {
                let state = self.create_bottle.as_ref();
                let running = state.is_some_and(|s| s.running());
                let finished = state.is_some_and(|s| s.finished);
                let progress = state.and_then(|s| {
                    s.status.and_then(|status| {
                        s.stage.as_ref().map(|stage| {
                            crate::create_bottle::progress_label(status, Some(stage)).to_owned()
                        })
                    })
                });
                let error = state.and_then(|s| s.error.clone());

                if running {
                    // The sheet stays open for the whole job; without an
                    // in-sheet cue the click reads as "nothing happened".
                    self.form(
                        "create-bottle-scroll",
                        vec![self.section(
                            "create-progress",
                            crate::i18n::s().dialogs.creating_label,
                            vec![row()
                                .gap(px(10.))
                                .items_center()
                                .py(px(10.))
                                .child(Spinner::new().with_size(px(14.)))
                                .child(
                                    progress.unwrap_or_else(|| crate::i18n::s().dialogs.initializing_label.to_owned()),
                                )
                                .into_any_element()],
                            cx,
                        )],
                    )
                } else if finished {
                    let (title, content) = if error.is_some() {
                        (crate::i18n::s().dialogs.create_failed, error.unwrap_or_else(|| crate::i18n::s().common.unknown.to_owned()))
                    } else {
                        (crate::i18n::s().dialogs.create_complete, crate::i18n::s().dialogs.create_complete_desc.to_owned())
                    };
                    self.form(
                        "create-bottle-scroll",
                        vec![self.section(
                            "create-result",
                            title,
                            vec![self.info(crate::i18n::s().common.details, content)],
                            cx,
                        )],
                    )
                } else {
                    let location = self
                        .default_bottle_location
                        .as_ref()
                        .map(|path| crate::create_bottle::display_bottle_location(path))
                        .unwrap_or_else(|| {
                            if cfg!(target_os = "macos") {
                                "~/Library/Containers/com.daydalek.Maltoria/Bottles".to_owned()
                            } else {
                                "~/.local/share/whisky/Bottles".to_owned()
                            }
                        });
                    self.form(
                        "create-bottle-scroll",
                        vec![self.section(
                            "create",
                            crate::i18n::s().dialogs.new_bottle_section,
                            vec![
                                div()
                                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                        if event.keystroke.key == "enter"
                                            && !this.create_bottle.as_ref().is_some_and(|s| s.running() || s.finished)
                                        {
                                            this.submit_create_bottle(cx);
                                        }
                                    }))
                                    .child(self.field("bottle-name", crate::i18n::s().common.name))
                                    .into_any_element(),
                                self.choice(
                                    "create-windows",
                                    crate::i18n::config::field_title("config.field.windowsVersion", "windowsVersion", "Windows Version"),
                                    crate::create_bottle::WINDOWS_LABELS,
                                    cx,
                                ),
                                self.create_engine_choice(cx),
                                self.info(crate::i18n::s().common.location, &location),
                                self.info(crate::i18n::s().common.architecture, crate::i18n::s().common.bits64),
                            ],
                            cx,
                        )],
                    )
                }
            }
            PreviewSheet::GameInfo(_) => self.game_info_sheet(window, cx),
            PreviewSheet::Dll => self.form(
                "dll-scroll",
                vec![self.section(
                    "dll-overrides",
                    crate::i18n::s().dialogs.sheet_dll,
                    vec![
                        self.info("d3dcompiler_47", crate::i18n::s().dialogs.native_then_builtin),
                        self.field("dll-name", crate::i18n::s().dialogs.add_override),
                        self.choice(
                            "dll-mode",
                            crate::i18n::s().dialogs.load_order,
                            &[
                                crate::i18n::s().dialogs.native_then_builtin,
                                crate::i18n::s().dialogs.builtin_then_native,
                                crate::i18n::s().program_settings.dll_native_only,
                                crate::i18n::s().program_settings.dll_builtin_only,
                                crate::i18n::s().program_settings.dll_disabled,
                            ],
                            cx,
                        ),
                    ],
                    cx,
                )],
            ),
            PreviewSheet::Graphics => self.form(
                "graphics-scroll",
                vec![self.section(
                    "advanced-graphics",
                    crate::i18n::s().dialogs.graphics_debugging,
                    #[cfg(not(target_os = "linux"))]
                    vec![
                        self.toggle("metal-hud", "Metal HUD", "", cx),
                        self.toggle("metal-trace", crate::i18n::config::field_title("config.field.metalTrace", "metalTrace", "Metal Trace"), "", cx),
                        self.toggle("dxvk-async", "DXVK Async", "", cx),
                        self.choice("dxvk-hud", "DXVK HUD", &[crate::i18n::s().common.off, "FPS", crate::i18n::s().program_settings.hud_full], cx),
                        self.toggle("dxr", crate::i18n::config::field_title("config.field.dxrEnabled", "dxrEnabled", "DXR"), "", cx),
                    ],
                    #[cfg(target_os = "linux")]
                    vec![
                        self.toggle("dxvk-async", "DXVK Async", "", cx),
                        self.choice("dxvk-hud", "DXVK HUD", &[crate::i18n::s().common.off, "FPS", crate::i18n::s().program_settings.hud_full], cx),
                    ],
                    cx,
                )],
            ),
            PreviewSheet::Audio => {
                let mut rows = vec![
                    self.info(crate::i18n::s().dialogs.n_1_output_device, crate::i18n::s().dialogs.system_default_output),
                    self.info(crate::i18n::s().dialogs.n_2_test_method, crate::i18n::s().dialogs.wine_winmm_440_hz_test_tone),
                    self.action(
                        "test-audio",
                        if self.audio_pending {
                            crate::i18n::s().dialogs.playing_test_tone
                        } else {
                            crate::i18n::s().dialogs.play_test_audio
                        },
                        Command::TestAudio,
                        cx,
                    ),
                ];
                if self.audio_pending {
                    rows.push(
                        row()
                            .gap(px(8.))
                            .py(px(8.))
                            .child(Spinner::new().with_size(px(14.)))
                            .child(muted(crate::i18n::s().dialogs.starting_wine_audio_test))
                            .into_any_element(),
                    );
                }
                if let Some(error) = &self.audio_error {
                    rows.push(muted(error.clone()).into_any_element());
                }
                if let Some(result) = &self.audio_result {
                    let c = &crate::i18n::s().common;
                    let status = match result.status.as_str() {
                        "passed" => c.passed,
                        "warning" => c.warning,
                        "error" => c.error,
                        "skipped" => c.skipped,
                        _ => result.status.as_str(),
                    };
                    rows.push(self.info(crate::i18n::s().dialogs.test_result, status));
                    rows.push(self.info(crate::i18n::s().library.summary, result.summary.clone()));
                    for line in result.evidence.iter().take(12) {
                        rows.push(muted(line.clone()).text_size(px(11.)).into_any_element());
                    }
                    for finding in &result.findings {
                        rows.push(
                            self.info(
                                finding.description.clone(),
                                finding
                                    .suggested_action
                                    .clone()
                                    .unwrap_or_else(|| crate::i18n::s().dialogs.check_wine_audio_driver_settings.to_owned()),
                            ),
                        );
                    }
                    rows.push(
                        muted(crate::i18n::s().dialogs.audio_test_tip)
                            .text_size(px(11.))
                            .into_any_element(),
                    );
                }
                self.form(
                    "audio-wizard-scroll",
                    vec![self.section("audio-check", crate::i18n::s().dialogs.check_audio_output, rows, cx)],
                )
            }
            PreviewSheet::Diagnostics => {
                let symptoms: Vec<&'static str> = if cfg!(target_os = "linux") {
                    vec![
                        crate::i18n::s().dialogs.bottle_files_or_import_error,
                        crate::i18n::s().dialogs.permissions_or_storage_space,
                        crate::i18n::s().dialogs.host_environment_info,
                    ]
                } else {
                    vec![
                        crate::i18n::s().dialogs.game_won_t_launch,
                        crate::i18n::s().dialogs.black_screen_or_glitches,
                        crate::i18n::s().dialogs.no_audio,
                        crate::i18n::s().dialogs.garbled_text,
                        crate::i18n::s().dialogs.slow_performance,
                    ]
                };
                let mut rows = vec![
                    self.choice(
                        "diagnosis-symptom",
                        crate::i18n::s().dialogs.what_issue_are_you_experiencing,
                        &symptoms,
                        cx,
                    ),
                    self.action(
                        "checks",
                        if self.checks_pending {
                            crate::i18n::s().dialogs.running_auto_checks
                        } else {
                            crate::i18n::s().dialogs.run_auto_checks
                        },
                        Command::RunBottleChecks,
                        cx,
                    ),
                    self.action(
                        "analyze",
                        if self.diagnostics_pending {
                            crate::i18n::s().dialogs.collecting_diagnostics
                        } else if cfg!(target_os = "linux") {
                            crate::i18n::s().dialogs.collect_host_bottle_info
                        } else {
                            crate::i18n::s().dialogs.analyze_run_logs
                        },
                        Command::RunDiagnostics,
                        cx,
                    ),
                ];
                if self.checks_pending {
                    rows.push(
                        row()
                            .gap(px(8.))
                            .py(px(8.))
                            .child(Spinner::new().with_size(px(14.)))
                            .child(muted(crate::i18n::s().dialogs.preflight_running))
                            .into_any_element(),
                    );
                }
                if let Some(checks) = &self.diagnostic_checks {
                    for check in checks {
                        let c = &crate::i18n::s().common;
                        let (label, symbol, color) = match check.outcome.as_str() {
                            "pass" => (c.passed, "check", crate::theme::installed_fg()),
                            "already_configured" => (crate::i18n::s().dialogs.configured, "check", crate::theme::installed_fg()),
                            "fail" => (crate::i18n::s().dialogs.failed, "triangle-alert", crate::theme::warning_fg()),
                            "warning" => (c.warning, "triangle-alert", crate::theme::warning_fg()),
                            "error" => (c.error, "x", crate::theme::danger_fg()),
                            _ => (c.unknown, "info", crate::theme::muted_color()),
                        };
                        let check_label = match check.check_id.as_str() {
                            "crash.log_classify" => crate::i18n::s().dialogs.crash_logs,
                            "launcher.type_check" => crate::i18n::config::field_title("config.field.launcherMode", "launcherMode", "Launcher Type"),
                            "process.running_check" => crate::i18n::s().bottle.running_processes,
                            "audio.device_check" => crate::i18n::s().dialogs.audio_device,
                            "audio.driver_check" => crate::i18n::config::field_title("config.field.audioDriver", "audioDriver", "Audio Driver"),
                            "game.config_available" => crate::i18n::s().dialogs.gamedb_config,
                            "diagnostics.can_enhance" => crate::i18n::s().dialogs.diagnostic_log_level,
                            "host.container_metadata" => crate::i18n::s().dialogs.bottle_metadata,
                            "host.drive_c" => crate::i18n::s().dialogs.bottle_directory,
                            "host.container_write" => crate::i18n::s().dialogs.file_permissions,
                            "host.storage" => crate::i18n::s().dialogs.available_storage,
                            _ => check.check_id.as_str(),
                        };
                        let mut detail = format!("{check_label}：{label} · {}", check.summary);
                        for (_, value) in check.evidence.iter().take(2) {
                            detail.push_str(" · ");
                            detail.push_str(value);
                        }
                        rows.push(
                            row()
                                .gap(px(8.))
                                .py(px(5.))
                                .child(icon(symbol, 13.).text_color(rgb(color)))
                                .child(muted(detail).text_size(px(11.)))
                                .into_any_element(),
                        );
                    }
                }
                if self.diagnostics_pending {
                    rows.push(
                        row()
                            .gap(px(8.))
                            .py(px(8.))
                            .child(Spinner::new().with_size(px(14.)))
                            .child(muted(crate::i18n::s().dialogs.reading_system_info_and_recent_logs))
                            .into_any_element(),
                    );
                }
                if let Some(error) = &self.diagnostics_error {
                    rows.push(muted(error.clone()).into_any_element());
                }
                if let Some(report) = &self.diagnostics_report {
                    let report_scroll = self.preview.form_scroll_handle("diagnostics-report");
                    let report_viewport = div()
                        .id("diagnostics-report")
                        .w_full()
                        .max_h(px(280.))
                        .track_scroll(&report_scroll)
                        .overflow_y_scroll()
                        .restrict_scroll_to_axis()
                        .p(px(10.))
                        .rounded(px(8.))
                        .bg(rgba(crate::theme::code_bg()))
                        .border_1()
                        .border_color(rgba(crate::theme::card_border()))
                        .text_size(px(11.))
                        .line_height(relative(1.45))
                        .child(report.clone());
                    rows.push(self.bounded_scroll_view(
                        "diagnostics-report",
                        &report_scroll,
                        report_viewport,
                    ));
                    rows.push(self.action(
                        "export",
                        crate::i18n::s().dialogs.export_diagnostics_report,
                        Command::ExportDiagnostics,
                        cx,
                    ));
                }
                if cfg!(target_os = "macos") {
                    rows.push(self.action(
                        "audio-link",
                        crate::i18n::s().dialogs.audio_troubleshooter,
                        Command::Sheet(PreviewSheet::Audio),
                        cx,
                    ));
                }
                self.form(
                    "diagnostics-scroll",
                    vec![self.section(
                        "guided-diagnostics",
                        crate::i18n::s().dialogs.guided_diagnostics,
                        rows,
                        cx,
                    )],
                )
            }
        };
        let pw = (width - 64.).min(740.);
        let ph = (height - 64.).min(700.);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(rgba(crate::theme::overlay()))
            .child(
                div()
                    .id("settings-dismiss")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.preview.sheet = None;
                        this.preview.sheet_history.clear();
                        this.preview_sheet_return_captured_gen = 0;
                        this.program_settings.pending_switch = None;
                        this.executable_picker = None;
                        this.restore_preview_sheet_focus(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                col()
                    .id("preview-sheet-card")
                    .tab_group()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .absolute()
                    .left(px((width - pw) / 2.))
                    .top(px((height - ph) / 2.))
                    .w(px(pw))
                    .h(px(ph))
                    .rounded(px(14.))
                    .overflow_hidden()
                    .bg(rgb(crate::theme::surface()))
                    .border_1()
                    .border_color(rgba(crate::theme::surface_border()))
                    .shadow_xl()
                    .child(
                        row()
                            .flex_shrink_0()
                            .h(px(54.))
                            .px(px(18.))
                            .gap(px(12.))
                            .border_b_1()
                            .border_color(rgba(crate::theme::divider()))
                            .when(!self.preview.sheet_history.is_empty(), |d| {
                                d.child(
                                    Button::new("sheet-back")
                                        .ghost()
                                        .small()
                                        .icon(icon("chevron-left", 14.))
                                        .tooltip(crate::i18n::s().dialogs.return_prev_tip)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.preview.sheet = this.preview.sheet_history.pop();
                                            cx.notify();
                                        })),
                                )
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(
                                Button::new("sheet-close")
                                    .ghost()
                                    .small()
                                    .icon(icon("x", 14.))
                                    .tooltip(crate::i18n::s().common.close_esc)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.preview.sheet = None;
                                        this.preview.sheet_history.clear();
                                        this.preview_sheet_return_captured_gen = 0;
                                        this.program_settings.pending_switch = None;
                                        this.executable_picker = None;
                                        this.tag_manager_rename_target = None;
                                        this.game_info = None;
                                        this.restore_preview_sheet_focus(window, cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(div().flex_1().min_h_0().child(content))
                    .child(
                        row()
                            .flex_shrink_0()
                            .justify_end()
                            .gap(px(8.))
                            .px(px(18.))
                            .py(px(13.))
                            .border_t_1()
                            .border_color(rgba(crate::theme::divider()))
                            .when(
                                matches!(sheet, PreviewSheet::Executable)
                                    && self
                                        .executable_picker
                                        .as_ref()
                                        .is_some_and(|picker| picker.multi_select),
                                |d| {
                                    d.child(Button::new("executable-cancel").label(crate::i18n::s().common.cancel).on_click(
                                        cx.listener(|this, _, window, cx| {
                                            this.cancel_executable_picker(cx);
                                            if this.preview.sheet.is_none() {
                                                this.restore_preview_sheet_focus(window, cx);
                                            }
                                        }),
                                    ))
                                },
                            )
                            .when(matches!(sheet, PreviewSheet::GameInfo(_)), |d| {
                                let busy = self.game_info.as_ref().is_some_and(|page| page.editor.busy);
                                let allowed = self.metadata_sources();
                                let can_refresh = self.game_info.as_ref().is_some_and(|page|
                                    page.editor.loaded && !page.editor.busy
                                        && page.editor.record["bindings"].as_object().is_some_and(|bindings|
                                            bindings.keys().any(|source| allowed.contains(source)
                                                && page.editor.capabilities[source]["refresh"].as_bool() == Some(true))));
                                d.child(Button::new("metadata-refresh-bound").label(crate::i18n::s().dialogs.refresh_bound_btn)
                                    .disabled(!can_refresh || self.metadata_editor_dirty(cx) || self.flag("offline")
                                        || !self.service_available("metadata.refresh"))
                                    .on_click(cx.listener(|this, _, _, cx| this.refresh_metadata_editor(cx))))
                                    .child(div().flex_1())
                                    .child(Button::new("metadata-cancel-draft").label(crate::i18n::s().common.cancel)
                                        .disabled(busy)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.preview_dismiss(cx);
                                            this.restore_preview_sheet_focus(window, cx);
                                        })))
                            })
                            .when(cfg!(target_os = "linux") && !self.demo
                                && matches!(sheet, PreviewSheet::CreateBottle), |d| {
                                d.child(Button::new("import-existing-bottle")
                                    .label(crate::i18n::s().dialogs.import_existing_bottle)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.preview.sheet = None;
                                        this.restore_preview_sheet_focus(window, cx);
                                        this.begin_import_scan(None, cx);
                                        cx.notify();
                                    })))
                            })
                            .when(matches!(sheet, PreviewSheet::RunSettings(_)), |d| {
                                let index = match sheet {
                                    PreviewSheet::RunSettings(index) => index,
                                    _ => 0,
                                };
                                let multi_apply_visible = !self.demo
                                    && self
                                        .game(index)
                                        .is_some_and(|g| g.multi_executable && g.launch_executables.len() >= 2)
                                    && self.program_settings.executable.is_some();
                                let multi_apply_dirty = multi_apply_visible && self.program_settings_is_dirty(cx);
                                d.when(
                                    self.demo || self.program_settings.executable.is_some(),
                                    |row| {
                                        row.child(self.action(
                                            "shortcut",
                                            crate::i18n::s().program_settings.create_shortcut,
                                            Command::Shortcut,
                                            cx,
                                        ))
                                    },
                                )
                                .child(self.action(
                                    "reveal",
                                    crate::platform::reveal_label(),
                                    Command::Reveal(index),
                                    cx,
                                ))
                                .when(self.game(index).is_some_and(|game| game.game_directory().is_some()), |d| {
                                    d.child(self.action(
                                        "open-game-directory",
                                        crate::i18n::s().library.open_game_dir,
                                        Command::OpenGameDirectory(index),
                                        cx,
                                    ))
                                })
                                .when(multi_apply_visible, |d| {
                                    d.child(
                                        Button::new("run-settings-apply")
                                            .label(crate::i18n::s().dialogs.save_only)
                                            .disabled(
                                                self.program_settings.loading
                                                    || self.program_settings.saving
                                                    || self.program_settings.settings.is_none()
                                                    || !multi_apply_dirty,
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.save_program_settings_keep_open(cx);
                                            })),
                                    )
                                })
                            })
                            .child(
                                Button::new("sheet-done")
                                    .label({
                                        let c = &crate::i18n::s().common;
                                        let d = &crate::i18n::s().dialogs;
                                        if matches!(sheet, PreviewSheet::GameInfo(_)) {
                                            if self.game_info.as_ref().is_some_and(|page| page.editor.busy) {
                                                c.processing.to_owned()
                                            } else { d.save_metadata.to_owned() }
                                        } else if matches!(sheet, PreviewSheet::Executable)
                                            && self.executable_picker.as_ref().is_some_and(|p| p.multi_select)
                                        {
                                            let picker = self.executable_picker.as_ref().unwrap();
                                            let n = picker.selected.len();
                                            if picker.launch_after {
                                                if n > 1 {
                                                    crate::i18n::s().dialogs.continue_launch(n)
                                                } else {
                                                    d.save_and_launch.to_owned()
                                                }
                                            } else {
                                                crate::i18n::s().dialogs.save_entries(n)
                                            }
                                        } else if !self.demo
                                            && matches!(sheet, PreviewSheet::RunSettings(_))
                                        {
                                            if self.program_settings.executable.is_none() {
                                                c.close.to_owned()
                                            } else if self.program_settings.saving {
                                                c.saving.to_owned()
                                            } else {
                                                c.save.to_owned()
                                            }
                                        } else if matches!(sheet, PreviewSheet::MoveEntry(_)) {
                                            if self.move_entry.as_ref().is_some_and(|state| state.pending) {
                                                d.moving.to_owned()
                                            } else {
                                                c.cancel.to_owned()
                                            }
                                        } else if matches!(sheet, PreviewSheet::OpenFile) {
                                            c.cancel.to_owned()
                                        } else if matches!(sheet, PreviewSheet::CreateBottle) {
                                            let state = self.create_bottle.as_ref();
                                            if state.is_some_and(|s| s.running()) {
                                                d.creating_btn.to_owned()
                                            } else if state.is_some_and(|s| s.finished) {
                                                c.close.to_owned()
                                            } else {
                                                d.create_btn.to_owned()
                                            }
                                        } else if matches!(sheet, PreviewSheet::Winetricks) {
                                            let job = self.winetricks_job();
                                            if job.is_some_and(crate::winetricks::InstallJob::running) {
                                                d.installing_btn.to_owned()
                                            } else if job.is_some_and(|job| job.finished) {
                                                c.close.to_owned()
                                            } else {
                                                d.install_selected_btn.to_owned()
                                            }
                                        } else {
                                            c.done.to_owned()
                                        }
                                    })
                                    .primary()
                                    .disabled(
                                        (matches!(sheet, PreviewSheet::GameInfo(_))
                                            && (!self.game_info.as_ref().is_some_and(|page| page.editor.loaded && !page.editor.busy)
                                                || !self.service_available("metadata.configuration.save")))
                                            || (matches!(sheet, PreviewSheet::Executable)
                                                && self.executable_picker.as_ref().is_some_and(|p| p.multi_select && (p.loading || p.selected.is_empty())))
                                            || (matches!(sheet, PreviewSheet::CreateBottle)
                                            && self.create_bottle.as_ref().is_some_and(|s| s.running()))
                                            || (!self.demo
                                                && matches!(sheet, PreviewSheet::RunSettings(_))
                                                && self.program_settings.executable.is_some()
                                                && (self.program_settings.loading
                                                    || self.program_settings.saving
                                                    || self.program_settings.settings.is_none()))
                                            || (matches!(sheet, PreviewSheet::MoveEntry(_))
                                                && self.move_entry.as_ref().is_some_and(|state| state.pending))
                                            || (!self.demo
                                                && matches!(sheet, PreviewSheet::Winetricks)
                                                && self.winetricks_job().is_some_and(crate::winetricks::InstallJob::running))
                                            || (!self.demo
                                                && matches!(sheet, PreviewSheet::Winetricks)
                                                && !self.winetricks_job().is_some_and(|job| job.finished)
                                                && self.winetricks.selected.is_none()),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if matches!(this.preview.sheet, Some(PreviewSheet::GameInfo(_))) {
                                            this.save_metadata_editor(cx);
                                            return;
                                        }
                                        if matches!(this.preview.sheet, Some(PreviewSheet::Executable))
                                            && this.executable_picker.as_ref().is_some_and(|p| p.multi_select)
                                        {
                                            this.confirm_multi_executable_picker(cx);
                                            if this.preview.sheet.is_none() && this.launch_chooser.is_none() {
                                                this.restore_preview_sheet_focus(window, cx);
                                            }
                                            return;
                                        }
                                        if matches!(this.preview.sheet, Some(PreviewSheet::CreateBottle)) {
                                            let state = this.create_bottle.as_ref();
                                            if state.is_some_and(|s| s.running()) {
                                                // Do nothing while running
                                                return;
                                            }
                                            if state.is_some_and(|s| s.finished) {
                                                // Close on finished and clear state
                                                this.create_bottle = None;
                                                this.preview.sheet = None;
                                                this.preview.sheet_history.clear();
                                                this.preview_sheet_return_captured_gen = 0;
                                                cx.notify();
                                            } else {
                                                // Submit the form
                                                this.submit_create_bottle(cx);
                                            }
                                        } else if matches!(this.preview.sheet, Some(PreviewSheet::RunSettings(_))) {
                                            if this.demo || this.program_settings.executable.is_none() {
                                                this.preview.sheet = None;
                                                this.preview.sheet_history.clear();
                                                this.preview_sheet_return_captured_gen = 0;
                                                this.restore_preview_sheet_focus(window, cx);
                                                cx.notify();
                                                return;
                                            }
                                            this.save_program_settings(cx);
                                        } else if matches!(this.preview.sheet, Some(PreviewSheet::Winetricks)) {
                                            if this.demo {
                                                this.preview.sheet = None;
                                                this.preview.sheet_history.clear();
                                                this.preview_sheet_return_captured_gen = 0;
                                                this.restore_preview_sheet_focus(window, cx);
                                                cx.notify();
                                                return;
                                            }
                                            if this.winetricks_job().is_some_and(crate::winetricks::InstallJob::running) {
                                                return;
                                            }
                                            if this.winetricks_job().is_some_and(|job| job.finished) {
                                                this.winetricks.job = None;
                                                this.preview.sheet = None;
                                                this.preview.sheet_history.clear();
                                                this.preview_sheet_return_captured_gen = 0;
                                                cx.notify();
                                            } else {
                                                this.start_winetricks_install(cx);
                                            }
                                        } else {
                                            this.preview_dismiss(cx);
                                        }
                                        if this.preview.sheet.is_none() && this.launch_chooser.is_none() {
                                            this.restore_preview_sheet_focus(window, cx);
                                        }
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
    /// Clipboard preflight. The helper refuses to launch until it is told
    /// whether to keep or clear the clipboard, so this sheet sits between the
    /// first and second `runtime.launch` call.
    ///
    /// `LaunchInteractionBroker.resolve` accepts exactly `clear` and `keep`;
    /// both continue the launch. Dismissing sends nothing, so nothing starts.
    pub(crate) fn clipboard_dialog(
        &self,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(prompt) = &self.clipboard_prompt else {
            return div().into_any_element();
        };
        let content = if prompt.content_type.is_empty() {
            crate::i18n::s().dialogs.clipboard_unknown_content.to_owned()
        } else {
            prompt.content_type.clone()
        };
        let detail = crate::i18n::s().dialogs.clipboard_prompt_body(&content, &clipboard_size(prompt.size_bytes));
        let pw = 460_f32.min(width - 48.);
        let ph = 290_f32.min(height - 48.);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(rgba(crate::theme::overlay()))
            .child(
                div()
                    .id("clipboard-dismiss")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_clipboard(cx);
                        this.focus_handle.focus(window, cx);
                    })),
            )
            .child(
                col()
                    .id("clipboard-dialog-card")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .absolute()
                    .left(px((width - pw) / 2.))
                    .top(px((height - ph) / 2.))
                    .w(px(pw))
                    .h(px(ph))
                    .rounded(px(RADIUS_LG))
                    .overflow_hidden()
                    .bg(rgb(crate::theme::surface()))
                    .border_1()
                    .border_color(rgba(crate::theme::surface_border()))
                    .shadow_xl()
                    .child(
                        row()
                            .flex_shrink_0()
                            .h(px(46.))
                            .px(px(16.))
                            .gap(px(12.))
                            .border_b_1()
                            .border_color(rgba(crate::theme::surface_border()))
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(crate::i18n::s().dialogs.clipboard_title),
                            )
                            .child(
                                Button::new("clipboard-close")
                                    .ghost()
                                    .small()
                                    .icon(icon("x", 14.))
                                    .tooltip(crate::i18n::s().common.close_esc)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.dismiss_clipboard(cx);
                                        this.focus_handle.focus(window, cx);
                                    })),
                            ),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_h_0()
                            .px(px(18.))
                            .py(px(14.))
                            .gap(px(10.))
                            .child(muted(detail).line_height(relative(1.5)).line_clamp(3))
                            .when_some(prompt.preview.clone(), |d, preview| {
                                d.child(
                                    div()
                                        .flex_shrink_0()
                                        .p(px(10.))
                                        .rounded(px(8.))
                                        .bg(rgba(crate::theme::card_bg()))
                                        .font_family("monospace")
                                        .text_size(px(11.))
                                        .text_color(rgb(crate::theme::muted_color()))
                                        .line_clamp(4)
                                        .child(preview),
                                )
                            }),
                    )
                    .child(
                        row()
                            .flex_shrink_0()
                            .justify_end()
                            .gap(px(8.))
                            .px(px(16.))
                            .py(px(12.))
                            .border_t_1()
                            .border_color(rgba(crate::theme::surface_border()))
                            .child(Button::new("clipboard-cancel").label(crate::i18n::s().common.cancel).ghost().on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.dismiss_clipboard(cx);
                                    this.focus_handle.focus(window, cx);
                                }),
                            ))
                            .child(
                                Button::new("clipboard-keep")
                                    .label(crate::i18n::s().dialogs.clipboard_keep_launch)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.resolve_clipboard(ClipboardDecision::Keep, cx);
                                        this.focus_handle.focus(window, cx);
                                    })),
                            )
                            .child(AutoFocusButton::new(
                                "clipboard-clear",
                                Button::new("clipboard-clear")
                                    .label(crate::i18n::s().dialogs.clipboard_clear_launch)
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.resolve_clipboard(ClipboardDecision::Clear, cx);
                                        this.focus_handle.focus(window, cx);
                                    })),
                            )),
                    ),
            )
            .into_any_element()
    }

    /// Post-run suggestion: the latest log recorded a rejected display-mode
    /// change, so the dialog offers the program-scoped compatibility switch
    /// directly. "不再建议" writes the permanent dismissal.
    pub(crate) fn legacy_fullscreen_dialog(
        &self,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(prompt) = &self.legacy_fullscreen_prompt else {
            return div().into_any_element();
        };
        let detail = crate::i18n::s().dialogs.legacy_fullscreen_body(&prompt.program_name);
        let pw = 480_f32.min(width - 48.);
        let ph = 300_f32.min(height - 48.);
        let enable = prompt.enable;
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(rgba(crate::theme::overlay()))
            .child(
                div()
                    .id("legacy-fullscreen-dismiss")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_legacy_fullscreen(cx);
                        this.focus_handle.focus(window, cx);
                    })),
            )
            .child(
                col()
                    .id("legacy-fullscreen-dialog-card")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .absolute()
                    .left(px((width - pw) / 2.))
                    .top(px((height - ph) / 2.))
                    .w(px(pw))
                    .h(px(ph))
                    .rounded(px(RADIUS_LG))
                    .overflow_hidden()
                    .bg(rgb(crate::theme::surface()))
                    .border_1()
                    .border_color(rgba(crate::theme::surface_border()))
                    .shadow_xl()
                    .child(
                        row()
                            .flex_shrink_0()
                            .h(px(46.))
                            .px(px(16.))
                            .gap(px(12.))
                            .border_b_1()
                            .border_color(rgba(crate::theme::surface_border()))
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(crate::i18n::s().dialogs.legacy_fs_title),
                            )
                            .child(
                                Button::new("legacy-fullscreen-close")
                                    .ghost()
                                    .small()
                                    .icon(icon("x", 14.))
                                    .tooltip(crate::i18n::s().common.close_esc)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.dismiss_legacy_fullscreen(cx);
                                        this.focus_handle.focus(window, cx);
                                    })),
                            ),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_h_0()
                            .px(px(18.))
                            .py(px(14.))
                            .gap(px(10.))
                            .child(muted(detail).line_height(relative(1.5)).line_clamp(4))
                            .child(self.program_switch(
                                "legacy-fullscreen-switch".into(),
                                crate::i18n::s().dialogs.legacy_fs_label,
                                enable,
                                false,
                                |this, on, _cx| this.legacy_fullscreen_set_enable(on),
                                cx,
                            )),
                    )
                    .child(
                        row()
                            .flex_shrink_0()
                            .justify_end()
                            .gap(px(8.))
                            .px(px(16.))
                            .py(px(12.))
                            .border_t_1()
                            .border_color(rgba(crate::theme::surface_border()))
                            .child(
                                Button::new("legacy-fullscreen-never")
                                    .label(crate::i18n::s().dialogs.legacy_fs_dont_ask)
                                    .ghost()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.legacy_fullscreen_dismiss_forever(cx);
                                        this.focus_handle.focus(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("legacy-fullscreen-apply")
                                    .label(crate::i18n::s().common.apply)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.legacy_fullscreen_apply(false, cx);
                                        this.focus_handle.focus(window, cx);
                                    })),
                            )
                            .child(AutoFocusButton::new(
                                "legacy-fullscreen-restart",
                                Button::new("legacy-fullscreen-restart")
                                    .label(crate::i18n::s().dialogs.legacy_fs_apply_restart)
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.legacy_fullscreen_apply(true, cx);
                                        this.focus_handle.focus(window, cx);
                                    })),
                            )),
                    ),
            )
            .into_any_element()
    }

    /// Submit the create-bottle form.
    fn submit_create_bottle(&mut self, cx: &mut Context<Self>) {
        use crate::create_bottle::windows_version_wire;

        let name = self
            .preview
            .inputs
            .get("bottle-name")
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let name = match crate::create_bottle::validate_name(&name) {
            Ok(name) => name,
            Err(reason) => {
                self.show_notice(reason, cx);
                return;
            }
        };

        let windows_label = self
            .preview
            .choices
            .get("create-windows")
            .cloned()
            .unwrap_or_else(|| "Windows 11".into());
        let windows_version = windows_version_wire(&windows_label);

        let parent = self
            .default_bottle_location
            .clone()
            .unwrap_or_else(|| {
                #[cfg(target_os = "macos")]
                {
                    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
                    format!("{}/Library/Containers/com.daydalek.Maltoria/Bottles", home)
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
                    format!("{}/.local/share/whisky/Bottles", home)
                }
            });

        let engine_id = self.resolved_create_engine_id();
        self.bridge
            .create_bottle_with_engine(name, &parent, windows_version, &engine_id);
        // Keep the sheet open: the Started event switches it to the
        // running view, which is the only in-progress cue the user gets
        // while the prefix bootstrap runs.
        cx.notify();
    }

    /// Sidebar rename/duplicate/remove modal. Rename and duplicate share one
    /// text field; remove offers the two outcomes the SwiftUI alert exposes:
    /// deregister only, or delete the prefix as well.
    pub(crate) fn bottle_dialog_view(
        &mut self,
        width: f32,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(dialog) = self.bottle_dialog.as_ref() else {
            return div().into_any_element();
        };
        let kind = dialog.kind;
        let name = dialog.name.clone();
        let container = dialog.container.clone();
        let d = &crate::i18n::s().dialogs;
        let title = match kind {
            BottleDialogKind::Rename => d.bottle_rename_title,
            BottleDialogKind::Duplicate => d.bottle_duplicate_title,
            BottleDialogKind::Remove => d.bottle_remove_title,
        };
        let body: AnyElement = match kind {
            BottleDialogKind::Rename | BottleDialogKind::Duplicate => {
                let input = self.bottle_dialog_input(window, cx);
                col()
                    .gap(px(12.))
                    .child(muted(if kind == BottleDialogKind::Rename {
                        crate::i18n::s().dialogs.bottle_rename_prompt(&name)
                    } else {
                        crate::i18n::s().dialogs.bottle_duplicate_prompt(&name)
                    }))
                    .child(Input::new(&input))
                    .into_any_element()
            }
            BottleDialogKind::Remove => col()
                .gap(px(10.))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .child(crate::i18n::s().dialogs.bottle_remove_confirm(&name)),
                )
                .child(muted(d.bottle_remove_note))
                .child(muted(container).text_size(px(11.)))
                .into_any_element(),
        };
        let footer = if kind == BottleDialogKind::Remove {
            row()
                .flex_shrink_0()
                .justify_end()
                .gap(px(8.))
                .px(px(18.))
                .py(px(13.))
                .border_t_1()
                .border_color(rgba(crate::theme::surface_border()))
                .child(
                    Button::new("bottle-remove-cancel")
                        .label(crate::i18n::s().common.cancel)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.command(Command::BottleDialogCancel, cx);
                            this.focus_handle.focus(window, cx);
                        })),
                )
                .child(
                    Button::new("bottle-remove-keep")
                        .label(d.bottle_remove_only)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.remove_bottle(false, cx);
                            this.focus_handle.focus(window, cx);
                        })),
                )
                .child(
                    Button::new("bottle-remove-delete")
                        .label(d.bottle_delete_permanently)
                        .danger()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.remove_bottle(true, cx);
                            this.focus_handle.focus(window, cx);
                        })),
                )
        } else {
            let confirm = if kind == BottleDialogKind::Rename {
                d.bottle_rename_btn
            } else {
                d.bottle_duplicate_btn
            };
            row()
                .flex_shrink_0()
                .justify_end()
                .gap(px(8.))
                .px(px(18.))
                .py(px(13.))
                .border_t_1()
                .border_color(rgba(crate::theme::surface_border()))
                .child(
                    Button::new("bottle-dialog-cancel")
                        .label(crate::i18n::s().common.cancel)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.command(Command::BottleDialogCancel, cx);
                            this.focus_handle.focus(window, cx);
                        })),
                )
                .child(
                    Button::new("bottle-dialog-confirm")
                        .label(confirm)
                        .primary()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.command(Command::BottleDialogConfirm, cx);
                            if this.bottle_dialog.is_none() {
                                this.focus_handle.focus(window, cx);
                            }
                        })),
                )
        };
        let pw = 460_f32.min(width - 48.);
        let ph = 260_f32.min(height - 48.);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(rgba(crate::theme::overlay()))
            .child(
                div()
                    .id("bottle-dialog-dismiss")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_bottle_dialog(cx);
                        this.focus_handle.focus(window, cx);
                    })),
            )
            .child(
                col()
                    .id("bottle-dialog-card")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "enter"
                            && this.bottle_dialog.as_ref().is_some_and(|d| d.kind != BottleDialogKind::Remove)
                        {
                            this.command(Command::BottleDialogConfirm, cx);
                            if this.bottle_dialog.is_none() {
                                this.focus_handle.focus(window, cx);
                            }
                        }
                    }))
                    .absolute()
                    .left(px((width - pw) / 2.))
                    .top(px((height - ph) / 2.))
                    .w(px(pw))
                    .h(px(ph))
                    .rounded(px(RADIUS_LG))
                    .overflow_hidden()
                    .bg(rgb(crate::theme::surface()))
                    .border_1()
                    .border_color(rgba(crate::theme::surface_border()))
                    .shadow_xl()
                    .child(
                        row()
                            .flex_shrink_0()
                            .h(px(46.))
                            .px(px(16.))
                            .border_b_1()
                            .border_color(rgba(crate::theme::surface_border()))
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            ),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_h_0()
                            .px(px(20.))
                            .py(px(18.))
                            .children([body]),
                    )
                    .child(footer),
            )
            .into_any_element()
    }

    /// First-run WhiskyWine runtime setup. The helper owns the download and
    /// install job; this surface only reports the stage.
    pub(crate) fn setup_dialog_view(
        &self,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let installed = self
            .setup_status
            .as_ref()
            .is_some_and(|status| status.installed);
        let version = self
            .setup_status
            .as_ref()
            .and_then(|status| status.version.clone());
        let r = &crate::i18n::s().runtime;
        let d = &crate::i18n::s().dialogs;
        let stage = self.setup_stage.as_deref().map(|stage| match stage {
            "checking" => r.checking_launch,
            "downloading" => crate::i18n::s().dialogs.downloading_wine_runtime,
            "verifying" => crate::i18n::s().dialogs.verifying_download,
            "installing" => crate::i18n::s().dialogs.extracting_and_installing_runtime,
            "rosetta" => crate::i18n::s().dialogs.installing_rosetta_2,
            _ => crate::i18n::s().runtime.preparing,
        });
        let mut rows: Vec<AnyElement> = vec![
            muted(d.setup_desc)
                .line_height(relative(1.5))
                .into_any_element(),
        ];
        let apple_silicon = self
            .setup_status
            .as_ref()
            .is_some_and(|status| status.apple_silicon);
        let rosetta_installed = self
            .setup_status
            .as_ref()
            .is_some_and(|status| status.rosetta_installed);
        if apple_silicon {
            if rosetta_installed {
                rows.push(
                    row()
                        .gap(px(8.))
                        .child(icon("check", 14.).text_color(rgb(crate::theme::SUCCESS)))
                        .child(muted(d.setup_rosetta_installed).text_size(px(11.)))
                        .into_any_element(),
                );
            } else {
                rows.push(
                    row()
                        .gap(px(8.))
                        .child(muted(d.setup_rosetta_missing).text_size(px(11.)))
                        .child(
                            Button::new("setup-rosetta")
                                .label(d.setup_install_rosetta)
                                .disabled(self.setup_installing)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.command(Command::InstallRosetta, cx);
                                })),
                        )
                        .into_any_element(),
                );
            }
        }
        if self.setup_installing {
            let progress_text = self.setup_progress.map(|(received, total)| {
                let mib = |bytes: u64| bytes as f64 / 1_048_576.0;
                if total > 0 {
                    format!(
                        "{:.0}%  ·  {:.1} / {:.1} MB",
                        (received as f64 / total as f64 * 100.0).min(100.0),
                        mib(received),
                        mib(total)
                    )
                } else {
                    format!("{:.1} MB", mib(received))
                }
            });
            rows.push(
                row()
                    .gap(px(10.))
                    .py(px(10.))
                    .child(Spinner::new().with_size(px(16.)))
                    .child(stage.unwrap_or(crate::i18n::s().dialogs.installing).to_owned())
                    .into_any_element(),
            );
            if let Some((received, total)) = self.setup_progress.filter(|(_, total)| *total > 0) {
                let fraction = (received as f32 / total as f32).clamp(0., 1.);
                rows.push(
                    div()
                        .h(px(6.))
                        .w_full()
                        .rounded(px(3.))
                        .bg(rgba(crate::theme::hover_strong()))
                        .child(
                            div()
                                .h_full()
                                .w(relative(fraction))
                                .rounded(px(3.))
                                .bg(rgb(crate::theme::SUCCESS)),
                        )
                        .into_any_element(),
                );
            }
            if let Some(text) = progress_text {
                rows.push(muted(text).text_size(px(11.)).into_any_element());
            }
        } else if let Some(error) = &self.setup_error {
            rows.push(muted(error.clone()).into_any_element());
        } else if installed || version.is_some() {
            rows.push(
                row()
                    .gap(px(10.))
                    .py(px(10.))
                    .child(icon("check", 16.).text_color(rgb(crate::theme::SUCCESS)))
                    .child(version.clone().map_or_else(
                        || d.setup_wine_installed.to_owned(),
                        |version| crate::i18n::s().dialogs.setup_ready_version(&version),
                    ))
                    .into_any_element(),
            );
        }
        let footer = row()
            .flex_shrink_0()
            .justify_end()
            .gap(px(8.))
            .px(px(18.))
            .py(px(13.))
            .border_t_1()
            .border_color(rgba(crate::theme::surface_border()))
            .when(
                !self.setup_installing && (installed || version.is_some()),
                |d| {
                    d.child(AutoFocusButton::new(
                        "setup-done",
                        Button::new("setup-done")
                            .label(crate::i18n::s().common.done)
                            .primary()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.command(Command::DismissSetup, cx);
                                this.focus_handle.focus(window, cx);
                            })),
                    ))
                },
            )
            .when(!self.setup_installing && !installed, |btn_row| {
                btn_row.child(
                    Button::new("setup-later")
                        .label(d.setup_later)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.command(Command::DismissSetup, cx);
                            this.focus_handle.focus(window, cx);
                        })),
                )
                .child(AutoFocusButton::new(
                    "setup-install",
                    Button::new("setup-install")
                        .label(if self.setup_error.is_some() {
                            d.setup_retry
                        } else {
                            d.setup_download_install
                        })
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.command(Command::InstallWineRuntime, cx);
                        })),
                ))
            });
        let pw = 480_f32.min(width - 48.);
        let ph = 320_f32.min(height - 48.);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(rgba(crate::theme::overlay()))
            .child(
                col()
                    .id("setup-dialog-card")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .absolute()
                    .left(px((width - pw) / 2.))
                    .top(px((height - ph) / 2.))
                    .w(px(pw))
                    .h(px(ph))
                    .rounded(px(RADIUS_LG))
                    .overflow_hidden()
                    .bg(rgb(crate::theme::surface()))
                    .border_1()
                    .border_color(rgba(crate::theme::surface_border()))
                    .shadow_xl()
                    .child(
                        row()
                            .flex_shrink_0()
                            .h(px(46.))
                            .px(px(16.))
                            .border_b_1()
                            .border_color(rgba(crate::theme::surface_border()))
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(d.setup_title),
                            ),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_h_0()
                            .px(px(20.))
                            .py(px(18.))
                            .gap(px(10.))
                            .children(rows),
                    )
                    .child(footer),
            )
            .into_any_element()
    }

    pub(crate) fn about_dialog(
        &self,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pw = 600_f32.min(width - 48.);
        let ph = 520_f32.min(height - 48.);
        let mut tab_strip = row()
            .flex_shrink_0()
            .flex_wrap()
            .px(px(16.))
            .py(px(10.))
            .gap(px(8.))
            .border_b_1()
            .border_color(rgba(crate::theme::surface_border()));
        for (tab, id, label) in about_tabs() {
            let active = self.about_tab == tab;
            tab_strip = tab_strip.child(about_tab_button(id, label, active, tab, cx));
        }

        let body = match self.about_tab {
            AboutTab::Overview => self.about_overview_tab(cx),
            AboutTab::Acknowledgments => self.about_acknowledgments_tab(),
            AboutTab::License => self.about_document_tab(
                "about-license-scroll",
                crate::i18n::s().dialogs.about_license_header,
                GPL_LICENSE_TEXT,
                false,
            ),
            AboutTab::Notices => self.about_document_tab(
                "about-notices-scroll",
                crate::i18n::s().dialogs.about_notices_header,
                THIRD_PARTY_NOTICES_TEXT,
                true,
            ),
        };

        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(rgba(crate::theme::overlay()))
            .child(
                div()
                    .id("about-dismiss")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_about();
                        this.focus_handle.focus(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                col()
                    .id("about-dialog-card")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .absolute()
                    .left(px((width - pw) / 2.))
                    .top(px((height - ph) / 2.))
                    .w(px(pw))
                    .h(px(ph))
                    .rounded(px(RADIUS_LG))
                    .overflow_hidden()
                    .bg(rgb(crate::theme::surface()))
                    .border_1()
                    .border_color(rgba(crate::theme::surface_border()))
                    .shadow_xl()
                    .child(
                        row()
                            .flex_shrink_0()
                            .h(px(46.))
                            .px(px(16.))
                            .gap(px(12.))
                            .border_b_1()
                            .border_color(rgba(crate::theme::surface_border()))
                            .child(
                                div()
                                    .flex_1()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(crate::i18n::s().dialogs.about_title),
                            )
                            .child(
                                Button::new("about-close")
                                    .ghost()
                                    .small()
                                    .icon(icon("x", 14.))
                                    .tooltip(crate::i18n::s().common.close_esc)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.dismiss_about();
                                        this.focus_handle.focus(window, cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(tab_strip)
                    .child(body)
                    .child(
                        row()
                            .flex_shrink_0()
                            .justify_between()
                            .items_center()
                            .gap(px(12.))
                            .px(px(18.))
                            .py(px(13.))
                            .border_t_1()
                            .border_color(rgba(crate::theme::surface_border()))
                            .child(muted(crate::i18n::s().dialogs.about_gpl_badge).text_size(px(11.)))
                            .child(AutoFocusButton::new(
                                "about-done",
                                Button::new("about-done")
                                    .label(crate::i18n::s().common.close)
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.dismiss_about();
                                        this.focus_handle.focus(window, cx);
                                        cx.notify();
                                    })),
                            )),
                    ),
            )
            .into_any_element()
    }

    fn about_overview_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let name = self
            .app_info
            .as_ref()
            .map(|info| info.app_name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Maltoria".into());
        let version = self
            .app_info
            .as_ref()
            .map_or_else(|| crate::i18n::s().dialogs.preview_badge.to_owned(), |info| info.version_label());
        let os_version = self
            .app_info
            .as_ref()
            .map(|info| info.os_version.clone())
            .filter(|version| !version.is_empty())
            .unwrap_or_else(|| {
                if cfg!(target_os = "macos") {
                    "macOS".into()
                } else {
                    "Linux".into()
                }
            });
        let bundle = self
            .app_info
            .as_ref()
            .map(|info| info.bundle_identifier.clone())
            .unwrap_or_default();
        let d = &crate::i18n::s().dialogs;
        let bridge = match &self.bridge_status {
            BridgeStatus::Connected { version, .. } => version.clone(),
            BridgeStatus::Failed { .. } => crate::i18n::s().common.unavailable.into(),
            BridgeStatus::Stopped => d.bridge_stopped.into(),
            BridgeStatus::Locating | BridgeStatus::Starting => d.bridge_connecting.into(),
        };
        let subtitle = if cfg!(target_os = "macos") {
            d.about_tagline_mac
        } else {
            d.about_tagline_linux
        };

        let mut facts = vec![
            about_fact_row(d.about_version, version),
            about_fact_row(d.about_system, os_version),
            about_fact_row(d.about_bridge, bridge),
        ];
        if !bundle.is_empty() {
            facts.push(about_fact_row(d.about_bundle_id, bundle));
        }

        let legal_card = col()
            .w_full()
            .rounded(px(10.))
            .p(px(14.))
            .gap(px(8.))
            .bg(rgba(crate::theme::section_bg()))
            .border_1()
            .border_color(rgba(crate::theme::section_border()))
            .child(
                row()
                    .justify_between()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(crate::theme::text()))
                            .child(d.about_gpl_heading),
                    )
                    .child(about_pill_badge("GPL-3.0-or-later", true)),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(crate::theme::subtext()))
                    .child("Copyright © 2023–2026 Whisky & Maltoria Contributors"),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .text_size(px(11.5))
                    .line_height(relative(1.5))
                    .text_color(rgb(crate::theme::muted_color()))
                    .child(d.about_gpl_body),
            )
            .child(
                row()
                    .flex_wrap()
                    .pt(px(2.))
                    .gap(px(8.))
                    .child(
                        Button::new("about-jump-license")
                            .label(d.about_view_license)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.about_tab = AboutTab::License;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("about-jump-ack")
                            .label(d.about_view_ack)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.about_tab = AboutTab::Acknowledgments;
                                cx.notify();
                            })),
                    ),
            );

        self.flex_scroll(
            "about-overview-scroll",
            col()
                .w_full()
                .px(px(20.))
                .py(px(18.))
                .gap(px(14.))
                .child(
                    col()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_size(px(22.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(crate::theme::text()))
                                .child(name),
                        )
                        .child(muted(subtitle)),
                )
                .child(
                    col()
                        .w_full()
                        .rounded(px(10.))
                        .px(px(14.))
                        .py(px(12.))
                        .gap(px(8.))
                        .bg(rgba(crate::theme::section_bg()))
                        .border_1()
                        .border_color(rgba(crate::theme::section_border()))
                        .children(facts),
                )
                .child(legal_card),
        )
    }

    pub(crate) fn bridge_status_dialog(
        &self,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = &crate::i18n::s().dialogs;
        let (title, detail) = match &self.bridge_status {
            BridgeStatus::Failed { reason, retryable } => (
                d.bridge_unavailable,
                if *retryable {
                    reason.clone()
                } else {
                    crate::i18n::s().dialogs.bridge_failed_detail(reason)
                },
            ),
            BridgeStatus::Connected {
                helper,
                version,
                methods,
            } => (
                d.bridge_connected,
                crate::i18n::s().dialogs.bridge_connected_detail(helper, *methods, version),
            ),
            BridgeStatus::Stopped => (d.bridge_stopped, d.bridge_stopped_detail.into()),
            BridgeStatus::Locating | BridgeStatus::Starting => (
                d.bridge_connecting,
                d.bridge_connecting_detail.into(),
            ),
        };
        let (title, detail) = self.quit_error.as_ref()
            .map(|error| (d.bridge_quit_incomplete, error.clone())).unwrap_or((title, detail));
        let retry = matches!(self.bridge_status, BridgeStatus::Failed { .. });
        let pw = 440_f32.min(width - 48.);
        let ph = 210_f32.min(height - 48.);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(rgba(crate::theme::overlay()))
            .child(
                div()
                    .id("bridge-dialog-dismiss")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.bridge_dialog = false;
                        this.focus_handle.focus(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                col()
                    .id("bridge-dialog-card")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .absolute()
                    .left(px((width - pw) / 2.))
                    .top(px((height - ph) / 2.))
                    .w(px(pw))
                    .h(px(ph))
                    .rounded(px(RADIUS_LG))
                    .overflow_hidden()
                    .bg(rgb(crate::theme::surface()))
                    .border_1()
                    .border_color(rgba(crate::theme::surface_border()))
                    .shadow_xl()
                    .child(
                        row()
                            .flex_shrink_0()
                            .h(px(46.))
                            .px(px(16.))
                            .gap(px(12.))
                            .border_b_1()
                            .border_color(rgba(crate::theme::surface_border()))
                            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(title))
                            .child(
                                Button::new("bridge-dialog-close")
                                    .ghost()
                                    .small()
                                    .icon(icon("x", 14.))
                                    .tooltip(crate::i18n::s().common.close_esc)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.bridge_dialog = false;
                                        this.focus_handle.focus(window, cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_h_0()
                            .px(px(18.))
                            .py(px(14.))
                            .gap(px(10.))
                            .child(
                                muted(detail)
                                    .line_height(relative(1.5))
                                    .line_clamp(4),
                            ),
                    )
                    .child(
                        row()
                            .flex_shrink_0()
                            .justify_end()
                            .gap(px(8.))
                            .px(px(16.))
                            .py(px(12.))
                            .border_t_1()
                            .border_color(rgba(crate::theme::surface_border()))
                            .child(AutoFocusButton::new(
                                "bridge-dialog-done",
                                Button::new("bridge-dialog-done")
                                    .label(crate::i18n::s().common.close)
                                    .ghost()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.bridge_dialog = false;
                                        this.focus_handle.focus(window, cx);
                                        cx.notify();
                                    })),
                            ).when_focusable(!retry))
                            .when(retry, |d| {
                                d.child(AutoFocusButton::new(
                                    "bridge-retry",
                                    Button::new("bridge-retry")
                                        .label(crate::i18n::s().common.retry)
                                        .primary()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.bridge_retry(cx);
                                        })),
                                ))
                            })
                            .when(self.quit_error.is_some(), |row| {
                                row.child(Button::new("quit-without-cleanup")
                                    .label(d.force_quit_app)
                                    .on_click(|_, _, cx| cx.quit()))
                            }),
                    ),
            )
            .into_any_element()
    }
}

impl WhiskyApp {
    fn metadata_manual_rows(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(page) = self.game_info.as_ref() else {
            return Vec::new();
        };
        if !page.editor.loaded {
            return vec![muted(crate::i18n::s().dialogs.loading_local_metadata_config).text_size(px(11.)).into_any_element()];
        }
        let editable = !page.editor.busy;
        let title = self.metadata_editor_input("manualTitle", window, cx);
        let summary = self.metadata_editor_input("manualSummary", window, cx);
        let Some((title_enabled, summary_enabled)) = self.game_info.as_ref().map(|page| {
            (
                page.editor.manual_enabled("manualTitle"),
                page.editor.manual_enabled("manualSummary"),
            )
        }) else {
            return Vec::new();
        };

        vec![
            col()
                .gap(px(8.))
                .py(px(4.))
                .child(
                    Switch::new("metadata-manual-title")
                        .label(crate::i18n::s().dialogs.manually_edit_title)
                        .checked(title_enabled)
                        .disabled(!editable)
                        .on_change(cx.listener(|this, enabled, _, cx| {
                            this.toggle_metadata_manual("manualTitle", *enabled, cx)
                        })),
                )
                .when(title_enabled, |d| d.child(Input::new(&title).disabled(!editable)))
                .child(
                    Switch::new("metadata-manual-summary")
                        .label(crate::i18n::s().dialogs.manually_edit_summary)
                        .checked(summary_enabled)
                        .disabled(!editable)
                        .on_change(cx.listener(|this, enabled, _, cx| {
                            this.toggle_metadata_manual("manualSummary", *enabled, cx)
                        })),
                )
                .when(summary_enabled, |d| d.child(Input::new(&summary).disabled(!editable)))
                .child(
                    muted(crate::i18n::s().dialogs.manual_override_tip)
                        .text_size(px(11.))
                        .line_height(relative(1.4)),
                )
                .into_any_element(),
        ]
    }

    fn metadata_policy_and_character_rows(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let Some(page) = self.game_info.as_ref() else {
            return Vec::new();
        };
        let snapshots = &page.editor.record["snapshots"];
        let mut rows = vec![
            muted(crate::i18n::s().dialogs.default_source_tip)
                .text_size(px(11.))
                .line_height(relative(1.4))
                .into_any_element(),
            self.metadata_policy_row("default", crate::i18n::s().onboarding.default_source, false, cx),
        ];

        let category_cell = |category: &'static str, label: &'static str| -> AnyElement {
            if category == "characters"
                && !crate::metadata_editor::PROVIDERS.iter().any(|source| {
                    page.editor.capabilities[*source]["characters"].as_bool() == Some(true)
                })
            {
                row()
                    .gap(px(8.))
                    .py(px(4.))
                    .child(div().flex_1().text_size(px(12.)).child(label))
                    .child(muted(crate::i18n::s().common.unavailable).text_size(px(11.)))
                    .into_any_element()
            } else {
                self.metadata_policy_row(category, label, true, cx)
            }
        };

        let grid_rows: Vec<AnyElement> = crate::metadata_editor::categories()
            .chunks(2)
            .map(|pair| {
                let mut r = row().gap(px(16.));
                for &(category, label) in pair {
                    r = r.child(div().flex_1().min_w_0().child(category_cell(category, label)));
                }
                r.into_any_element()
            })
            .collect();

        rows.push(
            col()
                .gap(px(6.))
                .p(px(10.))
                .my(px(4.))
                .rounded(px(8.))
                .bg(rgba(crate::theme::card_bg_subtle()))
                .border_1()
                .border_color(rgba(crate::theme::card_border()))
                .child(
                    muted(crate::i18n::s().dialogs.category_overrides_follow_default_above)
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .children(grid_rows)
                .into_any_element(),
        );

        let character_source = page.editor.effective_source("characters");
        if page.editor.capabilities[character_source]["characters"].as_bool() == Some(true) {
            let characters = snapshots[character_source]["characters"].as_array();
            let count = characters.map_or(0, Vec::len);
            let expanded = page.editor.characters_expanded;

            let mut header = row()
                .justify_between()
                .gap(px(8.))
                .pt(px(6.))
                .child(
                    row()
                        .gap(px(6.))
                        .child(
                            muted(crate::i18n::s().dialogs.characters(source_label(character_source)))
                                .text_size(px(11.5))
                                .font_weight(FontWeight::SEMIBOLD),
                        )
                        .when(count > 0, |d| {
                            d.child(info_pill(
                                crate::i18n::s().dialogs.character_count(count),
                                crate::theme::badge_bg(),
                                crate::theme::badge_text(),
                            ))
                        }),
                );
            if count > 3 {
                header = header.child(
                    Button::new("metadata-character-disclosure")
                        .ghost()
                        .label(if expanded {
                            crate::i18n::s().common.collapse.to_owned()
                        } else {
                            crate::i18n::s().dialogs.character_view_all(count)
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(page) = this.game_info.as_mut() {
                                page.editor.characters_expanded = !page.editor.characters_expanded;
                            }
                            cx.notify();
                        })),
                );
            }
            rows.push(header.into_any_element());

            match characters {
                Some(characters) if !characters.is_empty() => {
                    let limit = if expanded { characters.len() } else { 3 };
                    let char_items: Vec<AnyElement> = characters
                        .iter()
                        .take(limit)
                        .map(|character| {
                            let name = character["name"].as_str().unwrap_or(crate::i18n::s().dialogs.unnamed);
                            let original = character["originalName"].as_str().unwrap_or("");
                            let role = character["role"].as_str().unwrap_or("");
                            row()
                                .justify_between()
                                .gap(px(8.))
                                .child(
                                    row()
                                        .gap(px(6.))
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(rgb(crate::theme::text()))
                                                .truncate()
                                                .child(name.to_owned()),
                                        )
                                        .when(!original.is_empty() && original != name, |d| {
                                            d.child(
                                                muted(original.to_owned())
                                                    .text_size(px(11.))
                                                    .truncate(),
                                            )
                                        }),
                                )
                                .when(!role.is_empty(), |d| {
                                    d.child(info_pill(
                                        role.to_owned(),
                                        crate::theme::badge_bg(),
                                        crate::theme::badge_text(),
                                    ))
                                })
                                .into_any_element()
                        })
                        .collect();
                    rows.push(
                        col()
                            .gap(px(6.))
                            .p(px(10.))
                            .rounded(px(8.))
                            .bg(rgba(crate::theme::card_bg_subtle()))
                            .border_1()
                            .border_color(rgba(crate::theme::card_border()))
                            .children(char_items)
                            .into_any_element(),
                    );
                }
                Some(_) => {
                    rows.push(
                        muted(crate::i18n::s().dialogs.no_character_data)
                            .text_size(px(11.))
                            .into_any_element(),
                    );
                }
                None => {
                    rows.push(
                        muted(crate::i18n::s().dialogs.no_character_snapshot)
                            .text_size(px(11.))
                            .into_any_element(),
                    );
                }
            }
        } else if page.editor.loaded {
            rows.push(
                muted({
                    let src = source_label(character_source);
                    crate::i18n::s().dialogs.source_no_characters(src)
                })
                .text_size(px(11.))
                .into_any_element(),
            );
        }

        if self.metadata_editor_dirty(cx) {
            rows.push(
                muted(crate::i18n::s().dialogs.unsaved_before_refresh)
                    .text_size(px(11.))
                    .into_any_element(),
            );
        }
        rows
    }

    fn metadata_policy_row(&self, category: &'static str, label: &'static str, inherit: bool, cx: &Context<Self>) -> AnyElement {
        let Some(page) = self.game_info.as_ref() else { return div().into_any_element(); };
        let current = page.editor.source(category);
        let display = if current.is_empty() { crate::i18n::s().onboarding.follow_default } else { source_label(current) };
        let capabilities = page.editor.capabilities.clone();
        let allowed = page.editor.policy_sources(self.metadata_sources());
        let view = cx.entity().downgrade();
        row().gap(px(8.)).py(px(3.))
            .child(div().flex_1().min_w_0().text_size(px(12.5)).child(label))
            .child(Button::new(format!("metadata-policy-{category}"))
                .label(format!("{display}  ⌄"))
                .disabled(!page.editor.loaded || page.editor.busy)
                .dropdown_menu(move |mut menu, _, _| {
                    menu = menu.scrollable(true);
                    if inherit {
                        let view = view.clone();
                        menu = menu.item(PopupMenuItem::new(crate::i18n::s().onboarding.follow_default).on_click(move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| this.set_metadata_source(category, "", cx));
                        }));
                    }
                    for &source in &crate::metadata_editor::PROVIDERS {
                        let label = crate::onboarding::source_label(source);
                        if !allowed.iter().any(|enabled| enabled == source) { continue; }
                        if category == "characters" && capabilities[source]["characters"].as_bool() != Some(true) {
                            continue;
                        }
                        let view = view.clone();
                        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| this.set_metadata_source(category, source, cx));
                        }));
                    }
                    menu
                }))
            .into_any_element()
    }

    /// Per-entry searches are deliberately independent from global sync flags.
    fn metadata_source_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(page) = self.game_info.as_ref() else { return div().into_any_element(); };
        let allowed = self.metadata_sources();
        let mut line = row().gap(px(6.)).py(px(4.)).items_center().flex_wrap().child(
            muted(crate::i18n::s().dialogs.search_sources).text_size(px(11.5)).mr(px(4.)),
        );
        for &source in &crate::metadata_editor::PROVIDERS {
            let label = crate::onboarding::source_label(source);
            let active = page.editor.search_source == source;
            let disabled = page.searching
                || page.editor.busy
                || !page.editor.loaded
                || !allowed.iter().any(|enabled| enabled == source);
            line = line.child(selectable_pill(
                format!("metadata-search-source-{source}"),
                label,
                active,
                disabled,
                cx.listener(move |this, _, _, cx| this.set_metadata_search_source(source, cx)),
            ));
        }
        line.into_any_element()
    }
}

fn info_pill(label: impl Into<SharedString>, bg_rgba: u32, fg_rgb: u32) -> AnyElement {
    row()
        .flex_shrink_0()
        .px(px(8.))
        .py(px(2.))
        .rounded_full()
        .bg(rgba(bg_rgba))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(fg_rgb))
        .child(label.into())
        .into_any_element()
}

fn source_label(source: &str) -> &str {
    match source {
        "bangumi" => "Bangumi",
        "vndb" => "VNDB",
        "steam" => "Steam",
        "ymgal" => crate::i18n::s().metadata.ymgal_name,
        "nextmoe" => "NextMoe",
        "hikarinagi" => "Hikarinagi",
        _ => source,
    }
}

fn compat_label(rating: &str) -> &str {
    match rating {
        "unverified" => crate::i18n::s().dialogs.unverified,
        "verified" => crate::i18n::s().dialogs.verified,
        "playable" => crate::i18n::s().dialogs.playable,
        "runs" => crate::i18n::s().dialogs.runs,
        "unplayable" => crate::i18n::s().dialogs.unplayable,
        _ => rating,
    }
}

/// Bytes as the sheet shows them. The helper reports the raw byte count.
fn clipboard_size(bytes: i64) -> String {
    if bytes <= 0 {
        return crate::i18n::s().dialogs.unknown_size.to_owned();
    }
    if bytes < 1024 {
        return crate::i18n::s().common.bytes_human(bytes as u64);
    }
    format!("{:.1} KB", bytes as f64 / 1024.)
}


impl WhiskyApp {
    /// Create-bottle Wine engine picker. Values stored in
    /// `preview.choices["create-engine"]` are engine ids; the row shows
    /// names. Hidden when the helper predates `wine.engines`.
    fn create_engine_choice(&self, cx: &Context<Self>) -> AnyElement {
        if !self.service_available("wine.engines.list") || self.wine_engines.is_empty() {
            return div().into_any_element();
        }
        let selected = self.resolved_create_engine_id();
        let label = self
            .wine_engines
            .iter()
            .find(|engine| engine.id == selected)
            .map(|engine| engine.name.clone())
            .unwrap_or_else(|| "Whisky Wine".to_owned());
        let view = cx.entity().downgrade();
        let engines = self.wine_engines.clone();
        row()
            .py(px(10.))
            .gap(px(16.))
            .child(div().flex_1().min_w_0().child(crate::i18n::s().bottle.wine_engine_section))
            .child(
                Button::new("create-engine")
                    .label(format!("{label}  ⌄"))
                    .dropdown_menu(move |mut menu, _, _| {
                        menu = menu.scrollable(true);
                        for engine in &engines {
                            let view = view.clone();
                            let engine_id = engine.id.clone();
                            menu = menu.item(
                                PopupMenuItem::new(engine.name.clone())
                                    .disabled(!engine.available)
                                    .checked(selected == engine.id)
                                    .on_click(move |_, _, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            this.preview
                                                .choices
                                                .insert("create-engine", engine_id.clone().into());
                                            cx.notify();
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            )
            .into_any_element()
    }

    /// The Wine-engine confirmation overlay: approve runs the queued
    /// `wine.engines.select` / `setFeatureOverride`, cancel discards it.
    /// Same presentation as the bottle dialog so it reads as one family.
    pub(crate) fn engine_dialog_view(
        &mut self,
        width: f32,
        height: f32,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(dialog) = self.engine_dialog.as_ref() else {
            return div().into_any_element();
        };
        let d = &crate::i18n::s().dialogs;
        let (title, body): (&str, AnyElement) = match &dialog.kind {
            crate::state::EngineDialogKind::Feature { feature, reason, .. } => (
                d.engine_confirm_feature_title,
                col()
                    .gap(px(10.))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(crate::i18n::s().dialogs.engine_feature_confirm_question(feature)),
                    )
                    .child(muted(reason.clone()))
                    .child(
                        muted(d.engine_confirm_feature_note)
                            .line_height(relative(1.4)),
                    )
                    .into_any_element(),
            ),
            crate::state::EngineDialogKind::Select { engine_name } => (
                d.engine_switch_title,
                col()
                    .gap(px(10.))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(crate::i18n::s().dialogs.engine_switch_question(engine_name)),
                    )
                    .child(
                        muted(d.engine_switch_note)
                            .line_height(relative(1.4)),
                    )
                    .into_any_element(),
            ),
        };
        let footer = row()
            .flex_shrink_0()
            .justify_end()
            .gap(px(8.))
            .px(px(18.))
            .py(px(13.))
            .border_t_1()
            .border_color(rgba(crate::theme::surface_border()))
            .child(
                Button::new("engine-dialog-cancel")
                    .label(crate::i18n::s().common.cancel)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_engine_dialog(cx);
                        this.focus_handle.focus(window, cx);
                    })),
            )
            .child(AutoFocusButton::new(
                "engine-dialog-confirm",
                Button::new("engine-dialog-confirm")
                    .label(crate::i18n::s().common.confirm)
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.confirm_engine_dialog(cx);
                        this.focus_handle.focus(window, cx);
                    })),
            ));
        let pw = 460_f32.min(width - 48.);
        let ph = 280_f32.min(height - 48.);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(rgba(crate::theme::overlay()))
            .child(
                div()
                    .id("engine-dialog-dismiss")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.dismiss_engine_dialog(cx);
                        this.focus_handle.focus(window, cx);
                    })),
            )
            .child(
                col()
                    .id("engine-dialog-card")
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .absolute()
                    .left(px((width - pw) / 2.))
                    .top(px((height - ph) / 2.))
                    .w(px(pw))
                    .rounded(px(RADIUS_LG))
                    .overflow_hidden()
                    .bg(rgb(crate::theme::surface()))
                    .border_1()
                    .border_color(rgba(crate::theme::surface_border()))
                    .child(
                        col()
                            .flex_1()
                            .min_h_0()
                            .px(px(18.))
                            .py(px(16.))
                            .gap(px(10.))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                            .child(body),
                    )
                    .child(footer),
            )
            .into_any_element()
    }

    fn launch_entry_section(
        &self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let game = self.game(index)?;
        if !game.supports_multi_executable() {
            return None;
        }
        let multi_exe = game.multi_executable;
        let executables = game.launch_executables.clone();
        let last_executable = game.last_executable.clone();
        let folder = game
            .game_directory()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let current_target = self.program_settings.executable.clone();
        let primary_exe = game
            .executable
            .as_ref()
            .filter(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            })
            .map(|path| path.to_string_lossy().into_owned());

        let mut rows: Vec<AnyElement> = Vec::new();
        rows.push(
            row()
                .py(px(10.))
                .gap(px(16.))
                .items_center()
                .child(
                    col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(3.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(crate::theme::text()))
                                .child(crate::i18n::s().program_settings.multi_exe_title),
                        )
                        .child(
                            muted(crate::i18n::s().program_settings.multi_exe_desc)
                                .text_size(px(11.5))
                                .line_height(relative(1.4)),
                        ),
                )
                .child(
                    Switch::new("run-settings-multi-exe")
                        .checked(multi_exe)
                        .on_change(cx.listener(move |this, checked, _, cx| {
                            this.set_multi_executable(index, *checked, cx);
                        })),
                )
                .into_any_element(),
        );

        if multi_exe {
            let status = if executables.is_empty() {
                crate::i18n::s().program_settings.multi_exe_none_selected.to_owned()
            } else {
                let n = executables.len();
                crate::i18n::s().dialogs.launch_entries_prompt(n)
            };
            rows.push(
                row()
                    .py(px(8.))
                    .gap(px(10.))
                    .items_center()
                    .border_t_1()
                    .border_color(rgba(crate::theme::divider()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(rgb(crate::theme::subtext()))
                            .child(status),
                    )
                    .child(
                        Button::new("run-settings-manage-exes")
                            .label(if executables.is_empty() {
                                crate::i18n::s().program_settings.multi_exe_pick_ellipsis
                            } else {
                                crate::i18n::s().program_settings.multi_exe_manage_ellipsis
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_multi_executable_picker(index, false, true, false, cx);
                            })),
                    )
                    .into_any_element(),
            );

            if !executables.is_empty() {
                let is_dirty = self.program_settings_is_dirty(cx);
                let active_idx = current_target
                    .as_deref()
                    .and_then(|target| executables.iter().position(|exe| exe == target))
                    .unwrap_or(0);
                let entry_handles: Vec<FocusHandle> = (0..executables.len())
                    .map(|ei| {
                        window
                            .use_keyed_state(("run-settings-exe", ei), cx, |_, cx| {
                                cx.focus_handle().tab_stop(true).tab_index(0)
                            })
                            .read(cx)
                            .clone()
                    })
                    .collect();
                let auto_focused = window.use_keyed_state(
                    ("run-settings-autofocus", self.preview_sheet_generation),
                    cx,
                    |_, _| std::cell::Cell::new(false),
                );
                if !auto_focused.read(cx).replace(true) {
                    if let Some(handle) = entry_handles.get(active_idx) {
                        window.focus(handle, cx);
                    }
                }
                rows.push(
                    muted(crate::i18n::s().program_settings.multi_exe_edit_hint)
                        .text_size(px(11.5))
                        .pt(px(2.))
                        .into_any_element(),
                );
                let nav_handles = entry_handles.clone();
                let mut entries_list = col()
                    .id("run-settings-exe-list")
                    .w_full()
                    .gap(px(6.))
                    .pb(px(8.))
                    .on_key_down(cx.listener(move |_, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.modifiers.modified() || nav_handles.is_empty() {
                            return;
                        }
                        let n = nav_handles.len();
                        let current = nav_handles.iter().position(|h| h.is_focused(window));
                        match event.keystroke.key.as_str() {
                            "down" => {
                                let next = current.map_or(0, |i| (i + 1) % n);
                                nav_handles[next].focus(window, cx);
                                cx.stop_propagation();
                            }
                            "up" => {
                                let prev = current.map_or(n - 1, |i| (i + n - 1) % n);
                                nav_handles[prev].focus(window, cx);
                                cx.stop_propagation();
                            }
                            "home" => {
                                nav_handles[0].focus(window, cx);
                                cx.stop_propagation();
                            }
                            "end" => {
                                nav_handles[n - 1].focus(window, cx);
                                cx.stop_propagation();
                            }
                            _ => {}
                        }
                    }));
                for (ei, exe) in executables.iter().enumerate() {
                    let name = std::path::Path::new(exe)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| exe.clone());
                    let (rel_dir, _) =
                        crate::bridge::dto::relative_directory_label(exe, &folder);
                    let is_last = last_executable.as_deref() == Some(exe.as_str());
                    let is_active = current_target.as_deref() == Some(exe.as_str());
                    let exe_path = exe.clone();
                    entries_list = entries_list.child(
                        row()
                            .id(("run-settings-exe", ei))
                            .track_focus(&entry_handles[ei])
                            .w_full()
                            .px(px(10.))
                            .py(px(7.))
                            .gap(px(8.))
                            .items_center()
                            .rounded(px(8.))
                            .bg(rgba(if is_active {
                                crate::theme::accent_subtle_bg()
                            } else {
                                crate::theme::card_bg_subtle()
                            }))
                            .border_1()
                            .border_color(rgba(if is_active {
                                crate::theme::accent_subtle_border()
                            } else {
                                crate::theme::card_border()
                            }))
                            .focus(|s| s.border_2().border_color(rgb(crate::theme::ACCENT)))
                            .cursor_pointer()
                            .hover(|s| {
                                s.bg(rgba(if is_active {
                                    crate::theme::accent_subtle_hover()
                                } else {
                                    crate::theme::hover()
                                }))
                                .border_color(rgba(if is_active {
                                    crate::theme::accent_subtle_border()
                                } else {
                                    crate::theme::hover_strong()
                                }))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_run_settings_executable(index, exe_path.clone(), cx);
                            }))
                            .child(
                                icon(
                                    if is_active {
                                        "sliders-horizontal"
                                    } else {
                                        "play"
                                    },
                                    12.,
                                )
                                .text_color(rgb(if is_active || is_last {
                                    crate::theme::accent_fg()
                                } else {
                                    crate::theme::muted_color()
                                })),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(if is_active {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .text_color(rgb(if is_active {
                                        crate::theme::accent_fg()
                                    } else {
                                        crate::theme::text()
                                    }))
                                    .child(name),
                            )
                            .child(
                                muted(rel_dir)
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(11.)),
                            )
                            .when(is_active, |d| {
                                d.child(
                                    div()
                                        .px(px(6.))
                                        .py(px(1.))
                                        .rounded(px(4.))
                                        .bg(rgba(crate::theme::accent_subtle_hover()))
                                        .text_size(px(10.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(crate::theme::accent_fg()))
                                        .child(crate::i18n::s().program_settings.editing_badge),
                                )
                            })
                            .when(is_active && is_dirty, |d| {
                                d.child(
                                    div()
                                        .px(px(6.))
                                        .py(px(1.))
                                        .rounded(px(4.))
                                        .bg(rgba(0xf5a62328))
                                        .text_size(px(10.))
                                        .text_color(rgb(crate::theme::warning_fg()))
                                        .child(crate::i18n::s().program_settings.unsaved_badge),
                                )
                            })
                            .when(is_last, |d| {
                                d.child(
                                    div()
                                        .px(px(6.))
                                        .py(px(1.))
                                        .rounded(px(4.))
                                        .bg(rgba(crate::theme::accent_subtle_bg()))
                                        .text_size(px(10.))
                                        .text_color(rgb(crate::theme::accent_fg()))
                                        .child(crate::i18n::s().dialogs.last_launched),
                                )
                            })
                            .when(!is_active, |d| {
                                d.child(muted(crate::i18n::s().program_settings.switch_config_chevron).text_size(px(11.)))
                            }),
                    );
                }
                rows.push(entries_list.into_any_element());
            }
        } else if game.is_folder {
            let label = primary_exe
                .as_deref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| crate::i18n::s().program_settings.no_launch_program.to_owned());
            rows.push(
                row()
                    .py(px(8.))
                    .gap(px(10.))
                    .items_center()
                    .border_t_1()
                    .border_color(rgba(crate::theme::divider()))
                    .child(
                        muted(crate::i18n::s().dialogs.current_launch_program(label))
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.)),
                    )
                    .child(
                        Button::new("run-settings-single-exe")
                            .label(crate::i18n::s().library.select_launch_exe_ellipsis)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_executable_picker(index, false, cx);
                            })),
                    )
                    .into_any_element(),
            );
        }

        Some(self.section("program-launch-entries", crate::i18n::s().program_settings.launch_entries_section, rows, cx))
    }

    pub(crate) fn launch_chooser_dialog(
        &mut self,
        width: f32,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(chooser) = self.launch_chooser.as_ref().cloned() else {
            return div().into_any_element();
        };
        let pw = 500_f32.min(width - 48.);
        let ph = (210. + chooser.executables.len() as f32 * 58.)
            .clamp(270., 520.)
            .min(height - 48.);
        let artwork = self.artwork(chooser.game_index);
        let (ph_start, ph_end) = crate::theme::placeholder_gradient();
        let thumb = match artwork {
            Some(path) => self
                .artwork_image(path)
                .w(px(36.))
                .h(px(48.))
                .flex_shrink_0()
                .rounded(px(6.))
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => div()
                .w(px(36.))
                .h(px(48.))
                .flex_shrink_0()
                .rounded(px(6.))
                .bg(gradient(ph_start, ph_end))
                .flex()
                .items_center()
                .justify_center()
                .child(icon("package", 16.).text_color(rgb(crate::theme::muted_color())))
                .into_any_element(),
        };

        let list_scroll = window
            .use_keyed_state("launch-chooser-scroll", cx, |_, _| ScrollHandle::new())
            .read(cx)
            .clone();
        let entry_handles: Vec<FocusHandle> = (0..chooser.executables.len())
            .map(|i| {
                window
                    .use_keyed_state(("launch-chooser-entry", i), cx, |_, cx| {
                        cx.focus_handle().tab_stop(true).tab_index(0)
                    })
                    .read(cx)
                    .clone()
            })
            .collect();
        let settings_handles: Vec<FocusHandle> = (0..chooser.executables.len())
            .map(|i| {
                window
                    .use_keyed_state(("launch-chooser-settings", i), cx, |_, cx| {
                        cx.focus_handle().tab_stop(true).tab_index(0)
                    })
                    .read(cx)
                    .clone()
            })
            .collect();
        let default_idx = chooser
            .last_executable
            .as_deref()
            .and_then(|last| chooser.executables.iter().position(|e| e == last))
            .unwrap_or(0);
        let auto_focused = window.use_keyed_state(
            ("launch-chooser-autofocus", self.launch_chooser_generation),
            cx,
            |_, _| std::cell::Cell::new(false),
        );
        if !auto_focused.read(cx).replace(true) {
            self.launch_chooser_return_focus = window.focused(cx);
            if let Some(handle) = entry_handles.get(default_idx) {
                window.focus(handle, cx);
                list_scroll.scroll_to_item(default_idx);
            }
        }

        let mut list = col()
            .id("launch-chooser-list")
            .track_scroll(&list_scroll)
            .size_full()
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .px(px(18.))
            .py(px(12.))
            .gap(px(8.));
        for (i, exe_path) in chooser.executables.iter().enumerate() {
            let name = std::path::Path::new(exe_path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| exe_path.clone());
            let (location, _) =
                crate::bridge::dto::relative_directory_label(exe_path, &chooser.folder);
            let is_last = chooser.last_executable.as_deref() == Some(exe_path.as_str());
            let path_click = exe_path.clone();
            let path_settings = exe_path.clone();
            let game_index = chooser.game_index;
            list = list.child(
                row()
                    .w_full()
                    .flex_shrink_0()
                    .gap(px(6.))
                    .items_center()
                    .child(
                        row()
                            .id(("launch-chooser-entry", i))
                            .track_focus(&entry_handles[i])
                            .flex_1()
                            .min_w_0()
                            .p(px(10.))
                            .gap(px(12.))
                            .items_center()
                            .rounded(px(10.))
                            .bg(rgba(if is_last {
                                crate::theme::accent_subtle_bg()
                            } else {
                                crate::theme::card_bg_subtle()
                            }))
                            .border_1()
                            .border_color(rgba(if is_last {
                                crate::theme::accent_subtle_border()
                            } else {
                                crate::theme::card_border()
                            }))
                            .focus(|s| s.border_2().border_color(rgb(crate::theme::ACCENT)))
                            .cursor_pointer()
                            .hover(|s| {
                                s.bg(rgba(if is_last {
                                    crate::theme::accent_subtle_hover()
                                } else {
                                    crate::theme::hover()
                                }))
                                .border_color(rgba(if is_last {
                                    crate::theme::accent_subtle_border()
                                } else {
                                    crate::theme::hover_strong()
                                }))
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.launch_selected_entry(path_click.clone(), cx);
                                this.restore_launch_chooser_focus(window, cx);
                            }))
                            .child(
                                div()
                                    .size(px(34.))
                                    .flex_shrink_0()
                                    .rounded(px(8.))
                                    .bg(rgba(if is_last {
                                        crate::theme::accent_subtle_hover()
                                    } else {
                                        crate::theme::card_bg()
                                    }))
                                    .border_1()
                                    .border_color(rgba(crate::theme::card_border()))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("play", 14.).text_color(rgb(if is_last {
                                        crate::theme::accent_fg()
                                    } else {
                                        crate::theme::text()
                                    }))),
                            )
                            .child(
                                col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(
                                        row()
                                            .gap(px(6.))
                                            .items_center()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(px(13.))
                                                    .text_color(rgb(crate::theme::text()))
                                                    .truncate()
                                                    .child(name),
                                            )
                                            .when(is_last, |d| {
                                                d.child(
                                                    div()
                                                        .px(px(6.))
                                                        .py(px(1.))
                                                        .rounded(px(4.))
                                                        .bg(rgba(crate::theme::accent_subtle_hover()))
                                                        .text_size(px(10.))
                                                        .text_color(rgb(crate::theme::accent_fg()))
                                                        .child(crate::i18n::s().dialogs.last_launched),
                                                )
                                            }),
                                    )
                                    .child(muted(location).text_size(px(11.)).truncate()),
                            )
                            .child(muted(crate::i18n::s().dialogs.launch_chevron).text_size(px(12.))),
                    )
                    .child(
                        row()
                            .id(("launch-chooser-settings", i))
                            .track_focus(&settings_handles[i])
                            .flex_shrink_0()
                            .px(px(10.))
                            .py(px(10.))
                            .gap(px(5.))
                            .items_center()
                            .rounded(px(10.))
                            .bg(rgba(crate::theme::card_bg_subtle()))
                            .border_1()
                            .border_color(rgba(crate::theme::card_border()))
                            .focus(|s| s.border_2().border_color(rgb(crate::theme::ACCENT)))
                            .cursor_pointer()
                            .hover(|s| {
                                s.bg(rgba(crate::theme::hover()))
                                    .border_color(rgba(crate::theme::hover_strong()))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_run_settings_for_executable(
                                    game_index,
                                    path_settings.clone(),
                                    cx,
                                );
                            }))
                            .child(
                                icon("sliders-horizontal", 13.)
                                    .text_color(rgb(crate::theme::subtext())),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(crate::theme::text()))
                                    .child(crate::i18n::s().dialogs.settings_label),
                            ),
                    ),
            );
        }

        let list_panel = div()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(self.bouncing_scroll_view(
                "launch-chooser-scroll",
                &list_scroll,
                list,
            ))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .child(
                        gpui_kit::component::scroll::Scrollbar::vertical(&list_scroll)
                            .id("launch-chooser-scrollbar")
                            .viewport_from_layout(),
                    ),
            );

        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(rgba(crate::theme::overlay()))
            .child(
                div()
                    .id("launch-chooser-dismiss")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.close_launch_chooser(window, cx);
                    })),
            )
            .child(
                col()
                    .id("launch-chooser-card")
                    .tab_group()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_key_down(cx.listener(move |_, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.modifiers.modified() || entry_handles.is_empty() {
                            return;
                        }
                        let n = entry_handles.len();
                        let entry_idx = entry_handles.iter().position(|h| h.is_focused(window));
                        let settings_idx =
                            settings_handles.iter().position(|h| h.is_focused(window));
                        match event.keystroke.key.as_str() {
                            "down" => {
                                let next = entry_idx.or(settings_idx).map_or(0, |i| (i + 1) % n);
                                if settings_idx.is_some() {
                                    settings_handles[next].focus(window, cx);
                                } else {
                                    entry_handles[next].focus(window, cx);
                                }
                                list_scroll.scroll_to_item(next);
                                cx.stop_propagation();
                            }
                            "up" => {
                                let prev =
                                    entry_idx.or(settings_idx).map_or(n - 1, |i| (i + n - 1) % n);
                                if settings_idx.is_some() {
                                    settings_handles[prev].focus(window, cx);
                                } else {
                                    entry_handles[prev].focus(window, cx);
                                }
                                list_scroll.scroll_to_item(prev);
                                cx.stop_propagation();
                            }
                            "right" => {
                                if let Some(i) = entry_idx {
                                    settings_handles[i].focus(window, cx);
                                    cx.stop_propagation();
                                }
                            }
                            "left" => {
                                if let Some(i) = settings_idx {
                                    entry_handles[i].focus(window, cx);
                                    cx.stop_propagation();
                                }
                            }
                            "home" => {
                                entry_handles[0].focus(window, cx);
                                list_scroll.scroll_to_item(0);
                                cx.stop_propagation();
                            }
                            "end" => {
                                entry_handles[n - 1].focus(window, cx);
                                list_scroll.scroll_to_item(n - 1);
                                cx.stop_propagation();
                            }
                            _ => {}
                        }
                    }))
                    .absolute()
                    .left(px((width - pw) / 2.))
                    .top(px((height - ph) / 2.))
                    .w(px(pw))
                    .h(px(ph))
                    .rounded(px(RADIUS_LG))
                    .overflow_hidden()
                    .bg(rgb(crate::theme::surface()))
                    .border_1()
                    .border_color(rgba(crate::theme::surface_border()))
                    .shadow_xl()
                    .child(
                        row()
                            .flex_shrink_0()
                            .px(px(18.))
                            .py(px(14.))
                            .gap(px(12.))
                            .items_center()
                            .border_b_1()
                            .border_color(rgba(crate::theme::divider()))
                            .child(thumb)
                            .child(
                                col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_size(px(14.))
                                            .text_color(rgb(crate::theme::text()))
                                            .truncate()
                                            .child(chooser.title.clone()),
                                    )
                                    .child(
                                        muted(crate::i18n::s().dialogs.launch_chooser_hint)
                                            .text_size(px(11.5)),
                                    ),
                            )
                            .child(
                                Button::new("launch-chooser-close")
                                    .ghost()
                                    .small()
                                    .icon(icon("x", 14.))
                                    .tooltip(crate::i18n::s().common.close_esc)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_launch_chooser(window, cx);
                                    })),
                            ),
                    )
                    .child(list_panel)
                    .child(
                        row()
                            .flex_shrink_0()
                            .px(px(18.))
                            .py(px(12.))
                            .gap(px(8.))
                            .border_t_1()
                            .border_color(rgba(crate::theme::divider()))
                            .child(
                                Button::new("launch-chooser-edit")
                                    .label(crate::i18n::s().dialogs.edit_entries_ellipsis)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.edit_launch_chooser_executables(cx);
                                    })),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new("launch-chooser-cancel")
                                    .label(crate::i18n::s().common.cancel)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_launch_chooser(window, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn game_tags_sheet(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let Some(game) = self.game(index).cloned() else {
            return self.form(
                "game-tags-scroll",
                vec![self.section(
                    "game-tags-missing",
                    crate::i18n::s().dialogs.tags_dialog_title,
                    vec![muted(crate::i18n::s().dialogs.game_not_found).into_any_element()],
                    cx,
                )],
            );
        };
        let mut sections = Vec::new();
        let mut current_rows: Vec<AnyElement> = vec![self.info(crate::i18n::s().dialogs.work_label, game.title.clone())];
        if game.tags.is_empty() {
            current_rows.push(
                muted(crate::i18n::s().dialogs.tags_empty_hint)
                    .py(px(8.))
                    .text_size(px(12.))
                    .into_any_element(),
            );
        } else {
            let mut pills = row().flex_wrap().gap(px(6.)).py(px(8.));
            for (ti, tag) in game.tags.iter().enumerate() {
                let tag_remove = tag.clone();
                pills = pills.child(
                    row()
                        .id(("sheet-game-tag", ti))
                        .items_center()
                        .gap(px(6.))
                        .h(px(26.))
                        .pl(px(10.))
                        .pr(px(7.))
                        .rounded_full()
                        .bg(rgba(crate::theme::accent_subtle_bg()))
                        .border_1()
                        .border_color(rgba(crate::theme::accent_subtle_border()))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(crate::theme::accent_subtle_hover())))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_game_tag(index, &tag_remove, cx);
                        }))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(crate::theme::accent_fg()))
                                .child(tag.clone()),
                        )
                        .child(icon("x", 11.).text_color(rgb(crate::theme::accent_fg()))),
                );
            }
            current_rows.push(pills.into_any_element());
        }
        sections.push(self.section("game-tags-current", crate::i18n::s().dialogs.current_tags_section, current_rows, cx));

        let add_row = row()
            .gap(px(8.))
            .py(px(8.))
            .items_center()
            .child(div().flex_1().min_w_0().child(Input::new(&self.tag_input)))
            .child(
                Button::new("game-tags-add-btn")
                    .label(crate::i18n::s().common.add)
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.submit_tag_input(Some(window), cx);
                    })),
            )
            .into_any_element();
        sections.push(self.section(
            "game-tags-add",
            crate::i18n::s().dialogs.new_tag_label,
            vec![add_row],
            cx,
        ));

        let existing = crate::library_view::collect_tag_counts(self.games(), true);
        if !existing.is_empty() {
            let mut pills = row().flex_wrap().gap(px(6.)).py(px(8.));
            for (ti, (tag, count)) in existing.into_iter().enumerate() {
                let active = game.has_tag(&tag);
                let tag_toggle = tag.clone();
                pills = pills.child(crate::controls::tag_chip(
                    ("sheet-existing-tag", ti),
                    tag,
                    active,
                    Some(count),
                    cx.listener(move |this, _, _, cx| {
                        this.toggle_game_tag(index, &tag_toggle, cx);
                    }),
                ));
            }
            sections.push(self.section(
                "game-tags-library",
                crate::i18n::s().dialogs.quick_pick_tags,
                vec![pills.into_any_element()],
                cx,
            ));
        }

        self.form("game-tags-scroll", sections)
    }

    fn manage_tags_sheet(&self, cx: &Context<Self>) -> AnyElement {
        let tags = crate::library_view::collect_tag_counts(self.games(), true);
        if tags.is_empty() {
            return self.form(
                "manage-tags-scroll",
                vec![self.section(
                    "manage-tags-empty",
                    crate::i18n::s().dialogs.manage_tags_title,
                    vec![
                        self.info(crate::i18n::s().dialogs.no_custom_tags, crate::i18n::s().dialogs.no_custom_tags_desc),
                    ],
                    cx,
                )],
            );
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, (tag, count)) in tags.into_iter().enumerate() {
            let renaming = self
                .tag_manager_rename_target
                .as_ref()
                .is_some_and(|target| target.to_lowercase() == tag.to_lowercase());
            if renaming {
                rows.push(
                    row()
                        .w_full()
                        .py(px(8.))
                        .gap(px(8.))
                        .items_center()
                        .when(i > 0, |d| {
                            d.border_t_1().border_color(rgba(crate::theme::divider()))
                        })
                        .child(div().flex_1().min_w_0().child(Input::new(&self.tag_manager_input)))
                        .child(
                            Button::new(("manage-tag-save", i))
                                .label(crate::i18n::s().common.save)
                                .primary()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_rename_library_tag(Some(window), cx);
                                })),
                        )
                        .child(
                            Button::new(("manage-tag-cancel", i))
                                .label(crate::i18n::s().common.cancel)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.cancel_rename_library_tag(cx);
                                })),
                        )
                        .into_any_element(),
                );
            } else {
                let tag_filter = tag.clone();
                let tag_rename = tag.clone();
                let tag_delete = tag.clone();
                rows.push(
                    row()
                        .w_full()
                        .py(px(8.))
                        .gap(px(8.))
                        .items_center()
                        .when(i > 0, |d| {
                            d.border_t_1().border_color(rgba(crate::theme::divider()))
                        })
                        .child(icon("tag", 13.).text_color(rgb(crate::theme::accent_fg())))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(px(13.))
                                .text_color(rgb(crate::theme::text()))
                                .child(tag),
                        )
                        .child(muted(crate::i18n::s().dialogs.tag_count_works(count)).text_size(px(11.5)))
                        .child(
                            Button::new(("manage-tag-filter", i))
                                .label(crate::i18n::s().dialogs.filter_btn)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_tag_filter(
                                        crate::library_view::TagFilter::Tags(vec![tag_filter.clone()]),
                                        cx,
                                    );
                                    this.preview_dismiss(cx);
                                })),
                        )
                        .child(
                            Button::new(("manage-tag-rename", i))
                                .label(crate::i18n::s().dialogs.rename_btn)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.begin_rename_library_tag(&tag_rename, Some(window), cx);
                                })),
                        )
                        .child(
                            Button::new(("manage-tag-delete", i))
                                .label(crate::i18n::s().common.delete)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.delete_library_tag(&tag_delete, cx);
                                })),
                        )
                        .into_any_element(),
                );
            }
        }
        self.form(
            "manage-tags-scroll",
            vec![self.section("manage-tags-list", crate::i18n::s().dialogs.all_custom_tags_section, rows, cx)],
        )
    }
}

fn about_tab_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    tab: AboutTab,
    cx: &mut Context<WhiskyApp>,
) -> AnyElement {
    row()
        .id(id)
        .px(px(11.))
        .py(px(5.))
        .rounded(px(7.))
        .bg(rgba(if active {
            crate::theme::accent_subtle_bg()
        } else {
            crate::theme::card_bg()
        }))
        .border_1()
        .border_color(rgba(if active {
            crate::theme::accent_subtle_border()
        } else {
            crate::theme::card_border()
        }))
        .cursor_pointer()
        .when(!active, |d| d.hover(|s| s.bg(rgba(crate::theme::hover()))))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.about_tab = tab;
            cx.notify();
        }))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(if active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(rgb(if active {
                    crate::theme::accent_fg()
                } else {
                    crate::theme::subtext()
                }))
                .child(label),
        )
        .into_any_element()
}

fn about_fact_row(label: &'static str, value: String) -> AnyElement {
    row()
        .gap(px(18.))
        .child(div().w(px(72.)).child(muted(label)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(12.))
                .text_color(rgb(crate::theme::text()))
                .child(value),
        )
        .into_any_element()
}

fn about_pill_badge(label: &'static str, accent: bool) -> AnyElement {
    row()
        .flex_shrink_0()
        .px(px(7.))
        .py(px(2.))
        .rounded(px(6.))
        .bg(rgba(if accent {
            crate::theme::accent_subtle_bg()
        } else {
            crate::theme::badge_bg()
        }))
        .border_1()
        .border_color(rgba(if accent {
            crate::theme::accent_subtle_border()
        } else {
            crate::theme::card_border()
        }))
        .child(
            div()
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(if accent {
                    crate::theme::accent_fg()
                } else {
                    crate::theme::badge_text()
                }))
                .child(label),
        )
        .into_any_element()
}

fn about_ack_card(item: &AcknowledgmentItem) -> AnyElement {
    col()
        .w_full()
        .rounded(px(10.))
        .p(px(13.))
        .gap(px(6.))
        .bg(rgba(if item.highlighted {
            crate::theme::section_bg()
        } else {
            crate::theme::card_bg()
        }))
        .border_1()
        .border_color(rgba(if item.highlighted {
            crate::theme::accent_subtle_border()
        } else {
            crate::theme::card_border()
        }))
        .child(
            row()
                .flex_wrap()
                .justify_between()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(crate::theme::text()))
                        .child(item.display_name()),
                )
                .child(about_pill_badge(item.badge, item.highlighted)),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .text_size(px(11.5))
                .line_height(relative(1.45))
                .text_color(rgb(crate::theme::subtext()))
                .child(item.display_description()),
        )
        .into_any_element()
}

impl WhiskyApp {
    fn about_acknowledgments_tab(&self) -> AnyElement {
        self.flex_scroll(
            "about-ack-scroll",
            col()
                .w_full()
                .px(px(20.))
                .py(px(16.))
                .gap(px(10.))
                .child(
                    muted(crate::i18n::s().dialogs.about_ack_intro)
                        .text_size(px(11.5)),
                )
                .children(ACKNOWLEDGMENTS.iter().map(about_ack_card)),
        )
    }

    fn about_document_tab(
        &self,
        scroll_id: &'static str,
        intro: &'static str,
        body: &'static str,
        markdown_headings: bool,
    ) -> AnyElement {
        let paragraphs = body
            .split("\n\n")
            .map(|block| block.trim_end_matches('\n'))
            .filter(|block| !block.trim().is_empty())
            .map(|block| {
                let is_heading = markdown_headings && block.trim_start().starts_with('#');
                div()
                    .w_full()
                    .min_w_0()
                    .text_size(px(if is_heading { 12.5 } else { 11.5 }))
                    .font_weight(if is_heading {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .line_height(relative(1.5))
                    .text_color(rgb(if is_heading {
                        crate::theme::text()
                    } else {
                        crate::theme::subtext()
                    }))
                    .child(block.to_owned())
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        self.flex_scroll(
            scroll_id,
            col()
                .w_full()
                .px(px(20.))
                .py(px(16.))
                .gap(px(10.))
                .child(muted(intro).text_size(px(11.5)))
                .child(
                    col()
                        .w_full()
                        .rounded(px(10.))
                        .p(px(14.))
                        .gap(px(10.))
                        .bg(rgba(crate::theme::code_bg()))
                        .border_1()
                        .border_color(rgba(crate::theme::card_border()))
                        .children(paragraphs),
                ),
        )
    }
}

#[cfg(test)]
mod about_tests {
    use super::{ACKNOWLEDGMENTS, GPL_LICENSE_TEXT, THIRD_PARTY_NOTICES_TEXT};

    #[test]
    fn embedded_legal_documents_contain_expected_notices() {
        assert!(GPL_LICENSE_TEXT.contains("GNU GENERAL PUBLIC LICENSE"));
        assert!(THIRD_PARTY_NOTICES_TEXT.contains("LunaTranslator"));
        assert!(
            ACKNOWLEDGMENTS
                .iter()
                .any(|item| item.name.contains("LunaHook / LunaTranslator"))
        );
    }
}
