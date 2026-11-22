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
        ("adaptive", crate::i18n::tr("动态 MetalFX／静态 Real-CUGAN", "動的 MetalFX／静的 Real-CUGAN", "Dynamic MetalFX / Static Real-CUGAN", "동적 MetalFX / 정적 Real-CUGAN")),
        ("realesrgan", "Real-ESRGAN AnimeVideo-v3 (x2)"),
        ("adaptive-realesrgan", crate::i18n::tr("动态 MetalFX／静态 AnimeVideo-v3", "動的 MetalFX／静的 AnimeVideo-v3", "Dynamic MetalFX / Static AnimeVideo-v3", "동적 MetalFX / 정적 AnimeVideo-v3")),
        ("adaptive-anime4k", crate::i18n::tr("动态 Anime4K／静态 Real-CUGAN", "動的 Anime4K／静的 Real-CUGAN", "Dynamic Anime4K / Static Real-CUGAN", "동적 Anime4K / 정적 Real-CUGAN")),
        ("adaptive-anime4k-realesrgan", crate::i18n::tr("动态 Anime4K／静态 AnimeVideo-v3", "動的 Anime4K／静的 AnimeVideo-v3", "Dynamic Anime4K / Static AnimeVideo-v3", "동적 Anime4K / 정적 AnimeVideo-v3")),
    ]
}

fn gdi_caches() -> Vec<(&'static str, &'static str)> {
    vec![
        ("off", crate::i18n::tr("关闭跨场景缓存", "シーン間キャッシュをオフ", "Cross-scene cache off", "장면 간 캐시 끄기")),
        ("memory", crate::i18n::tr("本次游戏缓存", "今回のセッションのみキャッシュ", "Memory cache (this session)", "이번 세션만 메모리 캐시")),
        ("persistent", crate::i18n::tr("缓存并保存到磁盘", "キャッシュしてディスクに保存", "Cache and save to disk", "캐시 및 디스크에 저장")),
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
        ("matchDisplay", crate::i18n::tr("匹配显示器", "ディスプレイに合わせる", "Match Display", "디스플레이에 맞춤")),
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
        (AboutTab::Overview, "about-tab-overview", crate::i18n::tr("概览", "概要", "Overview", "개요")),
        (AboutTab::Acknowledgments, "about-tab-ack", d.about_view_ack),
        (AboutTab::License, "about-tab-license", crate::i18n::tr("许可证 (GPLv3)", "ライセンス (GPLv3)", "License (GPLv3)", "라이선스 (GPLv3)")),
        (AboutTab::Notices, "about-tab-notices", crate::i18n::tr("第三方声明", "サードパーティ表記", "Third-Party Notices", "서드파티 고지")),
    ]
}

struct AcknowledgmentItem {
    #[allow(dead_code)]
    name: &'static str,
    badge: &'static str,
    #[allow(dead_code)]
    description: &'static str,
    highlighted: bool,
}

impl AcknowledgmentItem {
    fn display_name(&self) -> &'static str {
        match self.name {
            "gpui-kit / Zed GPUI & Rust 生态依赖" => crate::i18n::tr(
                "gpui-kit / Zed GPUI & Rust 生态依赖",
                "gpui-kit / Zed GPUI & Rust エコシステム",
                "gpui-kit / Zed GPUI & Rust Ecosystem",
                "gpui-kit / Zed GPUI 및 Rust 생태계",
            ),
            "PotatoVN & 元数据公开接口" => crate::i18n::tr(
                "PotatoVN & 元数据公开接口",
                "PotatoVN & メタデータ公開 API",
                "PotatoVN & Public Metadata APIs",
                "PotatoVN 및 공개 메타데이터 API",
            ),
            other => other,
        }
    }

    fn display_description(&self) -> &'static str {
        match self.badge {
            "HIllya51/LunaTranslator · GPL-3.0" => crate::i18n::tr(
                "核心 Windows 文本钩取与进程注入引擎（LunaHook32/64.dll、LunaHost32/64.dll、LunaTmpFontLoader.dll），配套 MinHook（BSD-2-Clause，Tsuda Kageyu / HDE by Vyacheslav Patkov）、uchardet（GPL-2.0-or-later）与 nlohmann/json（MIT）。",
                "中核の Windows テキストフック・インジェクションエンジン（LunaHook32/64.dll、LunaHost32/64.dll、LunaTmpFontLoader.dll）。MinHook（BSD-2-Clause）、uchardet（GPL-2.0-or-later）、nlohmann/json（MIT）を同梱。",
                "Core Windows text hook & process injection engine (LunaHook32/64.dll, LunaHost32/64.dll, LunaTmpFontLoader.dll), with MinHook (BSD-2-Clause), uchardet (GPL-2.0-or-later), and nlohmann/json (MIT).",
                "핵심 Windows 텍스트 후킹 및 프로세스 인젝션 엔진(LunaHook32/64.dll, LunaHost32/64.dll, LunaTmpFontLoader.dll)과 MinHook(BSD-2-Clause), uchardet(GPL-2.0-or-later), nlohmann/json(MIT) 포함.",
            ),
            "whisky-app/whisky / frankea/Whisky · GPL-3.0" => crate::i18n::tr(
                "酒瓶容器管理架构、偏好设置模型与原生桥接基础。",
                "ボトル管理アーキテクチャ、設定モデル、およびネイティブブリッジ基盤。",
                "Bottle container management architecture, preference model, and native bridge foundation.",
                "보틀 컨테이너 관리 아키텍처, 환경설정 모델 및 네이티브 브리지 기반.",
            ),
            "LGPL-2.1+" => crate::i18n::tr(
                "Windows 兼容运行时，以及 msync、DXVK-macOS、DXMT、MoltenVK、D3DMetal 等图形与同步兼容组件。",
                "Windows 互換ランタイム、および msync、DXVK-macOS、DXMT、MoltenVK、D3DMetal などのグラフィックス・同期コンポーネント。",
                "Windows compatibility runtime along with graphics and sync components including msync, DXVK-macOS, DXMT, MoltenVK, and D3DMetal.",
                "Windows 호환 런타임과 msync, DXVK-macOS, DXMT, MoltenVK, D3DMetal 등 그래픽·동기화 구성 요소.",
            ),
            "LGPL-2.1+ · GPL-2.0+ / LGPL-2.1+" => crate::i18n::tr(
                "Windows 运行库组件安装脚本与 CAB 压缩包解包支持。",
                "Windows ランタイムライブラリ導入スクリプトと CAB アーカイブ展開サポート。",
                "Windows runtime component installation scripts and CAB archive extraction.",
                "Windows 런타임 라이브러리 설치 스크립트 및 CAB 압축 해제 지원.",
            ),
            "Apache-2.0" => crate::i18n::tr(
                "硬件加速原生桌面界面框架及 Rust 生态基础依赖。",
                "ハードウェアアクセラレーション対応のネイティブデスクトップ UI フレームワークおよび Rust 基盤クレート。",
                "Hardware-accelerated native desktop UI framework and foundational Rust ecosystem crates.",
                "하드웨어 가속 네이티브 데스크톱 UI 프레임워크 및 Rust 기반 크레이트.",
            ),
            "bilibili/ailab & nihui · MIT / bloc97 · MIT" => crate::i18n::tr(
                "封面超分辨率模型（Real-CUGAN models-se）与实时 Metal 着色器管线。",
                "カバー超解像モデル（Real-CUGAN models-se）およびリアルタイム Metal シェーダーパイプライン。",
                "Cover super-resolution models (Real-CUGAN models-se) and real-time Metal shader pipelines.",
                "커버 초해상도 모델(Real-CUGAN models-se) 및 실시간 Metal 셰이더 파이프라인.",
            ),
            _ => crate::i18n::tr(
                "视觉小说库与元数据设计参考，及 Bangumi、VNDB、Steam、YMGal、NextMoe 公开数据来源。",
                "ビジュアルノベルライブラリおよびメタデータ設計の参考、ならびに Bangumi・VNDB・Steam・YMGal・NextMoe の公開データソース。",
                "Visual novel library and metadata design reference, plus public metadata sources (Bangumi, VNDB, Steam, YMGal, NextMoe).",
                "비주얼 노벨 라이브러리·메타데이터 설계 참고 및 Bangumi·VNDB·Steam·YMGal·NextMoe 공개 데이터 소스.",
            ),
        }
    }
}

const ACKNOWLEDGMENTS: &[AcknowledgmentItem] = &[
    AcknowledgmentItem {
        name: "LunaHook / LunaTranslator",
        badge: "HIllya51/LunaTranslator · GPL-3.0",
        description: "核心 Windows 文本钩取与进程注入引擎（LunaHook32/64.dll、LunaHost32/64.dll、LunaTmpFontLoader.dll），配套 MinHook（BSD-2-Clause，Tsuda Kageyu / HDE by Vyacheslav Patkov）、uchardet（GPL-2.0-or-later）与 nlohmann/json（MIT）。",
        highlighted: true,
    },
    AcknowledgmentItem {
        name: "Whisky & Contributors",
        badge: "whisky-app/whisky / frankea/Whisky · GPL-3.0",
        description: "酒瓶容器管理架构、偏好设置模型与原生桥接基础。",
        highlighted: false,
    },
    AcknowledgmentItem {
        name: "Wine / CrossOver Wine & GE-Proton",
        badge: "LGPL-2.1+",
        description: "Windows 兼容运行时，以及 msync、DXVK-macOS、DXMT、MoltenVK、D3DMetal 等图形与同步兼容组件。",
        highlighted: false,
    },
    AcknowledgmentItem {
        name: "Winetricks & cabextract / libmspack",
        badge: "LGPL-2.1+ · GPL-2.0+ / LGPL-2.1+",
        description: "Windows 运行库组件安装脚本与 CAB 压缩包解包支持。",
        highlighted: false,
    },
    AcknowledgmentItem {
        name: "gpui-kit / Zed GPUI & Rust 生态依赖",
        badge: "Apache-2.0",
        description: "硬件加速原生桌面界面框架及 Rust 生态基础依赖。",
        highlighted: false,
    },
    AcknowledgmentItem {
        name: "Real-CUGAN & Anime4K",
        badge: "bilibili/ailab & nihui · MIT / bloc97 · MIT",
        description: "封面超分辨率模型（Real-CUGAN models-se）与实时 Metal 着色器管线。",
        highlighted: false,
    },
    AcknowledgmentItem {
        name: "PotatoVN & 元数据公开接口",
        badge: "GoldenPotato137/PotatoVN · Apache-2.0",
        description: "视觉小说库与元数据设计参考，及 Bangumi、VNDB、Steam、YMGal、NextMoe 公开数据来源。",
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
                                crate::i18n::tr("日语（日本）", "日本語（日本）", "Japanese (Japan)", "일본어 (일본)"),
                                crate::i18n::tr("简体中文（中国）", "簡体字中国語（中国）", "Simplified Chinese (China)", "중국어 간체 (중국)"),
                                crate::i18n::tr("英语（美国）", "英語（米国）", "English (US)", "영어 (미국)"),
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
                    crate::i18n::tr("程序专属覆盖", "プログラム固有の上書き", "Program Overrides", "프로그램별 재정의"),
                    vec![
                        self.toggle(
                            "override-graphics",
                            crate::i18n::s().program_settings.override_graphics,
                            crate::i18n::tr("关闭时继承容器设置。", "オフの場合はコンテナ設定を継承します。", "Inherits bottle settings when off.", "끄면 컨테이너 설정을 상속합니다."),
                            cx,
                        ),
                        self.choice(
                            "program-backend",
                            crate::i18n::tr(crate::i18n::tr("图形后端", "グラフィックスバックエンド", "Graphics Backend", "그래픽 백엔드"), "グラフィックスバックエンド", "Graphics Backend", "그래픽 백엔드"),
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
                    crate::i18n::tr("诊断", "診断", "Diagnostics", "진단"),
                    vec![
                        self.info(
                            crate::i18n::tr("最近一次运行", "前回の実行", "Last Run", "최근 실행"),
                            crate::i18n::s().program_settings.demo_last_run_value,
                        ),
                        self.action(
                            "run-troubleshoot",
                            crate::i18n::tr("故障排查…", "トラブルシューティング…", "Troubleshoot…", "문제 해결…"),
                            Command::Sheet(PreviewSheet::Diagnostics),
                            cx,
                        ),
                    ],
                    cx,
                ),
                self.section(
                    "console",
                    crate::i18n::tr("控制台与运行历史", "コンソールと実行履歴", "Console & Run History", "콘솔 및 실행 기록"),
                    vec![
                        self.info(crate::i18n::s().program_settings.just_now, crate::i18n::s().program_settings.demo_exit_code_0),
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
                crate::i18n::tr("保存失败", "保存に失敗しました", "Save Failed", "저장 실패"),
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
                Some(exe) => match crate::i18n::locale() {
                    crate::i18n::Locale::ZhHans => format!("{base} · 记忆目标进程：{exe}"),
                    crate::i18n::Locale::Ja => format!("{base} · 記憶プロセス：{exe}"),
                    crate::i18n::Locale::En => format!("{base} · Remembered process: {exe}"),
                    crate::i18n::Locale::Ko => format!("{base} · 기억된 프로세스: {exe}"),
                },
                None => match crate::i18n::locale() {
                    crate::i18n::Locale::ZhHans => format!("{base} · 尚未记忆目标进程"),
                    crate::i18n::Locale::Ja => format!("{base} · 対象プロセス未記憶"),
                    crate::i18n::Locale::En => format!("{base} · No remembered process"),
                    crate::i18n::Locale::Ko => format!("{base} · 기억된 대상 프로세스 없음"),
                },
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
                muted(match crate::i18n::locale() {
                    crate::i18n::Locale::ZhHans => format!("当前仅编辑 {} 的独立运行设置（含旧版全屏兼容、图形、语言与参数）。", settings.program_name),
                    crate::i18n::Locale::Ja => format!("現在 {} の個別実行設定（旧式フルスクリーン互換・グラフィックス・言語・引数）のみを編集中です。", settings.program_name),
                    crate::i18n::Locale::En => format!("Editing per-program run settings only for {} (including legacy fullscreen, graphics, locale, and args).", settings.program_name),
                    crate::i18n::Locale::Ko => format!("현재 {}의 개별 실행 설정(레거시 전체 화면 호환, 그래픽, 로캘 및 인수)만 편집 중입니다.", settings.program_name),
                })
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
        let mut children: Vec<AnyElement> = vec![self.program_env_presets(cx)];
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
                crate::i18n::tr("图形后端", "グラフィックスバックエンド", "Graphics Backend", "그래픽 백엔드"),
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
                muted(match crate::i18n::locale() {
                    crate::i18n::Locale::ZhHans => format!("继承：{g} · HUD {h}"),
                    crate::i18n::Locale::Ja => format!("継承：{g} · HUD {h}"),
                    crate::i18n::Locale::En => format!("Inherited: {g} · HUD {h}"),
                    crate::i18n::Locale::Ko => format!("상속됨: {g} · HUD {h}"),
                })
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
                muted(match crate::i18n::locale() {
                    crate::i18n::Locale::ZhHans => format!("继承：{u} · {c}"),
                    crate::i18n::Locale::Ja => format!("継承：{u} · {c}"),
                    crate::i18n::Locale::En => format!("Inherited: {u} · {c}"),
                    crate::i18n::Locale::Ko => format!("상속됨: {u} · {c}"),
                })
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
            children.push(muted(match crate::i18n::locale() { crate::i18n::Locale::ZhHans => format!("旧缓存：{}", dir.display()), crate::i18n::Locale::Ja => format!("旧キャッシュ：{}", dir.display()), crate::i18n::Locale::En => format!("Legacy cache: {}", dir.display()), crate::i18n::Locale::Ko => format!("이전 캐시: {}", dir.display()) }).text_size(px(11.)).truncate().into_any_element());
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
                muted(match crate::i18n::locale() { crate::i18n::Locale::ZhHans => format!("继承：{}", program_settings::sync_label(sync)), crate::i18n::Locale::Ja => format!("継承：{}", program_settings::sync_label(sync)), crate::i18n::Locale::En => format!("Inherited: {}", program_settings::sync_label(sync)), crate::i18n::Locale::Ko => format!("상속됨: {}", program_settings::sync_label(sync)) }).into_any_element(),
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
                    muted(match crate::i18n::locale() {
                        crate::i18n::Locale::ZhHans => format!("继承：着色器缓存 {st}"),
                        crate::i18n::Locale::Ja => format!("継承：シェーダーキャッシュ {st}"),
                        crate::i18n::Locale::En => format!("Inherited: Shader Cache {st}"),
                        crate::i18n::Locale::Ko => format!("상속됨: 셰이더 캐시 {st}"),
                    })
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
        let mut children = vec![self.program_switch(
            "ps-override-input".into(),
            crate::i18n::s().program_settings.override_input,
            enabled,
            false,
            |this, on, _cx| this.set_program_override_group("input", on),
            cx,
        )];
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
                muted(match crate::i18n::locale() {
                    crate::i18n::Locale::ZhHans => format!("继承：控制器兼容 {a} · 原生标签 {b}"),
                    crate::i18n::Locale::Ja => format!("継承：コントローラー互換 {a} · ネイティブ表示 {b}"),
                    crate::i18n::Locale::En => format!("Inherited: Controller Compat {a} · Native Labels {b}"),
                    crate::i18n::Locale::Ko => format!("상속됨: 컨트롤러 호환 {a} · 네이티브 레이블 {b}"),
                })
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
                    muted(match crate::i18n::locale() {
                        crate::i18n::Locale::ZhHans => format!("继承：{vd} · {res}"),
                        crate::i18n::Locale::Ja => format!("継承：{vd} · {res}"),
                        crate::i18n::Locale::En => format!("Inherited: {vd} · {res}"),
                        crate::i18n::Locale::Ko => format!("상속됨: {vd} · {res}"),
                    })
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
                muted(match crate::i18n::locale() { crate::i18n::Locale::ZhHans => format!("继承：容器级 {count} 条 DLL 覆盖"), crate::i18n::Locale::Ja => format!("継承：コンテナレベル {count} 件の DLL 上書き"), crate::i18n::Locale::En => format!("Inherited: {count} bottle DLL overrides"), crate::i18n::Locale::Ko => format!("상속됨: 컨테이너 DLL 재정의 {count}개") }).into_any_element(),
            );
        }
        self.section("override-dll", crate::i18n::tr("DLL 覆盖", "DLL オーバーライド", "DLL Overrides", "DLL 재정의"), children, cx)
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
            .map(|code| match crate::i18n::locale() {
                crate::i18n::Locale::ZhHans => format!("退出 {code}"),
                crate::i18n::Locale::Ja => format!("終了 {code}"),
                crate::i18n::Locale::En => format!("Exit {code}"),
                crate::i18n::Locale::Ko => format!("종료 {code}"),
            })
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
            muted(if cfg!(target_os = "linux") { crate::i18n::s().program_settings.shortcut_desc_linux } else { crate::i18n::s().program_settings.shortcut_desc_mac })
                .into_any_element(),
        ];
        if let Some(path) = &self.program_settings.shortcut_path {
            children.push(muted(match crate::i18n::locale() { crate::i18n::Locale::ZhHans => format!("已创建：{path}"), crate::i18n::Locale::Ja => format!("作成済み：{path}"), crate::i18n::Locale::En => format!("Created: {path}"), crate::i18n::Locale::Ko => format!("생성됨: {path}") }).into_any_element());
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
                    crate::i18n::tr("扫描文件夹", "スキャンフォルダ", "Scan Folders", "스캔 폴더"),
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
                        .child(div().flex_1().min_w_0().child(crate::i18n::tr("自动读取元数据", "メタデータを自動取得", "Fetch Metadata Automatically", "메타데이터 자동 가져오기")))
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
                                .child(crate::i18n::tr("包含固定程序与 Steam 游戏", "固定プログラムと Steam ゲームを含める", "Include Pinned Programs & Steam Games", "고정된 프로그램 및 Steam 게임 포함"))
                                .child(muted(crate::i18n::tr("关闭后游戏库只显示扫描文件夹中的游戏。", "オフにするとスキャンフォルダ内のゲームのみ表示します。", "When off, only games from scan folders are shown.", "끄면 스캔 폴더의 게임만 라이브러리에 표시됩니다.")).text_size(px(11.))),
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
                    rows.push(muted(crate::i18n::tr("尚未添加扫描文件夹", "スキャンフォルダがまだありません", "No scan folders added yet", "추가된 스캔 폴더가 없습니다")).into_any_element());
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
                                .label(crate::i18n::tr("选择文件夹…", "フォルダを選択…", "Choose Folder…", "폴더 선택…"))
                                .primary()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.choose_scan_roots(cx);
                                })),
                        )
                        .child(muted(crate::i18n::tr("可多选，从容器 C: 盘开始；子文件夹会成为游戏条目", "複数選択可（ドライブ C から開始）。サブフォルダがゲーム項目になります", "Multi-select starting from bottle C: drive; subfolders become game entries", "컨테이너 C: 드라이브부터 복수 선택 가능; 하위 폴더가 게임 항목이 됩니다")).text_size(px(11.)))
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
                                .label(crate::i18n::tr("添加路径", "パスを追加", "Add Path", "경로 추가"))
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
                                .label(crate::i18n::tr("添加单个游戏文件夹…", "単一ゲームフォルダを追加…", "Add Single Game Folder…", "단일 게임 폴더 추가…"))
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.choose_assign_folders(cx);
                                })),
                        )
                        .child(
                            muted(crate::i18n::tr("文件夹本身就是一个游戏，不扫描其子目录", "フォルダ自体を 1 つのゲームとして登録し、サブフォルダはスキャンしません", "Treat this folder itself as one game without scanning subfolders", "하위 폴더를 스캔하지 않고 이 폴더 자체를 하나의 게임으로 등록"))
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
                    .child(muted(crate::i18n::tr("正在读取扫描文件夹…", "スキャンフォルダを読み込み中…", "Loading scan folders…", "스캔 폴더 읽는 중…")))
                    .into_any_element(),
            ),
            None => {
                let reason = self
                    .folders_error
                    .get(&container)
                    .cloned()
                    .unwrap_or_else(|| crate::i18n::tr("尚未读取扫描文件夹。", "スキャンフォルダがまだ読み込まれていません。", "Scan folders have not been loaded yet.", "스캔 폴더를 아직 읽지 않았습니다.").to_owned());
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
            vec![self.section("scan-folders", crate::i18n::tr("扫描文件夹", "スキャンフォルダ", "Scan Folders", "스캔 폴더"), rows, cx)],
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
                    .label(crate::i18n::tr("移动…", "移動…", "Move…", "이동…"))
                    .ghost()
                    .dropdown_menu(move |mut menu, _, _| {
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
                            .child(muted(crate::i18n::tr("扫描文件夹", "スキャンフォルダ", "Scan Folder", "스캔 폴더")).truncate()),
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
            .child(muted(crate::i18n::s().library.container_prefix(&container)).text_size(px(10.)))
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
            crate::i18n::tr("未设置可执行文件", "実行ファイル未設定", "No executable set", "실행 파일이 설정되지 않음").to_owned()
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
                            .label(crate::i18n::tr("排除", "除外", "Exclude", "제외"))
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
                            .label(crate::i18n::tr("设置可执行文件", "実行ファイルを設定", "Set Executable", "실행 파일 설정"))
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
            .child(muted(crate::i18n::s().library.container_prefix(&container)).text_size(px(10.)))
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
                .placeholder(crate::i18n::tr("/路径/游戏.exe", "/path/to/game.exe", "/path/to/game.exe", "/경로/game.exe"))
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
                            .label(crate::i18n::tr("取消扫描", "スキャンをキャンセル", "Cancel Scan", "스캔 취소"))
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
                            .label(if scan.is_some() { crate::i18n::tr("再次扫描", "再スキャン", "Scan Again", "다시 스캔") } else { crate::i18n::tr("立即扫描", "今すぐスキャン", "Scan Now", "지금 스캔") })
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
                crate::i18n::tr("扫描文件夹", "スキャンフォルダ", "Scan Folders", "스캔 폴더"),
                vec![
                    self.info("~/Games", crate::i18n::tr("递归扫描", "再帰スキャン", "Recursive", "재귀 스캔")),
                    self.field("scan-path", crate::i18n::tr("添加文件夹", "フォルダを追加", "Add Folder", "폴더 추가")),
                    self.action("scan-add", crate::i18n::tr("添加文件夹", "フォルダを追加", "Add Folder", "폴더 추가"), Command::Notice, cx),
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
                    crate::i18n::tr("启动程序", "起動プログラム", "Launch Program", "실행 프로그램"),
                    vec![self.info(
                        crate::i18n::tr("没有找到可启动的程序", "起動可能なプログラムが見つかりません", "No launchable executable found", "실행 가능한 프로그램을 찾을 수 없습니다"),
                        crate::i18n::tr("该文件夹下没有 .exe", "このフォルダ内に .exe がありません", "No .exe in this folder", "이 폴더에 .exe 파일이 없습니다"),
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
                    .child(muted(crate::i18n::tr("正在递归扫描该文件夹下的可执行程序…", "このフォルダ内の実行ファイルを再帰スキャン中…", "Scanning executables in this folder…", "이 폴더의 실행 파일 스캔 중…")))
                    .into_any_element(),
            );
        } else if let Some(reason) = picker.error.as_deref() {
            rows.push(self.info(crate::i18n::tr("扫描失败", "スキャン失敗", "Scan Failed", "스캔 실패"), reason));
        } else if picker.candidates.is_empty() {
            rows.push(self.info(crate::i18n::tr("没有找到可启动的程序", "起動可能なプログラムが見つかりません", "No launchable executable found", "실행 가능한 프로그램을 찾을 수 없습니다"), crate::i18n::tr("该文件夹下没有 .exe", "このフォルダ内に .exe がありません", "No .exe in this folder", "이 폴더에 .exe 파일이 없습니다")));
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
                                                .child(crate::i18n::tr("主程序", "メイン", "Primary", "기본")),
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
                            .child(muted(if is_checked { crate::i18n::tr("已勾选", "選択済み", "Selected", "선택됨") } else { crate::i18n::tr("点击勾选", "クリックして選択", "Click to select", "클릭하여 선택") }))
                            .into_any_element()
                    } else {
                        row()
                            .gap(px(4.))
                            .items_center()
                            .child(muted(crate::i18n::tr("选择", "選択", "Select", "선택")))
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
                            .child(div().text_size(px(13.)).child(crate::i18n::tr("多 exe 入口模式", "複数 .exe エントリーモード", "Multi-exe Entry Mode", "다중 .exe 진입점 모드")))
                            .child(
                                muted(if multi_select {
                                    {
                                        let n = selected.len();
                                        match crate::i18n::locale() {
                                            crate::i18n::Locale::ZhHans => format!("已勾选 {n} 个入口，每次启动将弹出快捷选择面板（↑↓ 移动，Space 勾选，Cmd/Ctrl+Enter 确认）。"),
                                            crate::i18n::Locale::Ja => format!("{n} 個のエントリーを選択中。起動時に選択パネルを表示します（↑↓ 移動、Space 選択、Cmd/Ctrl+Enter 確定）。"),
                                            crate::i18n::Locale::En => format!("{n} entries selected; a quick chooser opens on launch (↑↓ move, Space toggle, Cmd/Ctrl+Enter confirm)."),
                                            crate::i18n::Locale::Ko => format!("{n}개 진입점 선택됨. 실행 시 빠른 선택 패널이 열립니다(↑↓ 이동, Space 선택, Cmd/Ctrl+Enter 확인)."),
                                        }
                                    }
                                } else {
                                    crate::i18n::tr("开启后可同时勾选多个 .exe 作为作品的启动入口。", "オンにすると複数の .exe を起動エントリーとして登録できます。", "Enable to select multiple .exe files as launch entries.", "켜면 여러 .exe를 실행 진입점으로 선택할 수 있습니다.").to_owned()
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
                    self.section("executable-folder", crate::i18n::tr("游戏目录", "ゲームフォルダ", "Game Folder", "게임 폴더"), info_rows, cx),
                    self.section(
                        "executable-list",
                        if multi_select {
                            crate::i18n::tr("勾选可执行程序（可多选）", "実行ファイルを選択（複数可）", "Select Executables (Multi-select)", "실행 파일 선택(복수 선택)")
                        } else {
                            crate::i18n::tr("选择可执行程序 (.exe)", "実行ファイル (.exe) を選択", "Select Executable (.exe)", "실행 파일 (.exe) 선택")
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
                    crate::i18n::tr("可用组件", "利用可能なコンポーネント", "Available Components", "사용 가능한 구성 요소"),
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
                .child(crate::i18n::tr("搜索组件", "コンポーネントを検索", "Search Components", "구성 요소 검색"))
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
                    .label(crate::i18n::tr("仅显示已安装", "インストール済みのみ", "Installed Only", "설치됨만 표시"))
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
                    .child(muted(crate::i18n::tr("正在读取已安装组件…", "インストール済みコンポーネントを読み込み中…", "Loading installed components…", "설치된 구성 요소 읽는 중…")));
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
        sections.push(self.section("trick-filter", crate::i18n::tr("搜索与筛选", "検索とフィルター", "Search & Filter", "검색 및 필터"), filter_rows, cx));

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
                        crate::i18n::tr("安装进度", "インストールの進行状況", "Install Progress", "설치 진행률"),
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
                            .child(muted(crate::i18n::tr("正在读取已安装组件…", "インストール済みコンポーネントを読み込み中…", "Loading installed components…", "설치된 구성 요소 읽는 중…")))
                            .into_any_element(),
                    );
                } else if installed_missing {
                    let reason = self
                        .winetricks
                        .installed_error
                        .get(&container)
                        .cloned()
                        .unwrap_or_else(|| crate::i18n::tr("尚未读取已安装组件。", "インストール済みコンポーネントはまだ読み込まれていません。", "Installed components have not been loaded yet.", "설치된 구성 요소를 아직 읽지 않았습니다.").to_owned());
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
                                crate::i18n::tr("该分类下还没有已安装的组件。", "このカテゴリにはインストール済みコンポーネントがありません。", "No installed components in this category.", "이 카테고리에 설치된 구성 요소가 없습니다.")
                            } else {
                                crate::i18n::tr("没有匹配的组件。", "一致するコンポーネントがありません。", "No matching components.", "일치하는 구성 요소가 없습니다.")
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
                        muted(crate::i18n::tr("该容器没有可用的组件目录。", "このコンテナで利用可能なコンポーネントカタログがありません。", "No component catalog available for this bottle.", "이 컨테이너에 사용 가능한 구성 요소 카탈로그가 없습니다."))
                            .py(px(10.))
                            .into_any_element(),
                    );
                }
                sections.push(self.section("tricks", crate::i18n::tr("可用组件", "利用可能なコンポーネント", "Available Components", "사용 가능한 구성 요소"), rows, cx));
            }
            None if self.winetricks.catalog_pending => {
                sections.push(self.section(
                    "trick-loading",
                    crate::i18n::tr("可用组件", "利用可能なコンポーネント", "Available Components", "사용 가능한 구성 요소"),
                    vec![
                        row()
                            .gap(px(8.))
                            .py(px(12.))
                            .child(Spinner::new().with_size(px(14.)))
                            .child(muted(crate::i18n::tr("正在读取组件目录…", "コンポーネントカタログを読み込み中…", "Loading component catalog…", "구성 요소 카탈로그 읽는 중…")))
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
                    .unwrap_or_else(|| crate::i18n::tr("尚未读取组件目录。", "コンポーネントカタログはまだ読み込まれていません。", "Component catalog has not been loaded yet.", "구성 요소 카탈로그를 아직 읽지 않았습니다.").to_owned());
                sections.push(self.section(
                    "trick-load-error",
                    crate::i18n::tr("可用组件", "利用可能なコンポーネント", "Available Components", "사용 가능한 구성 요소"),
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
                    .label(crate::i18n::tr("取消安装", "インストールをキャンセル", "Cancel Install", "설치 취소"))
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
                self.field("dependency-search", crate::i18n::tr("搜索组件", "コンポーネントを検索", "Search Components", "구성 요소 검색")),
                self.section(
                    "tricks",
                    crate::i18n::tr("可用组件", "利用可能なコンポーネント", "Available Components", "사용 가능한 구성 요소"),
                    [
                        ("vcrun2022", "Visual C++ 2015–2022"),
                        ("d3dx9", "DirectX 9"),
                        ("lavfilters", "LAV Filters"),
                        ("cjkfonts", crate::i18n::tr("中日韩字体", "CJK フォント", "CJK Fonts", "CJK 글꼴")),
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
            .filter_map(|&(source, label)| {
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
                crate::i18n::tr("尚未关联在线平台", "オンラインプラットフォーム未連携", "Not linked to an online source", "연결된 온라인 플랫폼 없음"),
                crate::theme::badge_bg(),
                crate::theme::warning_fg(),
            ));
        }
        if loaded {
            bound_pills.push(info_pill(
                match crate::i18n::locale() { crate::i18n::Locale::ZhHans => format!("默认来源：{}", source_label(&default_source)), crate::i18n::Locale::Ja => format!("既定の取得元：{}", source_label(&default_source)), crate::i18n::Locale::En => format!("Default source: {}", source_label(&default_source)), crate::i18n::Locale::Ko => format!("기본 출처: {}", source_label(&default_source)) },
                crate::theme::badge_bg(),
                crate::theme::badge_text(),
            ));
        }

        let hero_warning = if loaded && allowed.is_empty() {
            Some(crate::i18n::tr("未启用任何元数据平台，请在全局设置中开启来源后再搜索或刷新。", "メタデータソースが有効になっていません。設定で有効にしてから検索または更新してください。", "No metadata sources enabled. Enable sources in Settings before searching or refreshing.", "활성화된 메타데이터 플랫폼이 없습니다. 설정에서 소스를 켠 후 검색하거나 새로 고침하세요.").to_owned())
        } else if loaded && bindings[&basic_source].as_str().is_none_or(str::is_empty) {
            Some({
                let src = source_label(&basic_source);
                match crate::i18n::locale() {
                    crate::i18n::Locale::ZhHans => format!("当前基本信息来源「{src}」尚未关联作品，请在下方搜索或按 ID 关联。"),
                    crate::i18n::Locale::Ja => format!("現在の基本情報ソース「{src}」はまだ作品と紐付けられていません。下で検索または ID 連携してください。"),
                    crate::i18n::Locale::En => format!("Basic info source \"{src}\" is not linked to a title yet; search or link by ID below."),
                    crate::i18n::Locale::Ko => format!("현재 기본 정보 출처 '{src}'가 아직 작품과 연결되지 않았습니다. 아래에서 검색하거나 ID로 연결하세요."),
                }
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
                                            crate::i18n::tr("有未保存修改", "未保存の変更があります", "Unsaved changes", "저장되지 않은 변경 사항"),
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
                    .label(crate::i18n::tr("按 ID / 网址关联", "ID / URL で連携", "Link by ID / URL", "ID / URL로 연결"))
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
        for &(source, label) in crate::metadata_editor::PROVIDERS {
            let id = bindings[source].as_str().unwrap_or("");
            if !id.is_empty() {
                let snap_title = snapshots[source]["title"].as_str().unwrap_or("");
                let snap_title = if snap_title.is_empty() {
                    crate::i18n::tr("尚无快照 · 保存后刷新", "スナップショットなし · 保存後に更新", "No snapshot · save & refresh", "스냅샷 없음 · 저장 후 새로 고침")
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
                                .label(crate::i18n::tr("解除关联", "連携を解除", "Unlink", "연결 해제"))
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
                        muted(crate::i18n::tr("当前已关联（草稿）", "連携済み（下書き）", "Currently linked (draft)", "현재 연결됨(초안)"))
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
                    .child(muted(crate::i18n::tr("正在查询所选平台…", "選択したソースを検索中…", "Querying selected sources…", "선택한 플랫폼 조회 중…")))
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
                    muted(match crate::i18n::locale() { crate::i18n::Locale::ZhHans => format!("查询失败：{provider_error}"), crate::i18n::Locale::Ja => format!("照会に失敗：{provider_error}"), crate::i18n::Locale::En => format!("Query failed: {provider_error}"), crate::i18n::Locale::Ko => format!("조회 실패: {provider_error}") })
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
                                    crate::i18n::tr("✓ 已在草稿", "✓ 下書きに反映済み", "✓ In Draft", "✓ 초안에 적용됨")
                                } else {
                                    crate::i18n::tr("关联到草稿", "下書きへ連携", "Link to Draft", "초안에 연결")
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
                muted(crate::i18n::tr("输入目标平台上的名称搜索，或粘贴作品 ID / 网址直接关联；所有修改仅更新草稿，点击「保存资料」生效。", "タイトル名で検索するか作品 ID / URL を貼り付けて連携します。変更は下書きのみ更新され、「情報を保存」で確定します。", "Search by title on the source or paste a work ID/URL to link. Edits stay in the draft until you click Save Metadata.", "대상 플랫폼에서 제목으로 검색하거나 작품 ID/URL을 붙여 넣어 연결하세요. 저장을 눌러야 반영됩니다."))
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
                        .child(crate::i18n::tr("自定义封面", "カスタムカバー", "Custom Cover", "사용자 지정 커버"))
                        .child(
                            muted(crate::i18n::tr("立即生效（不属于资料草稿），本地图片优先级高于在线封面。", "即時反映されます（下書き対象外）。ローカル画像がオンラインカバーより優先されます。", "Takes effect immediately (outside the draft); local image takes priority over online covers.", "즉시 적용되며(초안 미포함) 로컬 이미지가 온라인 커버보다 우선합니다."))
                                .text_size(px(11.)),
                        ),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .child(self.action(
                            "cover-import",
                            crate::i18n::tr("选择本地图片…", "ローカル画像を選択…", "Choose Local Image…", "로컬 이미지 선택…"),
                            Command::MetadataCover,
                            cx,
                        ))
                        .child(self.action(
                            "cover-reset",
                            crate::i18n::tr("恢复默认封面", "デフォルトカバーに戻す", "Reset to Default Cover", "기본 커버로 복원"),
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
                    .child(Button::new("gamedb-retry").label(crate::i18n::tr("重新读取", "再読み込み", "Reload", "다시 읽기"))
                        .on_click(cx.listener(|this, _, _, cx| this.retry_game_db(cx))))
                    .into_any_element());
            }
            None => {
                config_rows.push(
                    row()
                        .gap(px(8.))
                        .py(px(8.))
                        .child(Spinner::new().with_size(px(14.)))
                        .child(muted(crate::i18n::tr("正在匹配本地 GameDB…", "ローカル GameDB と照合中…", "Matching local GameDB…", "로컬 GameDB 매칭 중…")))
                        .into_any_element(),
                );
            }
            Some(matches) if matches.is_empty() => {
                config_rows.push(
                    muted(crate::i18n::tr("本地 GameDB 中没有匹配条目。仍可使用在线元数据或手动调整运行设置。", "ローカル GameDB に一致する項目がありません。オンラインメタデータや手動設定をご利用ください。", "No matching entry in local GameDB. You can still use online metadata or adjust run settings manually.", "로컬 GameDB에 일치하는 항목이 없습니다. 온라인 메타데이터나 수동 설정을 이용하세요."))
                        .into_any_element(),
                );
            }
            Some(matches) => {
                for (index, matched) in matches.iter().enumerate() {
                    let entry_title = matched
                        .entry
                        .get("title")
                        .and_then(|value| value.as_str())
                        .unwrap_or(crate::i18n::tr("GameDB 条目", "GameDB 項目", "GameDB Entry", "GameDB 항목"));
                    let rating = matched
                        .entry
                        .get("rating")
                        .and_then(|value| value.as_str())
                        .unwrap_or("");
                    let pct = matched.confidence * 100.; let mut detail = match crate::i18n::locale() { crate::i18n::Locale::ZhHans => format!("{pct:.0}% 匹配"), crate::i18n::Locale::Ja => format!("一致度 {pct:.0}%"), crate::i18n::Locale::En => format!("{pct:.0}% match"), crate::i18n::Locale::Ko => format!("{pct:.0}% 일치") };
                    if matched.reference_platform == "macos" {
                        detail.push_str(crate::i18n::tr(" · macOS 参考记录", " · macOS 参考記録", " · macOS reference", " · macOS 참고 기록"));
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
                                        crate::i18n::tr("应用推荐配置", "推奨設定を適用", "Apply Recommended", "권장 설정 적용")
                                    } else { crate::i18n::tr("配置参考", "設定リファレンス", "Config Reference", "설정 참고") })
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
                self.section("game-metadata-search", crate::i18n::tr("在线匹配与关联", "オンライン検索と連携", "Online Match & Link", "온라인 검색 및 연결"), search_rows, cx),
                self.section("game-metadata-edit", crate::i18n::tr("手动编辑与封面", "手動編集とカバー", "Manual Edit & Cover", "수동 편집 및 커버"), edit_rows, cx),
                self.section("game-metadata-sources", crate::i18n::tr("来源策略与角色预览", "ソース方針とキャラクタープレビュー", "Sources & Character Preview", "출처 정책 및 캐릭터 미리보기"), source_rows, cx),
                self.section(
                    "game-gamedb",
                    if self.service_available("gameConfig.apply") {
                        crate::i18n::tr("GameDB 推荐配置", "GameDB 推奨設定", "GameDB Recommended Config", "GameDB 권장 설정")
                    } else {
                        crate::i18n::tr("GameDB 配置参考", "GameDB 設定リファレンス", "GameDB Config Reference", "GameDB 설정 참고")
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
                                self.info(crate::i18n::s().common.architecture, crate::i18n::s().common.bits_64),
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
                        self.info("d3dcompiler_47", crate::i18n::tr("原生优先，内建后备", "ネイティブ優先、内蔵フォールバック", "Native then Builtin", "네이티브 우선, 내장 후순위")),
                        self.field("dll-name", crate::i18n::tr("添加覆盖", "オーバーライドを追加", "Add Override", "재정의 추가")),
                        self.choice(
                            "dll-mode",
                            crate::i18n::tr("加载顺序", "読み込み順序", "Load Order", "로드 순서"),
                            &[
                                crate::i18n::tr("原生优先，内建后备", "ネイティブ優先、内蔵フォールバック", "Native then Builtin", "네이티브 우선, 내장 후순위"),
                                crate::i18n::tr("内建优先，原生后备", "内蔵優先、ネイティブフォールバック", "Builtin then Native", "내장 우선, 네이티브 후순위"),
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
                    crate::i18n::tr("图形调试", "グラフィックスデバッグ", "Graphics Debugging", "그래픽 디버깅"),
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
                    self.info(crate::i18n::tr("1. 输出设备", "1. 出力デバイス", "1. Output Device", "1. 출력 장치"), crate::i18n::tr("系统默认输出", "システム既定出力", "System Default Output", "시스템 기본 출력")),
                    self.info(crate::i18n::tr("2. 测试方式", "2. テスト方式", "2. Test Method", "2. 테스트 방식"), crate::i18n::tr("Wine WinMM 440 Hz 测试音", "Wine WinMM 440 Hz テストトーン", "Wine WinMM 440 Hz Test Tone", "Wine WinMM 440 Hz 테스트 톤")),
                    self.action(
                        "test-audio",
                        if self.audio_pending {
                            crate::i18n::tr("正在播放测试音…", "テストトーン再生中…", "Playing Test Tone…", "테스트 톤 재생 중…")
                        } else {
                            crate::i18n::tr("播放测试音频", "テスト音声を再生", "Play Test Audio", "테스트 오디오 재생")
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
                            .child(muted(crate::i18n::tr("正在启动 Wine 音频测试…", "Wine オーディオテストを開始中…", "Starting Wine audio test…", "Wine 오디오 테스트 시작 중…")))
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
                    rows.push(self.info(crate::i18n::tr("测试结果", "テスト結果", "Test Result", "테스트 결과"), status));
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
                                    .unwrap_or_else(|| crate::i18n::tr("检查 Wine 音频驱动设置", "Wine オーディオドライバ設定を確認してください", "Check Wine audio driver settings", "Wine 오디오 드라이버 설정 확인").to_owned()),
                            ),
                        );
                    }
                    rows.push(
                        muted(crate::i18n::tr("若能听到测试音，说明 Wine 音频链路正常；若没有声音，请检查系统输出设备与音频驱动。", "テストトーンが聞こえれば Wine オーディオは正常です。聞こえない場合は出力デバイスとオーディオドライバを確認してください。", "If you hear the tone, Wine audio output works; otherwise check your output device and audio driver.", "테스트 톤이 들리면 Wine 오디오가 정상입니다. 들리지 않으면 출력 장치와 오디오 드라이버를 확인하세요."))
                            .text_size(px(11.))
                            .into_any_element(),
                    );
                }
                self.form(
                    "audio-wizard-scroll",
                    vec![self.section("audio-check", crate::i18n::tr("检查音频输出", "オーディオ出力の確認", "Check Audio Output", "오디오 출력 확인"), rows, cx)],
                )
            }
            PreviewSheet::Diagnostics => {
                let symptoms: Vec<&'static str> = if cfg!(target_os = "linux") {
                    vec![
                        crate::i18n::tr("容器文件或导入异常", "コンテナまたはインポートエラー", "Bottle Files or Import Error", "컨테이너 또는 가져오기 오류"),
                        crate::i18n::tr("文件权限或存储空间", "ファイル権限または空き容量", "Permissions or Storage Space", "파일 권한 또는 저장 공간"),
                        crate::i18n::tr("主机环境信息", "ホスト環境情報", "Host Environment Info", "호스트 환경 정보"),
                    ]
                } else {
                    vec![
                        crate::i18n::tr("游戏无法启动", "ゲームが起動しない", "Game Won't Launch", "게임이 실행되지 않음"),
                        crate::i18n::tr("黑屏或画面异常", "画面が暗い／表示異常", "Black Screen or Glitches", "검은 화면 또는 그래픽 오류"),
                        crate::i18n::tr("没有声音", "音が出ない", "No Audio", "소리 없음"),
                        crate::i18n::tr("文字乱码", "文字化け", "Garbled Text", "문자 깨짐"),
                        crate::i18n::tr("游戏运行缓慢", "動作が重い", "Slow Performance", "성능 저하"),
                    ]
                };
                let mut rows = vec![
                    self.choice(
                        "diagnosis-symptom",
                        crate::i18n::tr("遇到了什么问题？", "どのような問題が発生していますか？", "What issue are you experiencing?", "어떤 문제가 발생했나요?"),
                        &symptoms,
                        cx,
                    ),
                    self.action(
                        "checks",
                        if self.checks_pending {
                            crate::i18n::tr("正在运行自动检查…", "自動チェックを実行中…", "Running Auto Checks…", "자동 검사 실행 중…")
                        } else {
                            crate::i18n::tr("运行自动检查", "自動チェックを実行", "Run Auto Checks", "자동 검사 실행")
                        },
                        Command::RunBottleChecks,
                        cx,
                    ),
                    self.action(
                        "analyze",
                        if self.diagnostics_pending {
                            crate::i18n::tr("正在收集诊断信息…", "診断情報を収集中…", "Collecting Diagnostics…", "진단 정보 수집 중…")
                        } else if cfg!(target_os = "linux") {
                            crate::i18n::tr("收集主机与容器信息", "ホストとコンテナ情報を収集", "Collect Host & Bottle Info", "호스트 및 컨테이너 정보 수집")
                        } else {
                            crate::i18n::tr("分析运行日志", "実行ログを分析", "Analyze Run Logs", "실행 로그 분석")
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
                            .child(muted(crate::i18n::tr("正在收集预检数据并运行检查项…", "プリフライトデータを収集してチェックを実行中…", "Collecting preflight data and running checks…", "사전 데이터 수집 및 검사 실행 중…")))
                            .into_any_element(),
                    );
                }
                if let Some(checks) = &self.diagnostic_checks {
                    for check in checks {
                        let c = &crate::i18n::s().common;
                        let (label, symbol, color) = match check.outcome.as_str() {
                            "pass" => (c.passed, "check", crate::theme::installed_fg()),
                            "already_configured" => (crate::i18n::tr("已配置", "設定済み", "Configured", "설정됨"), "check", crate::theme::installed_fg()),
                            "fail" => (crate::i18n::tr("未通过", "不合格", "Failed", "실패"), "triangle-alert", crate::theme::warning_fg()),
                            "warning" => (c.warning, "triangle-alert", crate::theme::warning_fg()),
                            "error" => (c.error, "x", crate::theme::danger_fg()),
                            _ => (c.unknown, "info", crate::theme::muted_color()),
                        };
                        let check_label = match check.check_id.as_str() {
                            "crash.log_classify" => crate::i18n::tr("崩溃日志", "クラッシュログ", "Crash Logs", "충돌 로그"),
                            "launcher.type_check" => crate::i18n::config::field_title("config.field.launcherMode", "launcherMode", "Launcher Type"),
                            "process.running_check" => crate::i18n::s().bottle.running_processes,
                            "audio.device_check" => crate::i18n::tr("音频设备", "オーディオデバイス", "Audio Device", "오디오 장치"),
                            "audio.driver_check" => crate::i18n::config::field_title("config.field.audioDriver", "audioDriver", "Audio Driver"),
                            "game.config_available" => crate::i18n::tr("GameDB 配置", "GameDB 設定", "GameDB Config", "GameDB 설정"),
                            "diagnostics.can_enhance" => crate::i18n::tr("诊断日志级别", "診断ログレベル", "Diagnostic Log Level", "진단 로그 수준"),
                            "host.container_metadata" => crate::i18n::tr("容器元数据", "コンテナメタデータ", "Bottle Metadata", "컨테이너 메타데이터"),
                            "host.drive_c" => crate::i18n::tr("容器目录", "コンテナフォルダ", "Bottle Directory", "컨테이너 폴더"),
                            "host.container_write" => crate::i18n::tr("文件权限", "ファイル権限", "File Permissions", "파일 권한"),
                            "host.storage" => crate::i18n::tr("可用空间", "空き容量", "Available Storage", "사용 가능 공간"),
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
                            .child(muted(crate::i18n::tr("正在读取系统信息与最近日志…", "システム情報と最近のログを読み込み中…", "Reading system info and recent logs…", "시스템 정보 및 최근 로그 읽는 중…")))
                            .into_any_element(),
                    );
                }
                if let Some(error) = &self.diagnostics_error {
                    rows.push(muted(error.clone()).into_any_element());
                }
                if let Some(report) = &self.diagnostics_report {
                    rows.push(
                        div()
                            .id("diagnostics-report")
                            .max_h(px(280.))
                            .overflow_y_scroll()
                            .p(px(10.))
                            .rounded(px(8.))
                            .bg(rgba(crate::theme::code_bg()))
                            .border_1()
                            .border_color(rgba(crate::theme::card_border()))
                            .text_size(px(11.))
                            .line_height(relative(1.45))
                            .child(report.clone())
                            .into_any_element(),
                    );
                    rows.push(self.action(
                        "export",
                        crate::i18n::tr("导出诊断报告…", "診断レポートを書き出す…", "Export Diagnostics Report…", "진단 보고서 내보내기…"),
                        Command::ExportDiagnostics,
                        cx,
                    ));
                }
                if cfg!(target_os = "macos") {
                    rows.push(self.action(
                        "audio-link",
                        crate::i18n::tr("音频故障排查…", "オーディオのトラブルシューティング…", "Audio Troubleshooter…", "오디오 문제 해결…"),
                        Command::Sheet(PreviewSheet::Audio),
                        cx,
                    ));
                }
                self.form(
                    "diagnostics-scroll",
                    vec![self.section(
                        "guided-diagnostics",
                        crate::i18n::tr("引导式故障排查", "ガイド付きトラブルシューティング", "Guided Diagnostics", "단계별 문제 해결"),
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
                                    .label(crate::i18n::tr("导入已有容器…", "既存ボトルをインポート…", "Import Existing Bottle…", "기존 컨테이너 가져오기…"))
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
                                                    match crate::i18n::locale() {
                                                        crate::i18n::Locale::ZhHans => format!("继续启动 ({n})"),
                                                        crate::i18n::Locale::Ja => format!("起動を続行 ({n})"),
                                                        crate::i18n::Locale::En => format!("Continue Launch ({n})"),
                                                        crate::i18n::Locale::Ko => format!("계속 실행 ({n})"),
                                                    }
                                                } else {
                                                    d.save_and_launch.to_owned()
                                                }
                                            } else {
                                                match crate::i18n::locale() {
                                                    crate::i18n::Locale::ZhHans => format!("保存入口 ({n})"),
                                                    crate::i18n::Locale::Ja => format!("エントリーを保存 ({n})"),
                                                    crate::i18n::Locale::En => format!("Save Entries ({n})"),
                                                    crate::i18n::Locale::Ko => format!("진입점 저장 ({n})"),
                                                }
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
            "downloading" => crate::i18n::tr("正在下载 Wine 运行时…", "Wine ランタイムをダウンロード中…", "Downloading Wine runtime…", "Wine 런타임 다운로드 중…"),
            "verifying" => crate::i18n::tr("正在校验下载完整性…", "ダウンロードの整合性を検証中…", "Verifying download…", "다운로드 무결성 검증 중…"),
            "installing" => crate::i18n::tr("正在解压并安装运行时…", "ランタイムを展開してインストール中…", "Extracting and installing runtime…", "런타임 압축 해제 및 설치 중…"),
            "rosetta" => crate::i18n::tr("正在安装 Rosetta 2…", "Rosetta 2 をインストール中…", "Installing Rosetta 2…", "Rosetta 2 설치 중…"),
            _ => crate::i18n::tr("正在准备…", "準備中…", "Preparing…", "준비 중…"),
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
                    .child(stage.unwrap_or(crate::i18n::tr("正在安装…", "インストール中…", "Installing…", "설치 중…")).to_owned())
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
            AboutTab::Acknowledgments => about_acknowledgments_tab(),
            AboutTab::License => about_document_tab(
                "about-license-scroll",
                crate::i18n::s().dialogs.about_license_header,
                GPL_LICENSE_TEXT,
                false,
            ),
            AboutTab::Notices => about_document_tab(
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

        col()
            .id("about-overview-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
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
            .child(legal_card)
            .into_any_element()
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
            return vec![muted(crate::i18n::tr("正在读取本地资料配置…", "ローカルのメタデータ設定を読み込み中…", "Loading local metadata config…", "로컬 메타데이터 설정 읽는 중…")).text_size(px(11.)).into_any_element()];
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
                        .label(crate::i18n::tr("手动编辑标题", "タイトルを手動編集", "Manually Edit Title", "제목 수동 편집"))
                        .checked(title_enabled)
                        .disabled(!editable)
                        .on_change(cx.listener(|this, enabled, _, cx| {
                            this.toggle_metadata_manual("manualTitle", *enabled, cx)
                        })),
                )
                .when(title_enabled, |d| d.child(Input::new(&title).disabled(!editable)))
                .child(
                    Switch::new("metadata-manual-summary")
                        .label(crate::i18n::tr("手动编辑简介", "概要を手動編集", "Manually Edit Summary", "줄거리 수동 편집"))
                        .checked(summary_enabled)
                        .disabled(!editable)
                        .on_change(cx.listener(|this, enabled, _, cx| {
                            this.toggle_metadata_manual("manualSummary", *enabled, cx)
                        })),
                )
                .when(summary_enabled, |d| d.child(Input::new(&summary).disabled(!editable)))
                .child(
                    muted(crate::i18n::tr("手动内容优先于平台资料；关闭开关可恢复来源内容，取消或关闭窗口将丢弃未保存修改。", "手動入力の内容がソース情報より優先されます。オフにすると取得元の内容に戻ります。キャンセルまたは閉じると未保存の変更は破棄されます。", "Manual text overrides source data; turn off to restore source content. Cancelling or closing discards unsaved changes.", "수동 입력 내용이 출처 데이터보다 우선합니다. 끄면 원본 내용이 복원되며 취소하거나 닫으면 저장되지 않은 변경 사항이 사라집니다."))
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
            muted(crate::i18n::tr("默认来源用于所有字段；仅需对个别字段使用不同平台时再设置分类例外。", "既定のソースが全項目に適用されます。特定項目だけ別ソースを使う場合にカテゴリ例外を設定してください。", "The default source applies to all fields; add per-category overrides only when needed.", "기본 출처가 모든 필드에 적용되며, 특정 필드에만 다른 출처를 쓸 때 분류 예외를 설정하세요."))
                .text_size(px(11.))
                .line_height(relative(1.4))
                .into_any_element(),
            self.metadata_policy_row("default", crate::i18n::s().onboarding.default_source, false, cx),
        ];

        let category_cell = |category: &'static str, label: &'static str| -> AnyElement {
            if category == "characters"
                && !crate::metadata_editor::PROVIDERS.iter().any(|(source, _)| {
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
                    muted(crate::i18n::tr("分类例外（默认跟随上方来源）", "カテゴリ別オーバーライド（既定は上のソースを継承）", "Category Overrides (Follow Default Above)", "분류별 예외(기본값은 위 출처 따름)"))
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
                            muted(match crate::i18n::locale() { crate::i18n::Locale::ZhHans => format!("角色资料 · {}", source_label(character_source)), crate::i18n::Locale::Ja => format!("キャラクター · {}", source_label(character_source)), crate::i18n::Locale::En => format!("Characters · {}", source_label(character_source)), crate::i18n::Locale::Ko => format!("등장인물 · {}", source_label(character_source)) })
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
                            let name = character["name"].as_str().unwrap_or(crate::i18n::tr("未命名", "名称未設定", "Unnamed", "이름 없음"));
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
                        muted(crate::i18n::tr("该作品未返回角色资料。", "この作品のキャラクター情報は返されませんでした。", "No character data returned for this title.", "이 작품의 캐릭터 정보가 없습니다."))
                            .text_size(px(11.))
                            .into_any_element(),
                    );
                }
                None => {
                    rows.push(
                        muted(crate::i18n::tr("尚无角色快照；关联作品并保存后，可点击左下角「刷新已保存的关联」获取角色。", "キャラクタースナップショットがありません。作品を連携・保存後、左下の「保存済みの連携を更新」で取得できます。", "No character snapshot yet; link and save, then click Refresh Saved Links in the bottom-left.", "캐릭터 스냅샷이 없습니다. 작품을 연결하고 저장한 뒤 왼쪽 아래 \"저장된 연결 새로고침\"을 누르세요."))
                            .text_size(px(11.))
                            .into_any_element(),
                    );
                }
            }
        } else if page.editor.loaded {
            rows.push(
                muted({
                    let src = source_label(character_source);
                    match crate::i18n::locale() {
                        crate::i18n::Locale::ZhHans => format!("当前角色来源「{src}」不支持角色资料；可从上方选择支持的分类来源。"),
                        crate::i18n::Locale::Ja => format!("現在のキャラクターソース「{src}」はキャラクター取得に対応していません。上から対応ソースを選択してください。"),
                        crate::i18n::Locale::En => format!("Character source \"{src}\" does not provide characters; pick a supported source above."),
                        crate::i18n::Locale::Ko => format!("현재 등장인물 출처 '{src}'는 캐릭터 정보를 지원하지 않습니다. 위에서 지원하는 출처를 선택하세요."),
                    }
                })
                .text_size(px(11.))
                .into_any_element(),
            );
        }

        if self.metadata_editor_dirty(cx) {
            rows.push(
                muted(crate::i18n::tr("存在未保存修改；请先点击「保存资料」，再执行按平台 ID 刷新。", "未保存の変更があります。先に「情報を保存」してからプラットフォーム ID で更新してください。", "Unsaved changes present; click Save Metadata before refreshing bound links.", "저장되지 않은 변경 사항이 있습니다. 먼저 정보 저장을 클릭한 후 새로 고침하세요."))
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
                    if inherit {
                        let view = view.clone();
                        menu = menu.item(PopupMenuItem::new(crate::i18n::s().onboarding.follow_default).on_click(move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| this.set_metadata_source(category, "", cx));
                        }));
                    }
                    for &(source, label) in crate::metadata_editor::PROVIDERS {
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
            muted(crate::i18n::tr("搜索平台", "検索ソース", "Search Sources", "검색 출처")).text_size(px(11.5)).mr(px(4.)),
        );
        for &(source, label) in crate::metadata_editor::PROVIDERS {
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
        "unverified" => crate::i18n::tr("待验证", "未検証", "Unverified", "미검증"),
        "verified" => crate::i18n::tr("已验证", "検証済み", "Verified", "검증됨"),
        "playable" => crate::i18n::tr("可玩", "プレイ可能", "Playable", "플레이 가능"),
        "runs" => crate::i18n::tr("可运行", "起動可能", "Runs", "실행 가능"),
        "unplayable" => crate::i18n::tr("不可玩", "動作不可", "Unplayable", "실행 불가"),
        _ => rating,
    }
}

/// Bytes as the sheet shows them. The helper reports the raw byte count.
fn clipboard_size(bytes: i64) -> String {
    if bytes <= 0 {
        return crate::i18n::tr("大小未知", "サイズ不明", "Unknown size", "크기 알 수 없음").to_owned();
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
                match crate::i18n::locale() {
                    crate::i18n::Locale::ZhHans => format!("已配置 {n} 个启动入口，每次启动将提示选择"),
                    crate::i18n::Locale::Ja => format!("{n} 個の起動エントリーを設定済み（起動時に毎回選択）"),
                    crate::i18n::Locale::En => format!("{n} launch entries configured; prompts on launch"),
                    crate::i18n::Locale::Ko => format!("{n}개 실행 진입점 설정됨 (실행 시 선택 표시)"),
                }
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
                        muted(match crate::i18n::locale() { crate::i18n::Locale::ZhHans => format!("当前启动程序：{label}"), crate::i18n::Locale::Ja => format!("現在の起動プログラム：{label}"), crate::i18n::Locale::En => format!("Current launch program: {label}"), crate::i18n::Locale::Ko => format!("현재 실행 프로그램: {label}") })
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
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
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
                    .child(list)
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

fn about_acknowledgments_tab() -> AnyElement {
    col()
        .id("about-ack-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .px(px(20.))
        .py(px(16.))
        .gap(px(10.))
        .child(
            muted(crate::i18n::s().dialogs.about_ack_intro)
                .text_size(px(11.5)),
        )
        .children(ACKNOWLEDGMENTS.iter().map(about_ack_card))
        .into_any_element()
}

fn about_document_tab(
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

    col()
        .id(scroll_id)
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
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
        )
        .into_any_element()
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
