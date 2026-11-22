//! Bottle configuration schema and Wine engine feature localization.

use super::{Locale, locale, tr};

/// Localized bottle configuration section title for `section_id`.
pub(crate) fn section_title(section_id: &str) -> &'static str {
    match section_id {
        "wine" => "Wine",
        "launcher" => tr("启动器兼容性", "ランチャー互換", "Launcher Compatibility", "런처 호환성"),
        "input" => tr("输入与控制器", "入力・コントローラー", "Input & Controllers", "입력 및 컨트롤러"),
        "graphics" => tr("图形", "グラフィックス", "Graphics", "그래픽"),
        "upscaling" => tr("超分", "超解像", "Upscaling", "업스케일링"),
        "audio" => tr("音频", "オーディオ", "Audio", "오디오"),
        "discord" => "Discord",
        "display" => tr("显示", "ディスプレイ", "Display", "디스플레이"),
        "performance" => tr("性能", "パフォーマンス", "Performance", "성능"),
        "cleanup" => tr("退出与清理", "終了とクリーンアップ", "Exit & Cleanup", "종료 및 정리"),
        "program" => tr("程序配置", "プログラム設定", "Program Configuration", "프로그램 설정"),
        _ => tr("其他", "その他", "Other", "기타"),
    }
}

/// Localized bottle configuration field title, falling back to the wire `fallback`.
pub(crate) fn field_title<'a>(title_key: &str, id: &str, fallback: &'a str) -> &'a str {
    let key = if !title_key.is_empty() {
        title_key
    } else {
        match id {
            "windowsVersion" => "config.field.windowsVersion",
            "enhancedSync" => "config.field.enhancedSync",
            "asfTimestampBase" => "config.field.asfTimestampBase",
            "avxEnabled" => "config.field.avxEnabled",
            "launcherCompatibilityMode" => "config.field.launcherCompatibilityMode",
            "launcherMode" => "config.field.launcherMode",
            "detectedLauncher" => "config.field.detectedLauncher",
            "launcherLocale" => "config.field.launcherLocale",
            "gpuSpoofing" => "config.field.gpuSpoofing",
            "gpuVendor" => "config.field.gpuVendor",
            "autoEnableDXVK" => "config.field.autoEnableDXVK",
            "controllerCompatibilityMode" => "config.field.controllerCompatibilityMode",
            "disableHIDAPI" => "config.field.disableHIDAPI",
            "allowBackgroundEvents" => "config.field.allowBackgroundEvents",
            "useButtonLabels" => "config.field.useButtonLabels",
            "commandActsAsControl" => "config.field.commandActsAsControl",
            "graphicsBackend" => "config.field.graphicsBackend",
            "frameGeneration" => "config.field.frameGeneration",
            "hostMemoryImport" => "config.field.hostMemoryImport",
            "shadowImportedHostMemory" => "config.field.shadowImportedHostMemory",
            "metal4Enabled" => "config.field.metal4Enabled",
            "metalHud" => "config.field.metalHud",
            "metalTrace" => "config.field.metalTrace",
            "dxrEnabled" => "config.field.dxrEnabled",
            "metalValidation" => "config.field.metalValidation",
            "dxvkAsync" => "config.field.dxvkAsync",
            "dxvkHud" => "config.field.dxvkHud",
            "metalFX" => "config.field.metalFX",
            "metalFXSpatial" => "config.field.metalFXSpatial",
            "gdiUpscaler" => "config.field.gdiUpscaler",
            "gdiUpscalerExcludedRegions" => "config.field.gdiUpscalerExcludedRegions",
            "gdiUpscalerCache" => "config.field.gdiUpscalerCache",
            "audioDriver" => "config.field.audioDriver",
            "audioLatencyPreset" => "config.field.audioLatencyPreset",
            "discordPresence" => "config.field.discordPresence",
            "discordBridge" => "config.field.discordBridge",
            "virtualDesktopEnabled" => "config.field.virtualDesktopEnabled",
            "resolutionPreset" => "config.field.resolutionPreset",
            "customResolutionWidth" => "config.field.customResolutionWidth",
            "customResolutionHeight" => "config.field.customResolutionHeight",
            "shaderCacheEnabled" => "config.field.shaderCacheEnabled",
            "disableAppNap" => "config.field.disableAppNap",
            "clipboardPolicy" => "config.field.clipboardPolicy",
            "clipboardThreshold" => "config.field.clipboardThreshold",
            "killOnQuit" => "config.field.killOnQuit",
            _ => "",
        }
    };
    let loc = locale();
    match key {
        "config.field.windowsVersion" => match loc {
            Locale::ZhHans => "Windows 版本",
            Locale::Ja => "Windows バージョン",
            Locale::En => "Windows Version",
            Locale::Ko => "Windows 버전",
        },
        "config.field.enhancedSync" => match loc {
            Locale::ZhHans => "增强同步",
            Locale::Ja => "拡張同期",
            Locale::En => "Enhanced Sync",
            Locale::Ko => "향상된 동기화",
        },
        "config.field.asfTimestampBase" => match loc {
            Locale::ZhHans => "ASF/WMV 时间戳同步",
            Locale::Ja => "ASF/WMV タイムスタンプ同期",
            Locale::En => "ASF/WMV Timestamp Sync",
            Locale::Ko => "ASF/WMV 타임스탬프 동기화",
        },
        "config.field.avxEnabled" => match loc {
            Locale::ZhHans => "AVX 支持",
            Locale::Ja => "AVX サポート",
            Locale::En => "AVX Support",
            Locale::Ko => "AVX 지원",
        },
        "config.field.launcherCompatibilityMode" => match loc {
            Locale::ZhHans => "启动器兼容模式",
            Locale::Ja => "ランチャー互換モード",
            Locale::En => "Launcher Compatibility Mode",
            Locale::Ko => "런처 호환성 모드",
        },
        "config.field.launcherMode" => match loc {
            Locale::ZhHans => "检测模式",
            Locale::Ja => "検出モード",
            Locale::En => "Detection Mode",
            Locale::Ko => "감지 모드",
        },
        "config.field.detectedLauncher" => match loc {
            Locale::ZhHans => "启动器类型",
            Locale::Ja => "ランチャーの種類",
            Locale::En => "Launcher Type",
            Locale::Ko => "런처 유형",
        },
        "config.field.launcherLocale" => match loc {
            Locale::ZhHans => "启动器语言区域",
            Locale::Ja => "ランチャーロケール",
            Locale::En => "Launcher Locale",
            Locale::Ko => "런처 로캘",
        },
        "config.field.gpuSpoofing" => match loc {
            Locale::ZhHans => "GPU 伪装",
            Locale::Ja => "GPU スプーフィング",
            Locale::En => "GPU Spoofing",
            Locale::Ko => "GPU 스푸핑",
        },
        "config.field.gpuVendor" => match loc {
            Locale::ZhHans => "GPU 厂商",
            Locale::Ja => "GPU ベンダー",
            Locale::En => "GPU Vendor",
            Locale::Ko => "GPU 제조사",
        },
        "config.field.autoEnableDXVK" => match loc {
            Locale::ZhHans => "为 Rockstar 启动器使用 DXVK",
            Locale::Ja => "Rockstar Launcher で DXVK を使用",
            Locale::En => "Use DXVK for Rockstar Launcher",
            Locale::Ko => "Rockstar 런처에 DXVK 사용",
        },
        "config.field.controllerCompatibilityMode" => match loc {
            Locale::ZhHans => "控制器兼容模式",
            Locale::Ja => "コントローラー互換モード",
            Locale::En => "Controller Compatibility Mode",
            Locale::Ko => "컨트롤러 호환성 모드",
        },
        "config.field.disableHIDAPI" => match loc {
            Locale::ZhHans => "停用 HIDAPI",
            Locale::Ja => "HIDAPI を無効化",
            Locale::En => "Disable HIDAPI",
            Locale::Ko => "HIDAPI 비활성화",
        },
        "config.field.allowBackgroundEvents" => match loc {
            Locale::ZhHans => "允许后台输入",
            Locale::Ja => "バックグラウンド入力を許可",
            Locale::En => "Allow Background Input",
            Locale::Ko => "백그라운드 입력 허용",
        },
        "config.field.useButtonLabels" => match loc {
            Locale::ZhHans => "使用原生按钮标签",
            Locale::Ja => "ネイティブのボタンラベルを使用",
            Locale::En => "Use Native Button Labels",
            Locale::Ko => "네이티브 버튼 레이블 사용",
        },
        "config.field.commandActsAsControl" => match loc {
            Locale::ZhHans => "将 Command 映射为 Windows Ctrl",
            Locale::Ja => "Command キーを Windows の Ctrl にマップ",
            Locale::En => "Map Command to Windows Ctrl",
            Locale::Ko => "Command 키를 Windows Ctrl로 매핑",
        },
        "config.field.graphicsBackend" => match loc {
            Locale::ZhHans => "图形后端",
            Locale::Ja => "グラフィックスバックエンド",
            Locale::En => "Graphics Backend",
            Locale::Ko => "그래픽 백엔드",
        },
        "config.field.frameGeneration" => match loc {
            Locale::ZhHans => "帧生成",
            Locale::Ja => "フレーム生成",
            Locale::En => "Frame Generation",
            Locale::Ko => "프레임 생성",
        },
        "config.field.hostMemoryImport" | "config.field.shadowImportedHostMemory" => match loc {
            Locale::ZhHans => "32 位 Vulkan 内存导入兼容（重启 Steam／游戏生效）",
            Locale::Ja => "32 ビット Vulkan メモリインポート互換（Steam/ゲーム再起動後に有効）",
            Locale::En => "32-bit Vulkan Memory Import Compatibility (Restart Steam/Game)",
            Locale::Ko => "32비트 Vulkan 메모리 가져오기 호환(Steam/게임 재시작 시 적용)",
        },
        "config.field.metal4Enabled" => "Metal 4",
        "config.field.metalHud" => match loc {
            Locale::ZhHans => "Metal 性能面板",
            Locale::Ja => "Metal パフォーマンス HUD",
            Locale::En => "Metal HUD",
            Locale::Ko => "Metal 성능 HUD",
        },
        "config.field.metalTrace" => match loc {
            Locale::ZhHans => "Metal 跟踪",
            Locale::Ja => "Metal トレース",
            Locale::En => "Metal Trace",
            Locale::Ko => "Metal 트레이스",
        },
        "config.field.dxrEnabled" => match loc {
            Locale::ZhHans => "DirectX 光线追踪",
            Locale::Ja => "DirectX レイトレーシング",
            Locale::En => "DirectX Raytracing",
            Locale::Ko => "DirectX 레이 트레이싱",
        },
        "config.field.metalValidation" => match loc {
            Locale::ZhHans => "Metal 验证",
            Locale::Ja => "Metal バリデーション",
            Locale::En => "Metal Validation",
            Locale::Ko => "Metal 유효성 검사",
        },
        "config.field.dxvkAsync" => match loc {
            Locale::ZhHans => "DXVK 异步编译",
            Locale::Ja => "DXVK 非同期コンパイル",
            Locale::En => "DXVK Async Compilation",
            Locale::Ko => "DXVK 비동기 컴파일",
        },
        "config.field.dxvkHud" => match loc {
            Locale::ZhHans => "DXVK 性能面板",
            Locale::Ja => "DXVK パフォーマンス HUD",
            Locale::En => "DXVK HUD",
            Locale::Ko => "DXVK 성능 HUD",
        },
        "config.field.gdiUpscaler" => match loc {
            Locale::ZhHans => "游戏超分算法",
            Locale::Ja => "ゲーム超解像アルゴリズム",
            Locale::En => "Game Upscaler",
            Locale::Ko => "게임 업스케일러",
        },
        "config.field.gdiUpscalerCache" => match loc {
            Locale::ZhHans => "GDI 超分缓存",
            Locale::Ja => "GDI 超解像キャッシュ",
            Locale::En => "GDI Upscaler Cache",
            Locale::Ko => "GDI 업스케일러 캐시",
        },
        "config.field.gdiUpscalerExcludedRegions" => match loc {
            Locale::ZhHans => "超分屏蔽区域",
            Locale::Ja => "超解像の除外領域",
            Locale::En => "Upscaler Excluded Regions",
            Locale::Ko => "업스케일링 제외 영역",
        },
        "config.field.metalFX" => match loc {
            Locale::ZhHans => "DLSS→MetalFX 桥（仅 D3DMetal）",
            Locale::Ja => "DLSS→MetalFX ブリッジ（D3DMetal 専用）",
            Locale::En => "DLSS→MetalFX Bridge (D3DMetal Only)",
            Locale::Ko => "DLSS→MetalFX 브리지(D3DMetal 전용)",
        },
        "config.field.metalFXSpatial" => match loc {
            Locale::ZhHans => "MetalFX Spatial 超分",
            Locale::Ja => "MetalFX Spatial 超解像",
            Locale::En => "MetalFX Spatial Upscaling",
            Locale::Ko => "MetalFX Spatial 업스케일링",
        },
        "config.field.audioDriver" => match loc {
            Locale::ZhHans => "音频驱动",
            Locale::Ja => "オーディオドライバー",
            Locale::En => "Audio Driver",
            Locale::Ko => "오디오 드라이버",
        },
        "config.field.audioLatencyPreset" => match loc {
            Locale::ZhHans => "延迟预设",
            Locale::Ja => "レイテンシプリセット",
            Locale::En => "Latency Preset",
            Locale::Ko => "지연 시간 프리셋",
        },
        "config.field.discordPresence" => match loc {
            Locale::ZhHans => "发布当前游戏状态",
            Locale::Ja => "プレイ中のゲームステータスを公開",
            Locale::En => "Publish Current Game Activity",
            Locale::Ko => "현재 게임 활동 게시",
        },
        "config.field.discordBridge" => match loc {
            Locale::ZhHans => "转发游戏 Rich Presence",
            Locale::Ja => "ゲームの Rich Presence を転送",
            Locale::En => "Forward Game Rich Presence",
            Locale::Ko => "게임 Rich Presence 전달",
        },
        "config.field.virtualDesktopEnabled" => match loc {
            Locale::ZhHans => "虚拟桌面",
            Locale::Ja => "仮想デスクトップ",
            Locale::En => "Virtual Desktop",
            Locale::Ko => "가상 데스크톱",
        },
        "config.field.resolutionPreset" => match loc {
            Locale::ZhHans => "分辨率",
            Locale::Ja => "解像度",
            Locale::En => "Resolution",
            Locale::Ko => "해상도",
        },
        "config.field.customResolutionWidth" => match loc {
            Locale::ZhHans => "自定义宽度",
            Locale::Ja => "カスタム幅",
            Locale::En => "Custom Width",
            Locale::Ko => "사용자 지정 너비",
        },
        "config.field.customResolutionHeight" => match loc {
            Locale::ZhHans => "自定义高度",
            Locale::Ja => "カスタム高さ",
            Locale::En => "Custom Height",
            Locale::Ko => "사용자 지정 높이",
        },
        "config.field.shaderCacheEnabled" => match loc {
            Locale::ZhHans => "着色器缓存",
            Locale::Ja => "シェーダーキャッシュ",
            Locale::En => "Shader Cache",
            Locale::Ko => "셰이더 캐시",
        },
        "config.field.disableAppNap" => match loc {
            Locale::ZhHans => "防止 App Nap 限制性能",
            Locale::Ja => "App Nap によるパフォーマンス低下を防止",
            Locale::En => "Prevent App Nap Throttling",
            Locale::Ko => "App Nap 성능 제한 방지",
        },
        "config.field.clipboardPolicy" => match loc {
            Locale::ZhHans => "剪贴板策略",
            Locale::Ja => "クリップボードポリシー",
            Locale::En => "Clipboard Policy",
            Locale::Ko => "클립보드 정책",
        },
        "config.field.clipboardThreshold" => match loc {
            Locale::ZhHans => "剪贴板大小阈值（字节）",
            Locale::Ja => "クリップボードサイズ閾値（バイト）",
            Locale::En => "Clipboard Size Threshold (Bytes)",
            Locale::Ko => "클립보드 크기 임계값(바이트)",
        },
        "config.field.killOnQuit" => match loc {
            Locale::ZhHans => "退出时结束进程",
            Locale::Ja => "終了時にプロセスを強制終了",
            Locale::En => "Terminate Processes on Quit",
            Locale::Ko => "종료 시 프로세스 강제 종료",
        },
        _ => fallback,
    }
}

/// Localized bottle configuration choice option title.
pub(crate) fn option_title<'a>(
    field_id: &str,
    option_id: &str,
    fallback: &'a str,
) -> &'a str {
    let loc = locale();
    match (field_id, option_id) {
        ("gdiUpscalerCache", "off") => match loc {
            Locale::ZhHans => "关闭跨场景缓存",
            Locale::Ja => "シーン間キャッシュをオフ",
            Locale::En => "Disable Cross-Scene Cache",
            Locale::Ko => "장면 간 캐시 끄기",
        },
        ("gdiUpscalerCache", "memory") => match loc {
            Locale::ZhHans => "本次游戏缓存",
            Locale::Ja => "セッション内メモリキャッシュ",
            Locale::En => "Session Memory Cache",
            Locale::Ko => "세션 메모리 캐시",
        },
        ("gdiUpscalerCache", "persistent") => match loc {
            Locale::ZhHans => "缓存并保存到磁盘",
            Locale::Ja => "キャッシュしてディスクに保存",
            Locale::En => "Cache & Save to Disk",
            Locale::Ko => "캐시 및 디스크에 저장",
        },
        ("launcherLocale", "") => match loc {
            Locale::ZhHans => "自动",
            Locale::Ja => "自動",
            Locale::En => "Auto",
            Locale::Ko => "자동",
        },
        ("launcherMode", "auto") | ("detectedLauncher", "") => match loc {
            Locale::ZhHans => "自动检测",
            Locale::Ja => "自動検出",
            Locale::En => "Auto Detect",
            Locale::Ko => "자동 감지",
        },
        ("launcherMode", "manual") => match loc {
            Locale::ZhHans => "手动选择",
            Locale::Ja => "手動選択",
            Locale::En => "Manual",
            Locale::Ko => "수동 선택",
        },
        ("asfTimestampBase", "auto") => match loc {
            Locale::ZhHans => "自动",
            Locale::Ja => "自動",
            Locale::En => "Auto",
            Locale::Ko => "자동",
        },
        ("asfTimestampBase", "first") => match loc {
            Locale::ZhHans => "首帧基准",
            Locale::Ja => "先頭フレーム基準",
            Locale::En => "First Frame",
            Locale::Ko => "첫 프레임 기준",
        },
        ("asfTimestampBase", "legacy") => match loc {
            Locale::ZhHans => "传统模式",
            Locale::Ja => "レガシー",
            Locale::En => "Legacy",
            Locale::Ko => "레거시",
        },
        (_, "none") | (_, "off") | (_, "disabled") => match loc {
            Locale::ZhHans => "关闭",
            Locale::Ja => "オフ",
            Locale::En => "Off",
            Locale::Ko => "끄기",
        },
        (_, "auto") => match loc {
            Locale::ZhHans => "自动",
            Locale::Ja => "自動",
            Locale::En => "Auto",
            Locale::Ko => "자동",
        },
        (_, "recommended") => match loc {
            Locale::ZhHans => "推荐",
            Locale::Ja => "推奨",
            Locale::En => "Recommended",
            Locale::Ko => "권장",
        },
        (_, "fps") => match loc {
            Locale::ZhHans => "帧率",
            Locale::Ja => "FPS のみ",
            Locale::En => "FPS",
            Locale::Ko => "FPS",
        },
        (_, "partial") => match loc {
            Locale::ZhHans => "简要",
            Locale::Ja => "簡易",
            Locale::En => "Partial",
            Locale::Ko => "간략히",
        },
        (_, "full") => match loc {
            Locale::ZhHans => "完整",
            Locale::Ja => "詳細",
            Locale::En => "Full",
            Locale::Ko => "전체",
        },
        ("gdiUpscaler", "adaptive") => match loc {
            Locale::ZhHans => "动态 MetalFX／静态 Real-CUGAN",
            Locale::Ja => "動的 MetalFX／静的 Real-CUGAN",
            Locale::En => "Dynamic MetalFX / Static Real-CUGAN",
            Locale::Ko => "동적 MetalFX / 정적 Real-CUGAN",
        },
        ("gdiUpscaler", "adaptive-realesrgan") => match loc {
            Locale::ZhHans => "动态 MetalFX／静态 AnimeVideo-v3",
            Locale::Ja => "動的 MetalFX／静的 AnimeVideo-v3",
            Locale::En => "Dynamic MetalFX / Static AnimeVideo-v3",
            Locale::Ko => "동적 MetalFX / 정적 AnimeVideo-v3",
        },
        ("gdiUpscaler", "adaptive-anime4k") => match loc {
            Locale::ZhHans => "动态 Anime4K／静态 Real-CUGAN",
            Locale::Ja => "動的 Anime4K／静的 Real-CUGAN",
            Locale::En => "Dynamic Anime4K / Static Real-CUGAN",
            Locale::Ko => "동적 Anime4K / 정적 Real-CUGAN",
        },
        ("gdiUpscaler", "adaptive-anime4k-realesrgan") => match loc {
            Locale::ZhHans => "动态 Anime4K／静态 AnimeVideo-v3",
            Locale::Ja => "動的 Anime4K／静的 AnimeVideo-v3",
            Locale::En => "Dynamic Anime4K / Static AnimeVideo-v3",
            Locale::Ko => "동적 Anime4K / 정적 AnimeVideo-v3",
        },
        ("audioLatencyPreset", "defaultPreset") => match loc {
            Locale::ZhHans => "默认",
            Locale::Ja => "デフォルト",
            Locale::En => "Default",
            Locale::Ko => "기본값",
        },
        ("audioLatencyPreset", "lowLatency") => match loc {
            Locale::ZhHans => "低延迟",
            Locale::Ja => "低遅延",
            Locale::En => "Low Latency",
            Locale::Ko => "저지연",
        },
        ("audioLatencyPreset", "stable") => match loc {
            Locale::ZhHans => "稳定",
            Locale::Ja => "安定重視",
            Locale::En => "Stable",
            Locale::Ko => "안정적",
        },
        ("resolutionPreset", "matchDisplay") => match loc {
            Locale::ZhHans => "匹配显示器",
            Locale::Ja => "ディスプレイに合わせる",
            Locale::En => "Match Display",
            Locale::Ko => "디스플레이에 맞춤",
        },
        ("resolutionPreset", "custom") => match loc {
            Locale::ZhHans => "自定义",
            Locale::Ja => "カスタム",
            Locale::En => "Custom",
            Locale::Ko => "사용자 지정",
        },
        ("clipboardPolicy", "warn") => match loc {
            Locale::ZhHans => "始终询问",
            Locale::Ja => "毎回確認",
            Locale::En => "Always Ask",
            Locale::Ko => "항상 확인",
        },
        ("clipboardPolicy", "clear") => match loc {
            Locale::ZhHans => "始终清理",
            Locale::Ja => "常にクリア",
            Locale::En => "Always Clear",
            Locale::Ko => "항상 지우기",
        },
        ("clipboardPolicy", "never") => match loc {
            Locale::ZhHans => "保留内容",
            Locale::Ja => "内容を保持",
            Locale::En => "Keep Contents",
            Locale::Ko => "내용 유지",
        },
        ("killOnQuit", "inherit") => match loc {
            Locale::ZhHans => "跟随应用设置",
            Locale::Ja => "アプリ設定に従う",
            Locale::En => "Follow App Settings",
            Locale::Ko => "앱 설정 따름",
        },
        ("killOnQuit", "always") => match loc {
            Locale::ZhHans => "始终结束",
            Locale::Ja => "常に終了",
            Locale::En => "Always Terminate",
            Locale::Ko => "항상 종료",
        },
        ("killOnQuit", "never") => match loc {
            Locale::ZhHans => "保留进程",
            Locale::Ja => "プロセスを維持",
            Locale::En => "Keep Running",
            Locale::Ko => "프로세스 유지",
        },
        _ => fallback,
    }
}

/// Localized Wine engine feature name.
pub(crate) fn engine_feature_label(id: &str) -> &str {
    match id {
        "dxvk" => "DXVK",
        "dxmt" => "DXMT",
        "d3dMetal" => "D3DMetal",
        "esync" => "ESync",
        "msync" => "MSync",
        "avx" => "AVX",
        "gdiUpscaling" => tr("GDI 超分", "GDI 超解像", "GDI Upscaling", "GDI 업스케일링"),
        "gdiCoreML" => "GDI CoreML",
        "metal" => tr("Metal 选项", "Metal オプション", "Metal Options", "Metal 옵션"),
        "metalFX" => tr("MetalFX 桥", "MetalFX ブリッジ", "MetalFX Bridge", "MetalFX 브리지"),
        "frameGeneration" => tr("帧生成", "フレーム生成", "Frame Generation", "프레임 생성"),
        "shadowImportedHostMemory" => tr(
            "32 位 Vulkan 内存导入",
            "32 ビット Vulkan メモリインポート",
            "32-bit Vulkan Memory Import",
            "32비트 Vulkan 메모리 가져오기",
        ),
        "dxvkAsync" => "DXVK Async",
        _ => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Locale, LocaleGuard};

    #[test]
    fn config_sections_fields_options_and_features_localize_across_all_locales() {
        let mut titles_by_locale = Vec::new();
        for loc in Locale::ALL {
            let _guard = LocaleGuard::new(loc);
            let sec = section_title("graphics");
            let fld = field_title("config.field.dxvk", "dxvk", "DXVK");
            let opt = option_title("killOnQuit", "inherit", "Inherit");
            let feat = engine_feature_label("gdiUpscaling");
            assert!(!sec.is_empty());
            assert!(!fld.is_empty());
            assert!(!opt.is_empty());
            assert!(!feat.is_empty());
            titles_by_locale.push((sec, opt, feat));
        }
        // All 4 locales produce distinct section, option, and feature labels.
        let sections: std::collections::HashSet<_> = titles_by_locale.iter().map(|t| t.0).collect();
        let options: std::collections::HashSet<_> = titles_by_locale.iter().map(|t| t.1).collect();
        let features: std::collections::HashSet<_> = titles_by_locale.iter().map(|t| t.2).collect();
        assert_eq!(sections.len(), 4);
        assert_eq!(options.len(), 4);
        assert_eq!(features.len(), 4);
        assert_eq!(section_title("unknown_id"), section_title("other"));
        assert_eq!(
            option_title("unknownField", "unknownOpt", "Raw Fallback"),
            "Raw Fallback"
        );
    }
}
