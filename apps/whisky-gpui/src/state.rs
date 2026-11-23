//! Application state: the root view model, page/sheet enums, and navigation.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use gpui_kit::base::Transition;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{prelude::*, *};
use serde_json::json;

use crate::bridge::dto::{
    AudioResultDto, CandidateDto, EngineFieldPolicyDto, FoldersDto, GameDbMatchDto,
    MetadataCandidate, MetadataSearchGroupDto, ProgramDto, SetupStatusDto, SystemInfoDto,
    TroubleshootingCheckDto, WineEngineDto, WineEngineSelectionDto,
};
use crate::bridge::{
    BottleCreateUpdate, BottleOperation, BottleOperationUpdate, BridgeEvent, BridgeStatus,
    BridgeSupervisor, DiagnosticKind, DiagnosticUpdate, EntryMutation, FolderEdit, MetadataUpdate,
    ProgramMutation, ScanUpdate, SetupUpdate,
};
use crate::config::{self, ConfigField};
use crate::create_bottle::CreateBottleState;
#[cfg(feature = "visual-test")]
use crate::library_layout::CardRect;
use crate::library_view::{self, Sort};
use crate::model::{
    BottleVm, Game, demo_bottles, demo_data, demo_requested, empty_library_message,
};
use crate::preferences;
use crate::processes::{self, KillOutcome, ProcessTable};
use crate::program_settings::{ProgramSettingsPage, settings_patch};
use crate::programs::ProgramTable;
use crate::runtime::{
    ClipboardDecision, ClipboardPrompt, ConfiguredBottles, PendingEdits, PrimaryAction,
    RuntimeAction, RuntimeState, RuntimeTable, primary_action_for,
};
use crate::scan::JobStatus;
use crate::views::Command;
use crate::winetricks::{InstallJob, WinetricksState};

/// Toast copy for the demo-only menu actions, which still do not mutate
/// anything. Runtime failures reuse the same toast with the helper's reason.
pub(crate) fn demo_notice() -> &'static str {
    crate::i18n::s().library.demo_notice
}

/// How long preference writes are coalesced before reaching the bridge.
const PREFERENCE_DEBOUNCE: Duration = Duration::from_millis(300);
/// Text and number rows wait this long after the last keystroke; toggles and
/// pickers commit immediately.
const CONFIG_DEBOUNCE: Duration = Duration::from_millis(400);
/// Immersive hero auto-advance cadence.
const CAROUSEL_SECS: u64 = 8;

/// Tabs inside the in-app About Maltoria modal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AboutTab {
    #[default]
    Overview,
    Acknowledgments,
    License,
    Notices,
}

/// Top-level sections in the macOS-style Settings sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SettingsSection {
    #[default]
    General,
    Library,
    Translation,
    Metadata,
    Runtime,
    Network,
}

impl SettingsSection {
    pub(crate) const ALL: [Self; 6] = [
        Self::General,
        Self::Library,
        Self::Translation,
        Self::Metadata,
        Self::Runtime,
        Self::Network,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Library => "library",
            Self::Translation => "translation",
            Self::Metadata => "metadata",
            Self::Runtime => "runtime",
            Self::Network => "network",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        let s = &crate::i18n::s().settings;
        match self {
            Self::General => s.section_general,
            Self::Library => s.section_library,
            Self::Translation => s.section_translation,
            Self::Metadata => s.section_metadata,
            Self::Runtime => s.section_runtime,
            Self::Network => s.section_network,
        }
    }

    pub(crate) fn icon(self) -> &'static str {
        match self {
            Self::General => "sliders-horizontal",
            Self::Library => "layout-grid",
            Self::Translation => "languages",
            Self::Metadata => "book-open",
            Self::Runtime => "cpu",
            Self::Network => "globe",
        }
    }

    pub(crate) fn scroll_id(self) -> &'static str {
        match self {
            Self::General => "settings-section-general",
            Self::Library => "settings-section-library",
            Self::Translation => "settings-section-translation",
            Self::Metadata => "settings-section-metadata",
            Self::Runtime => "settings-section-runtime",
            Self::Network => "settings-section-network",
        }
    }
}

/// Lifecycle state for a secondary native window owned by `WhiskyApp`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum DetachedWindow {
    #[default]
    Closed,
    Opening,
    Open(AnyWindowHandle),
}

impl DetachedWindow {
    pub(crate) fn is_active(&self) -> bool {
        !matches!(self, Self::Closed)
    }

    pub(crate) fn handle(&self) -> Option<AnyWindowHandle> {
        match self {
            Self::Open(handle) => Some(*handle),
            Self::Closed | Self::Opening => None,
        }
    }
}

/// `configuration.schema` + `configuration.read` for the selected bottle.
#[derive(Clone)]
pub(crate) struct ConfigPage {
    pub(crate) container: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) fields: Vec<ConfigField>,
    pub(crate) values: BTreeMap<String, serde_json::Value>,
    pub(crate) loading: bool,
    /// A page-level failure (schema or read); field rejections use the toast.
    pub(crate) error: Option<String>,
}

/// One scan, as the sheet renders it. `finished` stays true after completion so
/// the last result remains visible; starting another scan replaces it.
#[derive(Clone, Default)]
pub(crate) struct ScanState {
    pub(crate) container: String,
    pub(crate) job_id: Option<String>,
    pub(crate) status: Option<JobStatus>,
    pub(crate) stage: Option<String>,
    pub(crate) loaded: usize,
    pub(crate) total: Option<u64>,
    pub(crate) error: Option<String>,
    pub(crate) finished: bool,
}

/// What the in-app executable picker is for. `candidates` is filled in by the
/// `folders.candidates` reply, so the sheet lists them as they arrive.
#[derive(Clone)]
pub(crate) struct ExecutablePickerState {
    pub(crate) game_index: usize,
    pub(crate) container: String,
    pub(crate) entry_id: String,
    pub(crate) title: String,
    /// The folder entry's own path: a folder, never a file.
    pub(crate) folder: String,
    /// Play starts the game afterwards; the card menu only records the choice.
    pub(crate) launch_after: bool,
    pub(crate) multi_select: bool,
    pub(crate) selected: Vec<String>,
    pub(crate) return_to_run_settings: bool,
    pub(crate) terminal: bool,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) candidates: Vec<CandidateDto>,
}

/// Launch entry chooser shown on launch when a work has multiple `.exe` entry points.
#[derive(Clone)]
pub(crate) struct LaunchChooserState {
    pub(crate) game_index: usize,
    pub(crate) container: String,
    pub(crate) entry_id: String,
    pub(crate) title: String,
    pub(crate) folder: String,
    pub(crate) executables: Vec<String>,
    pub(crate) last_executable: Option<String>,
    pub(crate) terminal: bool,
}

impl ScanState {
    pub(crate) fn running(&self) -> bool {
        self.status.is_some_and(|status| !status.is_terminal())
    }
}

/// `programs.list` for one bottle. Pin/block replies carry the refreshed
/// list, so the table is always the helper's own view of the truth except
/// for edits still in flight.
pub(crate) struct ProgramsPage {
    pub(crate) container: String,
    pub(crate) table: ProgramTable,
    pub(crate) loading: bool,
    /// A page-level failure; a rejected pin/block uses the toast and rolls
    /// its own flag back.
    pub(crate) error: Option<String>,
}

/// `processes.list` for one bottle, polled while the Processes page is open.
pub(crate) struct ProcessesPage {
    pub(crate) container: String,
    pub(crate) table: ProcessTable,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreviewPage {
    Library,
    Statistics,
    /// Storefront clients (Steam etc.), kept apart from the game library.
    Launchers,
    Bottle,
    ConfigureBottle,
    Programs,
    Processes,
    GameConfigs,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewSheet {
    RunSettings(usize),
    Scan,
    /// The folder's `.exe` files, for picking what a folder entry starts.
    Executable,
    Winetricks,
    CreateBottle,
    /// Choose another bottle to re-home one library entry.
    MoveEntry(usize),
    /// Choose a bottle for a file opened from Finder or dropped on the window.
    OpenFile,
    Diagnostics,
    Audio,
    Dll,
    Graphics,
    /// Online metadata, custom cover and GameDB recommendations for a card.
    GameInfo(usize),
    /// Custom tag assignment sheet for one library entry.
    GameTags(usize),
    /// Global tag manager across loaded bottles.
    ManageTags,
}

/// One `entry.move` request while its target picker is open.
pub(crate) struct MoveEntryState {
    pub(crate) title: String,
    pub(crate) source_container: String,
    pub(crate) entry_id: String,
    pub(crate) pending: bool,
    pub(crate) target_container: Option<String>,
    pub(crate) error: Option<String>,
}
/// A file opened from Finder or dropped on the window, awaiting a bottle.
pub(crate) struct OpenFileState {
    pub(crate) path: String,
    pub(crate) name: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BottleDialogKind {
    Rename,
    Duplicate,
    Remove,
}

/// The library sidebar's three presentations: labelled rows, an icon-only
/// rail, or removed entirely.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidebarMode {
    Hidden,
    Icons,
    Expanded,
}

impl SidebarMode {
    /// The strip's resting width; the window's spring animates to it.
    pub(crate) fn width(self) -> f32 {
        match self {
            Self::Hidden => 0.,
            Self::Icons => 56.,
            Self::Expanded => 200.,
        }
    }
    /// The fixed width the content is laid out at under the animated clip;
    /// `Hidden` keeps the expanded rows so the slide-out reads identically.
    pub(crate) fn layout_width(self) -> f32 {
        match self {
            Self::Icons => Self::Icons.width(),
            _ => Self::Expanded.width(),
        }
    }
    /// The `ToggleSidebar` action: collapse to the remembered collapsed style
    /// (`Icons` or `Hidden`), or expand again when already collapsed.
    pub(crate) fn toggled(self, collapsed_style: Self) -> Self {
        match self {
            Self::Expanded => collapsed_style,
            _ => Self::Expanded,
        }
    }
    /// The `librarySidebarMode` preference value.
    pub(crate) fn raw(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Icons => "icons",
            Self::Expanded => "expanded",
        }
    }
    /// A stored `librarySidebarMode`; unknown values keep the caller's default.
    pub(crate) fn from_raw(raw: &str) -> Option<Self> {
        Some(match raw {
            "hidden" => Self::Hidden,
            "icons" => Self::Icons,
            "expanded" => Self::Expanded,
            _ => return None,
        })
    }
}

/// What the Wine-engine dialog is asking the user to confirm.
#[derive(Clone)]
pub(crate) enum EngineDialogKind {
    /// Enabling an `unverified` engine feature, then applying the value the
    /// user asked for. `fingerprint` is bound at open time; the helper
    /// rejects it if the engine changed meanwhile.
    Feature {
        feature: String,
        apply: PendingEngineApply,
        reason: String,
        fingerprint: String,
    },
    /// Rebinding an existing prefix to another engine. The prefix is not
    /// converted automatically — the warning says so before anything runs.
    Select {
        engine_name: String,
    },
}

/// Where a confirmed feature action lands once its approval succeeds.
#[derive(Clone)]
pub(crate) enum PendingEngineApply {
    /// `bottle.setConfiguration` for a bottle configuration-page field.
    Config {
        field: String,
        value: serde_json::Value,
    },
    /// The open program-settings draft — never the bottle page. The
    /// executable is bound at open time so an approval for one game never
    /// mutates a draft the user has since opened for another.
    ProgramDraft {
        field: String,
        value: serde_json::Value,
        executable: String,
    },
}

/// Outcome of re-checking a confirmed apply against the fresh selection.
enum PendingVerdict {
    /// Policy lets the requested value through — perform the write.
    Ready,
    /// Another unverified prerequisite gates it — confirm that next.
    NextApproval(String),
    /// A hard-unavailable prerequisite blocks it — drop the action.
    Blocked,
}

/// A write queued behind a feature confirmation. The approval RPC goes out
/// alone; the write the user asked for is applied only after the matching
/// `EngineSelection` reply carries the approval — a stale, failed or
/// cancelled approval performs no write at all.
pub(crate) struct PendingEngineAction {
    pub(crate) container: String,
    pub(crate) engine_id: String,
    pub(crate) feature: String,
    pub(crate) fingerprint: String,
    pub(crate) apply: PendingEngineApply,
}

/// Pending Wine-engine confirmation: approve runs the queued action, cancel
/// drops it without touching the bottle.
pub(crate) struct EngineDialogState {
    pub(crate) kind: EngineDialogKind,
    pub(crate) container: String,
    /// Engine the action targets — the bound engine for a feature
    /// confirmation, the candidate for a rebind.
    pub(crate) engine_id: String,
}

/// The sidebar's modal actions. Rename and Duplicate read a text field;
/// Remove asks whether the directory should be deleted as well.
pub(crate) struct BottleDialogState {
    pub(crate) kind: BottleDialogKind,
    pub(crate) container: String,
    pub(crate) name: String,
    pub(crate) suggested: String,
    pub(crate) input: Option<Entity<InputState>>,
}

/// What a `system.choosePath` reply should do when it comes back.
#[derive(Clone)]
enum PathAction {
    Run { container: String },
    Move { container: String },
    Export { container: String },
    Pin { container: String },
    DiagnosticsExport { container: String },
    MetadataCover { container: String, entry_id: String },
}

#[derive(Clone)]
pub(crate) struct MetadataGroup {
    pub(crate) source: String,
    pub(crate) error: Option<String>,
    pub(crate) games: Vec<MetadataCandidate>,
}

/// Scraper sources: toggle key, wire name, label.
pub(crate) const METADATA_SOURCES: [(&str, &str); 5] = [
    ("src-steam", "steam"),
    ("src-bangumi", "bangumi"),
    ("src-vndb", "vndb"),
    ("src-ymgal", "ymgal"),
    ("src-nextmoe", "nextmoe"),
];

/// The Game Info sheet: metadata search, cover choice and GameDB matches for
/// one library card.
pub(crate) struct GameInfoPage {
    pub(crate) editor: crate::metadata_editor::MetadataEditor,
    pub(crate) container: String,
    pub(crate) entry_id: String,
    pub(crate) title: String,
    pub(crate) input: Option<Entity<InputState>>,
    pub(crate) searching: bool,
    pub(crate) searching_sources: Vec<String>,
    pub(crate) sources_expanded: bool,
    pub(crate) applying: bool,
    pub(crate) error: Option<String>,
    pub(crate) groups: Vec<MetadataGroup>,
    pub(crate) matches: Option<Vec<GameDbMatchDto>>,
    pub(crate) config_message: Option<String>,
    pub(crate) config_error: Option<String>,
    pub(crate) selected_character: Option<String>,
}

impl GameInfoPage {
    fn accepts_search(&self, container: &str, entry_id: &str, generation: u64, request: u64) -> bool {
        self.searching
            && self.container == container
            && self.entry_id == entry_id
            && self.editor.generation == generation
            && self.editor.search_request == request
    }
}

#[cfg(test)]
mod setup_prompt_tests {
    use super::WhiskyApp;
    use crate::bridge::dto::SetupStatusDto;

    fn status(installed: bool, managed: Option<bool>, any_engine: Option<bool>) -> Option<SetupStatusDto> {
        Some(SetupStatusDto {
            installed,
            managed_installed: managed,
            any_engine_available: any_engine,
            version: None,
            path: String::new(),
            rosetta_installed: true,
            apple_silicon: true,
        })
    }

    #[test]
    fn external_engine_availability_suppresses_managed_prompt() {
        // installed:false / anyEngine:true — an external engine can run
        // bottles, so no managed-install prompt.
        assert!(!WhiskyApp::managed_install_prompt_needed(&status(
            false,
            Some(false),
            Some(true),
        )));
        // Nothing at all → prompt.
        assert!(WhiskyApp::managed_install_prompt_needed(&status(
            false,
            Some(false),
            Some(false),
        )));
        // Managed installed → no prompt.
        assert!(!WhiskyApp::managed_install_prompt_needed(&status(
            true,
            Some(true),
            Some(true),
        )));
        // Legacy helper semantics: only `installed`, no new keys — installed
        // is both the managed and the any-engine answer.
        assert!(!WhiskyApp::managed_install_prompt_needed(&status(true, None, None)));
        assert!(WhiskyApp::managed_install_prompt_needed(&status(false, None, None)));
        // No status at all → prompt.
        assert!(WhiskyApp::managed_install_prompt_needed(&None));
    }
}

#[cfg(test)]
mod metadata_search_tests {
    use super::GameInfoPage;
    use crate::metadata_editor::MetadataEditor;

    fn searching_page() -> GameInfoPage {
        GameInfoPage {
            editor: MetadataEditor {
                search_request: 1,
                ..MetadataEditor::default()
            },
            container: "bottle".into(),
            entry_id: "entry".into(),
            title: "Game".into(),
            input: None,
            searching: true,
            searching_sources: Vec::new(),
            sources_expanded: false,
            applying: false,
            error: None,
            groups: Vec::new(),
            matches: None,
            config_message: None,
            config_error: None,
            selected_character: None,
        }
    }

    #[test]
    fn search_events_require_current_editor_request_and_entry() {
        let mut page = searching_page();
        let generation = page.editor.generation;
        assert!(page.accepts_search("bottle", "entry", generation, 1));
        assert!(!page.accepts_search("other", "entry", generation, 1));
        assert!(!page.accepts_search("bottle", "other", generation, 1));
        page.editor.search_request += 1;
        assert!(!page.accepts_search("bottle", "entry", generation, 1));
        assert!(page.accepts_search("bottle", "entry", generation, 2));
        page.searching = false;
        assert!(!page.accepts_search("bottle", "entry", generation, 2));
    }

    #[test]
    fn reopened_editor_rejects_old_search_even_for_same_entry_and_request() {
        let old = searching_page();
        let reopened = searching_page();
        assert_ne!(old.editor.generation, reopened.editor.generation);
        // Started, results and failures all use this same acceptance rule.
        assert!(!reopened.accepts_search("bottle", "entry", old.editor.generation, 1));
        let closed: Option<GameInfoPage> = None;
        assert!(!closed.as_ref().is_some_and(|page| {
            page.accepts_search("bottle", "entry", old.editor.generation, 1)
        }));
    }
}

pub(crate) struct PreviewState {
    pub(crate) page: PreviewPage,
    pub(crate) statistics_day: Option<chrono::NaiveDate>,
    pub(crate) history: Vec<PreviewPage>,
    pub(crate) sheet: Option<PreviewSheet>,
    pub(crate) sheet_history: Vec<PreviewSheet>,
    pub(crate) toggles: BTreeMap<&'static str, bool>,
    pub(crate) choices: BTreeMap<&'static str, SharedString>,
    pub(crate) collapsed: BTreeSet<&'static str>,
    pub(crate) inputs: BTreeMap<&'static str, Entity<InputState>>,
    /// Schema text/number rows, created on first render because `InputState`
    /// needs a window. Cleared when the bottle changes.
    pub(crate) config_inputs: BTreeMap<String, Entity<InputState>>,
    /// Per-root executable fields in the scan sheet, keyed `scan-exe:<root>`.
    pub(crate) folder_inputs: BTreeMap<String, Entity<InputState>>,
    /// Run-settings fields, recreated whenever a read replaces the draft.
    pub(crate) program_inputs: BTreeMap<String, Entity<InputState>>,
    /// Fields whose input must be rebuilt on the next render, after a rejected
    /// or normalized write. Replacing the entity avoids `set_value`, which
    /// would notify an observer while the root view is rendering.
    pub(crate) config_resets: BTreeSet<String>,
    pub(crate) sort: usize,
    // Retained per document, including forms mounted inside settings sheets.
    pub(crate) scroll_handles: std::cell::RefCell<BTreeMap<&'static str, ScrollHandle>>,
}
impl PreviewState {
    pub(crate) fn form_scroll_handle(&self, id: &'static str) -> ScrollHandle {
        self.scroll_handles
            .borrow_mut()
            .entry(id)
            .or_insert_with(ScrollHandle::new)
            .clone()
    }
    pub(crate) fn new(window: &mut Window, cx: &mut Context<WhiskyApp>) -> Self {
        let mut inputs = BTreeMap::new();
        for (key, placeholder) in WhiskyApp::preview_input_specs() {
            inputs.insert(
                key,
                cx.new(|cx| InputState::new(window, cx).placeholder(placeholder)),
            );
        }
        Self {
            page: PreviewPage::Library,
            statistics_day: None,
            history: vec![],
            sheet: None,
            sheet_history: vec![],
            inputs,
            config_inputs: BTreeMap::new(),
            folder_inputs: BTreeMap::new(),
            program_inputs: BTreeMap::new(),
            config_resets: BTreeSet::new(),
            toggles: [
                ("refresh", preferences::DEFAULT_REFRESH_AFTER_EXIT),
                ("single-title", preferences::DEFAULT_SINGLE_LINE_TITLES),
                ("center-cover-text", preferences::DEFAULT_CENTER_WIDE_COVER_TEXT),
                ("blurred-fill", preferences::DEFAULT_BLURRED_COVER_FILL),
                ("backdrop-fill", preferences::DEFAULT_BACKDROP_FILL),
                ("scrape-characters", preferences::DEFAULT_SCRAPE_CHARACTERS),
                ("shader", true),
                ("appnap", true),
                ("kill", preferences::DEFAULT_KILL_ON_TERMINATE),
                ("menubar", preferences::DEFAULT_SHOW_MENU_BAR_EXTRA),
                ("src-steam", true),
                ("src-bangumi", true),
                ("src-vndb", true),
                ("src-ymgal", true),
                ("src-nextmoe", true),
                ("offline", preferences::DEFAULT_OFFLINE_MODE),
                ("updates", preferences::DEFAULT_CHECK_UPDATES),
                ("app-updates", preferences::DEFAULT_CHECK_APP_UPDATES),
                ("blur", preferences::DEFAULT_DETAIL_BLURRED_BACKGROUND),
            ]
            .into(),
            choices: [
                ("appearance", crate::ui_state::load_appearance().wire().into()),
                ("language", crate::ui_state::load_language().wire().into()),
                ("create-windows", "Windows 11".into()),
                ("terminal", preferences::DEFAULT_PREFERRED_TERMINAL.into()),
                ("cover-fill", preferences::DEFAULT_COVER_FILL_STYLE.into()),
                ("detail-layout", preferences::DEFAULT_DETAIL_LAYOUT_STYLE.into()),
            ]
            .into(),
            collapsed: [
                "launcher",
                "input",
                "upscaling",
                "dll",
                "cleanup",
                "overrides",
                "console",
            ]
            .into(),
            sort: 0,
            scroll_handles: Default::default(),
        }
    }
}

/// The query/scope a cached index list was computed for. `revision` changes
/// whenever an entry is added, removed, or mutated.
#[derive(Clone, PartialEq, Eq)]
struct ViewKey {
    query: String,
    tag_filter: library_view::TagFilter,
    studio_filter: library_view::StudioFilter,
    slot: usize,
    show_hidden: bool,
    revision: u64,
}

#[derive(Clone, PartialEq, Eq)]
struct OrderKey {
    view: ViewKey,
    sort: usize,
    reversed: bool,
}

#[derive(Default)]
struct ViewCache {
    view: Option<(ViewKey, Vec<usize>)>,
    ordered: Option<(OrderKey, Vec<usize>)>,
}

pub(crate) struct WhiskyApp {
    pub(crate) preview: PreviewState,
    pub(crate) search: Entity<InputState>,
    pub(crate) mode: usize,
    pub(crate) gallery_selection: Option<(String, String)>,
    /// The focused card of the staggered timeline, keyed by `(container, id)`
    /// so it survives re-sorts; cleared when the entry leaves the view.
    pub(crate) staggered_selection: Option<(String, String)>,
    pub(crate) gallery_detail_scroll: ScrollHandle,
    /// The bundled demo snapshot, enabled with `WHISKY_GPUI_DEMO_DATA=1`.
    pub(crate) demo: bool,
    pub(crate) bottles: Vec<BottleVm>,
    /// Every loaded entry, in bottle order, de-duplicated for display.
    /// `Game::bottle` is the one-based slot and `Game::container` is the path.
    pub(crate) entries: Vec<Game>,
    /// Successful reads keyed by bottle path: revisiting a bottle is instant.
    pub(crate) bottle_entries: BTreeMap<String, Vec<Game>>,
    /// Index into `bottles`; `None` means the whole library.
    pub(crate) selected_bottle: Option<usize>,
    /// A read for the visible scope is still in flight.
    pub(crate) loading: bool,
    /// Session/catalogue failure for the whole library surface.
    pub(crate) load_error: Option<String>,
    /// Bottle indices with a queued or in-flight read.
    pending: BTreeSet<usize>,
    /// Per-bottle read errors, shown when that bottle is visible.
    pub(crate) failures: BTreeMap<usize, String>,
    /// Bottles already handed to `runtime.configure`, so Play on the Library
    /// page configures each bottle exactly once.
    configured: ConfiguredBottles,
    /// `libraryShowHiddenGames`; hidden entries stay in `entries` (so indices
    /// are stable) and only leave the grid through this filter.
    show_hidden: bool,
    view_cache: RefCell<ViewCache>,
    library_revision: u64,
    /// Keys `preferences.schema` advertises. Anything else stays in memory.
    prefs_supported: BTreeSet<String>,
    update_preferences_loaded: bool,
    prefs_pending: BTreeMap<String, serde_json::Value>,
    prefs_generation: u64,
    pub(crate) metadata_preferences: crate::onboarding::MetadataPreferences,
    pub(crate) onboarding: Option<crate::onboarding::Onboarding>,
    onboarding_checked: bool,
    onboarding_completed: bool,
    /// Bottle configuration for the selected bottle; `None` until the page is
    /// opened or after a bottle switch.
    pub(crate) config: Option<ConfigPage>,
    /// Last committed value per field, for rolling back a rejected write.
    config_writes: PendingEdits<serde_json::Value>,
    /// Text/number drafts not yet sent, coalesced across fields.
    config_text_pending: BTreeMap<String, String>,
    config_text_generation: u64,
    /// Folder state keyed by container path.
    pub(crate) folders: BTreeMap<String, FoldersDto>,
    pub(crate) folders_pending: BTreeSet<String>,
    pub(crate) folders_error: BTreeMap<String, String>,
    /// Containers whose folder settings changed and whose entries need a
    /// read after the next `Folders` reply.
    folders_reload_after_write: BTreeSet<String>,
    /// Containers that already had their automatic metadata pass this
    /// session. A scan may reset it: new folders can be pending again.
    auto_synced: BTreeSet<String>,
    /// Container waiting for the native multi-folder picker reply.
    scan_root_picker: Option<String>,
    /// `launchers.read` results: entry identities (`metadataIdentity`) that
    /// auto-start with Whisky, keyed by container path.
    pub(crate) launcher_prefs: BTreeMap<String, BTreeSet<String>>,
    /// Containers whose `launchers.read` is in flight or already answered.
    launchers_pending: BTreeSet<String>,
    /// Launchers already auto-started this session; the flag fires once per
    /// app run, not once per entries reload.
    launcher_autostarted: BTreeSet<String>,
    /// The card the in-app executable picker is open for, and whether choosing
    /// one should start it (Play) or only record it (the card menu).
    pub(crate) executable_picker: Option<ExecutablePickerState>,
    /// Container environment map keyed by container path.
    pub(crate) environments: BTreeMap<String, BTreeMap<String, String>>,
    pub(crate) environment_error: BTreeMap<String, String>,
    /// Installed programs for the selected bottle.
    pub(crate) programs: Option<ProgramsPage>,
    /// Running processes for the selected bottle, polled while its page is
    /// open.
    pub(crate) processes: Option<ProcessesPage>,
    /// Bumped to stop the process poll loop: a loop exits once the generation
    /// it captured no longer matches.
    process_generation: u64,
    /// The one scan job this session follows.
    pub(crate) scan: Option<ScanState>,
    /// The one create-bottle job this session follows.
    pub(crate) create_bottle: Option<CreateBottleState>,
    pub(crate) import_bottles: Option<crate::container_import::ImportState>,
    pub(crate) import_generation: u64,
    pub(crate) host_terminals: Option<crate::host::TerminalsDto>,
    pub(crate) host_terminal_error: Option<String>,
    pub(crate) desktop_status: Option<crate::host::DesktopDto>,
    pub(crate) desktop_error: Option<String>,
    pub(crate) desktop_installing: bool,
    /// Winetricks catalog, per-bottle installed sets and the one install job.
    pub(crate) winetricks: WinetricksState,
    /// Draft of the program whose run-settings sheet is open.
    pub(crate) program_settings: ProgramSettingsPage,
    pub(crate) translation_page: crate::translation::TranslationPage,
    pub(crate) capture: crate::capture::CaptureState,
    /// The pending library-entry move, while its target picker is open.
    pub(crate) move_entry: Option<MoveEntryState>,
    /// The opened/dropped file, while its bottle picker is open.
    pub(crate) open_file: Option<OpenFileState>,
    pub(crate) catalogue_ready: bool,
    desktop_resolution_generation: u64,
    desktop_resolutions: HashMap<u64, crate::desktop_open::ExternalOpen>,
    external_opens: std::collections::VecDeque<crate::desktop_open::ExternalOpen>,
    /// Metadata, cover and GameDB state for the open Game Info sheet.
    pub(crate) game_info: Option<GameInfoPage>,
    /// The sidebar's rename/duplicate/remove dialog.
    pub(crate) bottle_dialog: Option<BottleDialogState>,
    /// `wine.engines.list` — the managed row plus registered externals.
    /// Empty when the helper predates the API (`service_available` gates UI).
    pub(crate) wine_engines: Vec<WineEngineDto>,
    /// Registry corruption or a failed management call, shown in Settings.
    pub(crate) wine_engines_error: Option<String>,
    /// `wine.engines.selection` per container path, refreshed on config load
    /// and after every selection/override mutation.
    pub(crate) engine_selections: HashMap<String, WineEngineSelectionDto>,
    /// The pending unverified-feature / engine-rebind confirmation.
    pub(crate) engine_dialog: Option<EngineDialogState>,
    /// The write queued behind an in-flight `setFeatureOverride` approval,
    /// applied only when the matching reply confirms it landed.
    pub(crate) pending_engine_action: Option<PendingEngineAction>,
    /// When set, the next `FolderChosen` registers a Wine engine instead of
    /// updating the default bottle location.
    engine_folder_pending: bool,
    /// The lifecycle operation currently in flight, if any.
    pub(crate) bottle_operation: Option<(BottleOperation, String)>,
    /// A file/folder/save picker whose reply drives a bottle action.
    pending_path_action: Option<PathAction>,
    /// A bottle path to select once the refreshed catalogue arrives (move,
    /// duplicate).
    select_after_refresh: Option<String>,
    /// `bottle.diagnostics` report text and its request state.
    pub(crate) diagnostics_report: Option<String>,
    pub(crate) diagnostics_pending: bool,
    pub(crate) diagnostics_error: Option<String>,
    pub(crate) diagnostic_checks: Option<Vec<TroubleshootingCheckDto>>,
    pub(crate) checks_pending: bool,
    /// `audio.test` result and its request state.
    pub(crate) audio_result: Option<AudioResultDto>,
    pub(crate) audio_pending: bool,
    pub(crate) audio_error: Option<String>,
    /// WhiskyWine runtime status and the first-run setup dialog.
    pub(crate) setup_status: Option<SetupStatusDto>,
    pub(crate) gptk_status: Option<crate::bridge::dto::GptkStatusDto>,
    pub(crate) gptk_importing: bool,
    pub(crate) gptk_error: Option<String>,
    /// The one-shot WhiskyWine update check for this session.
    wine_update_checked: bool,
    pub(crate) installed_app: Option<crate::app_update::InstalledApp>,
    pub(crate) app_update: crate::app_update::State,
    pub(crate) setup_dialog: bool,
    pub(crate) setup_installing: bool,
    pub(crate) setup_stage: Option<String>,
    /// Downloaded and total bytes while the runtime archive downloads.
    pub(crate) setup_progress: Option<(u64, u64)>,
    pub(crate) setup_error: Option<String>,
    setup_prompted: bool,
    /// `defaultBottleLocation` from preferences, used as `bottle.create`'s parent.
    pub(crate) default_bottle_location: Option<String>,
    /// Key that "编辑" put in the env inputs; saving with a different name
    /// removes it first, because the bridge has no rename call.
    env_original: Option<String>,
    env_clear_on_reply: bool,
    /// Text "编辑" staged for the two env inputs; applied on the next render.
    pub(crate) env_fill: Option<(String, String)>,
    /// Last committed value per entry, for rolling back optimistic edits.
    rating_edits: PendingEdits<Option<u8>>,
    hidden_edits: PendingEdits<bool>,
    tag_edits: PendingEdits<Vec<String>>,
    launch_options_edits: PendingEdits<(bool, Vec<String>, Option<String>)>,
    pub(crate) tag_filter: library_view::TagFilter,
    pub(crate) studio_filter: library_view::StudioFilter,
    pub(crate) group_by_studio: bool,
    pub(crate) collapsed_studios: BTreeSet<String>,
    pub(crate) filter_bar_tab: crate::ui_state::FilterBarTab,
    pub(crate) tag_bar_mode: crate::ui_state::TagBarMode,
    pub(crate) tag_bar_hint_seen: bool,
    pub(crate) tag_bar_hint_peeking: bool,
    tag_bar_hint_generation: u64,
    tag_bar_hint_scheduled: bool,
    pub(crate) pull_reveal: library_view::PullRevealState,
    pull_idle_generation: u64,
    pub(crate) launch_chooser: Option<LaunchChooserState>,
    pub(crate) launch_chooser_generation: u64,
    pub(crate) launch_chooser_return_focus: Option<gpui_kit::FocusHandle>,
    pub(crate) executable_picker_generation: u64,
    pub(crate) preview_sheet_generation: u64,
    pub(crate) preview_sheet_return_focus: Option<gpui_kit::FocusHandle>,
    pub(crate) preview_sheet_return_captured_gen: u64,
    pub(crate) tag_input: Entity<InputState>,
    tag_input_subscription: Subscription,
    pub(crate) tag_input_fill: Option<String>,
    pub(crate) tag_input_refocus: bool,
    pub(crate) tag_manager_rename_target: Option<String>,
    pub(crate) tag_manager_input: Entity<InputState>,
    tag_manager_input_subscription: Subscription,
    pub(crate) tag_manager_fill: Option<String>,
    pub(crate) run_settings_target_exe: Option<String>,
    /// The library sidebar's presentation; `Hidden` leaves the toolbar's
    /// reveal button as the way back.
    pub(crate) sidebar: SidebarMode,
    /// Set once `set_sidebar` runs: a stored `librarySidebarMode` arriving
    /// later (a reconnect's `preferences.read` or a `setMany` echo) must not
    /// override the user's in-session choice.
    sidebar_dirty: bool,
    /// The collapsed presentation (`Icons` or `Hidden`) last chosen; what the
    /// collapse button and `ToggleSidebar` switch to. Persisted.
    pub(crate) sidebar_collapsed_style: SidebarMode,
    pub(crate) settings: bool,
    pub(crate) settings_section: SettingsSection,
    pub(crate) settings_window: DetachedWindow,
    pub(crate) capture_window: DetachedWindow,
    /// The About panel; independent of the settings page and sheets.
    pub(crate) about: bool,
    pub(crate) about_tab: AboutTab,
    /// Packaged version metadata from `system.info`, when the helper answered.
    pub(crate) app_info: Option<SystemInfoDto>,
    /// An explicit quit is waiting for the helper to apply the exit policy.
    quitting: bool,
    quit_generation: u64,
    pub(crate) quit_error: Option<String>,
    pub(crate) motion: bool,
    pub(crate) lively: bool,
    pub(crate) reversed: bool,
    pub(crate) library_scroll: [ScrollHandle; 6],
    pub(crate) detail_scroll: ScrollHandle,
    pub(crate) scroll_needs_measurement: std::cell::Cell<bool>,
    #[cfg(feature = "visual-test")]
    pub(crate) motion_samples: std::cell::RefCell<std::collections::BTreeMap<usize, CardRect>>,
    pub(crate) mixed: bool,
    /// Chosen cover width in points; zero divides the row between the covers.
    pub(crate) cover_width: f32,
    /// The width the shelf drew last, so the control can show an automatic
    /// size instead of parking its thumb on a number nobody chose.
    pub(crate) cover_unit: std::cell::Cell<f32>,
    /// The row width the shelf last laid out into, so the control knows which
    /// flush size levels exist. Zero until the shelf first renders.
    pub(crate) cover_span: std::cell::Cell<f32>,
    pub(crate) artwork_cache: Entity<crate::artwork::ArtworkCache>,
    /// How hard the cover size option works; `Off` draws the file as it arrived.
    pub(crate) cover_upscale: crate::upscale::Factor,
    /// Which network enlarges the covers.
    pub(crate) cover_engine: crate::upscale::Engine,
    /// Backdrop blur level for the ambient fill and the detail panel tint;
    /// zero draws the artwork as it arrived.
    pub(crate) backdrop_blur: u8,
    /// Derivatives already prepared, keyed by cover path, passes and engine.
    cover_derivatives: RefCell<
        HashMap<(std::path::PathBuf, u8, crate::upscale::Engine), Option<std::path::PathBuf>>,
    >,
    /// Jobs in flight, so a redraw does not queue the same work twice.
    cover_jobs: RefCell<std::collections::HashSet<(std::path::PathBuf, u8, crate::upscale::Engine)>>,
    /// Whole-library cover derivation while one is running.
    pub(crate) cover_batch: Option<CoverBatch>,
    cover_batch_generation: u64,
    cover_batch_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    cover_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) hovered: Option<usize>,
    pub(crate) ambient_index: usize,
    /// Current hero slide of the immersive mode; wraps at render time against
    /// the featured set.
    pub(crate) immersive_slide: usize,
    /// Hero pointer hover: pauses the auto-advance and reveals the edge
    /// chevrons.
    pub(crate) immersive_hero_hovered: bool,
    /// Bumped by manual slide navigation; a running loop exits once its
    /// captured epoch no longer matches, which restarts the 8 s countdown.
    immersive_carousel_epoch: std::cell::Cell<u64>,
    /// True while one auto-advance loop is alive; guards the lazy start.
    immersive_carousel_running: std::cell::Cell<bool>,
    /// Scroll handle per horizontal row, keyed by the row's stable name.
    pub(crate) immersive_row_scrolls: std::cell::RefCell<HashMap<String, ScrollHandle>>,
    pub(crate) detail: Option<usize>,
    pub(crate) detail_open: bool,
    pub(crate) detail_open_generation: u64,
    pub(crate) detail_return_focus: Option<gpui_kit::FocusHandle>,
    pub(crate) detail_character: Option<String>,
    pub(crate) detail_tags_expanded: bool,
    pub(crate) appearance: crate::theme::AppearanceMode,
    pub(crate) language: crate::i18n::LanguageMode,
    pub(crate) rendered_locale: crate::i18n::Locale,
    /// The message on the existing toast; `None` hides it. Runtime and entry
    /// failures reuse the toast instead of introducing another surface.
    pub(crate) notice: Option<SharedString>,
    /// Per-entry launch state, keyed by bridge entry id.
    pub(crate) runtime: RuntimeTable,
    /// Active graphics backend keyed by bridge entry id.
    pub(crate) active_backends: HashMap<String, String>,
    /// Active executable path keyed by bridge entry id, tracked on launch so
    /// post-exit legacy fullscreen probes inspect the exe that actually ran.
    pub(crate) active_executables: HashMap<String, String>,
    /// The clipboard preflight awaiting a decision, if any.
    pub(crate) clipboard_prompt: Option<ClipboardPrompt>,
    /// The post-run legacy fullscreen suggestion awaiting a decision, if any.
    pub(crate) legacy_fullscreen_prompt: Option<LegacyFullscreenPrompt>,
    pub(crate) bridge: BridgeSupervisor,
    pub(crate) bridge_status: BridgeStatus,
    pub(crate) service_methods: BTreeSet<String>,
    pub(crate) bridge_dialog: bool,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) _subscriptions: Vec<Subscription>,
}

/// Progress of a whole-library cover derivation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CoverBatch {
    pub(crate) done: usize,
    pub(crate) total: usize,
    generation: u64,
}

/// The legacy fullscreen suggestion surfaced after a run logs a failed
/// display-mode change. `enable` is the dialog switch draft — it only becomes
/// the program override when the user applies it.
#[derive(Clone, Debug)]
pub(crate) struct LegacyFullscreenPrompt {
    pub(crate) container: String,
    pub(crate) executable: String,
    pub(crate) program_name: String,
    /// The library entry the finished run belongs to, for the restart button.
    pub(crate) entry_id: String,
    pub(crate) enable: bool,
}

impl WhiskyApp {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (bridge, bridge_events) = BridgeSupervisor::start();
        let demo = demo_requested(std::env::var("WHISKY_GPUI_DEMO_DATA").ok().as_deref());
        Self::new_with_bridge(window, cx, bridge, bridge_events, demo)
    }

    /// The shared constructor: `new` supplies a real supervisor and reads the
    /// demo flag from the environment; headless tests hand over
    /// `BridgeSupervisor::disconnected_for_tests` and an empty event channel
    /// so no helper process is spawned.
    pub(crate) fn new_with_bridge(
        window: &mut Window,
        cx: &mut Context<Self>,
        bridge: BridgeSupervisor,
        bridge_events: smol::channel::Receiver<BridgeEvent>,
        demo: bool,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let language = crate::ui_state::load_language();
        #[cfg(not(test))]
        crate::i18n::init(language);
        let rendered_locale = crate::i18n::locale();
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::i18n::s().library.search_placeholder)
        });
        let subscription = cx.observe(&search, |_, _, cx| cx.notify());
        let mut preview = PreviewState::new(window, cx);
        let library_ui = crate::ui_state::load_library_ui_prefs();
        preview
            .choices
            .insert("tag-bar-mode", library_ui.tag_bar_mode.wire().into());
        let tag_bar_hint_seen = library_ui.tag_bar_hint_seen
            || demo
            || crate::ui_state::capture_mode_active();
        let mut pull_reveal = library_view::PullRevealState::default();
        if library_ui.tag_bar_mode == crate::ui_state::TagBarMode::AlwaysVisible {
            pull_reveal.set_open(true);
        }
        let dependency_search = preview.inputs["dependency-search"].clone();
        let dependency_subscription = cx.observe(&dependency_search, |_, _, cx| cx.notify());
        let (tag_input, tag_input_subscription) = Self::new_tag_input(String::new(), window, cx);
        let (tag_manager_input, tag_manager_input_subscription) =
            Self::new_tag_manager_input(String::new(), window, cx);
        // Polling pauses in the background, so the worker starts from the real
        // activation state rather than assuming the window is focused.
        bridge.set_active(window.is_window_active());
        let activation = cx.observe_window_activation(window, |this, window, _| {
            this.bridge.set_active(window.is_window_active());
        });
        let appearance = crate::ui_state::load_appearance();
        cx.set_window_appearance(appearance.window_override());
        crate::theme::sync_theme(appearance, Some(window), cx);
        let appearance_subscription = cx.observe_window_appearance(window, |this, window, cx| {
            crate::theme::sync_theme(this.appearance, Some(window), cx);
            cx.notify();
        });
        // Closing the last window quits, but the helper applies the exit policy
        // first. The frame is saved in request_quit for Cmd-Q and close alike.
        window.on_window_should_close(cx, {
            let view = cx.entity().downgrade();
            move |window, cx| {
                let _ = view.update(cx, |this, cx| this.request_quit(window, cx));
                false
            }
        });
        cx.spawn(async move |this, cx| {
            while let Ok(event) = bridge_events.recv().await {
                let _ = this.update(cx, |this, cx| this.apply_bridge_event(event, cx));
            }
        })
        .detach();
        Self {
            focus_handle,
            preview,
            search,
            mode: 0,
            gallery_selection: None,
            staggered_selection: None,
            gallery_detail_scroll: ScrollHandle::new(),
            demo,
            bottles: if demo { demo_bottles() } else { Vec::new() },
            entries: Vec::new(),
            bottle_entries: BTreeMap::new(),
            selected_bottle: None,
            loading: false,
            load_error: None,
            pending: BTreeSet::new(),
            failures: BTreeMap::new(),
            configured: ConfiguredBottles::default(),
            show_hidden: preferences::DEFAULT_SHOW_HIDDEN,
            view_cache: RefCell::new(ViewCache::default()),
            library_revision: 0,
            prefs_supported: BTreeSet::new(),
            update_preferences_loaded: false,
            prefs_pending: BTreeMap::new(),
            prefs_generation: 0,
            metadata_preferences: crate::onboarding::MetadataPreferences::default(),
            onboarding: None,
            onboarding_checked: false,
            onboarding_completed: false,
            config: None,
            config_writes: PendingEdits::default(),
            config_text_pending: BTreeMap::new(),
            config_text_generation: 0,
            folders: BTreeMap::new(),
            folders_pending: BTreeSet::new(),
            folders_error: BTreeMap::new(),
            folders_reload_after_write: BTreeSet::new(),
            auto_synced: BTreeSet::new(),
            scan_root_picker: None,
            launcher_prefs: BTreeMap::new(),
            launchers_pending: BTreeSet::new(),
            launcher_autostarted: BTreeSet::new(),
            executable_picker: None,
            environments: BTreeMap::new(),
            environment_error: BTreeMap::new(),
            programs: None,
            processes: None,
            process_generation: 0,
            scan: None,
            create_bottle: None,
            import_bottles: None,
            import_generation: 0,
            host_terminals: None,
            host_terminal_error: None,
            desktop_status: None,
            desktop_error: None,
            desktop_installing: false,
            winetricks: WinetricksState::default(),
            program_settings: ProgramSettingsPage::default(),
            translation_page: crate::translation::TranslationPage::default(),
            capture: crate::capture::CaptureState::default(),
            move_entry: None,
            open_file: None,
            catalogue_ready: false,
            desktop_resolution_generation: 0,
            desktop_resolutions: HashMap::new(),
            external_opens: std::collections::VecDeque::new(),
            game_info: None,
            bottle_dialog: None,
            wine_engines: Vec::new(),
            wine_engines_error: None,
            engine_selections: HashMap::new(),
            engine_dialog: None,
            pending_engine_action: None,
            engine_folder_pending: false,
            bottle_operation: None,
            pending_path_action: None,
            select_after_refresh: None,
            diagnostics_report: None,
            diagnostics_pending: false,
            diagnostics_error: None,
            diagnostic_checks: None,
            checks_pending: false,
            audio_result: None,
            audio_pending: false,
            audio_error: None,
            setup_status: None,
            gptk_status: None,
            gptk_importing: false,
            gptk_error: None,
            wine_update_checked: false,
            installed_app: (!demo).then(crate::app_update::InstalledApp::detect).flatten(),
            app_update: crate::app_update::State::default(),
            setup_dialog: false,
            setup_installing: false,
            setup_stage: None,
            setup_progress: None,
            setup_error: None,
            setup_prompted: false,
            default_bottle_location: Self::default_bottle_location(),
            env_original: None,
            env_clear_on_reply: false,
            env_fill: None,
            rating_edits: PendingEdits::default(),
            hidden_edits: PendingEdits::default(),
            tag_edits: PendingEdits::default(),
            launch_options_edits: PendingEdits::default(),
            tag_filter: library_view::TagFilter::All,
            studio_filter: library_view::StudioFilter::All,
            group_by_studio: library_ui.group_by_studio,
            collapsed_studios: BTreeSet::new(),
            filter_bar_tab: library_ui.filter_bar_tab,
            tag_bar_mode: library_ui.tag_bar_mode,
            tag_bar_hint_seen,
            tag_bar_hint_peeking: false,
            tag_bar_hint_generation: 0,
            tag_bar_hint_scheduled: false,
            pull_reveal,
            pull_idle_generation: 0,
            launch_chooser: None,
            launch_chooser_generation: 0,
            launch_chooser_return_focus: None,
            executable_picker_generation: 0,
            preview_sheet_generation: 0,
            preview_sheet_return_focus: None,
            preview_sheet_return_captured_gen: 0,
            tag_input,
            tag_input_subscription,
            tag_input_fill: None,
            tag_input_refocus: false,
            tag_manager_rename_target: None,
            tag_manager_input,
            tag_manager_input_subscription,
            tag_manager_fill: None,
            run_settings_target_exe: None,
            sidebar: SidebarMode::Expanded,
            sidebar_dirty: false,
            sidebar_collapsed_style: SidebarMode::Icons,
            settings: false,
            settings_section: SettingsSection::default(),
            settings_window: DetachedWindow::Closed,
            capture_window: DetachedWindow::Closed,
            about: false,
            about_tab: AboutTab::default(),
            app_info: None,
            quitting: false,
            quit_generation: 0,
            quit_error: None,
            motion: true,
            lively: false,
            reversed: false,
            library_scroll: std::array::from_fn(|_| ScrollHandle::new()),
            detail_scroll: ScrollHandle::new(),
            scroll_needs_measurement: Default::default(),
            #[cfg(feature = "visual-test")]
            motion_samples: Default::default(),
            mixed: preferences::DEFAULT_MIXED_COVER_LAYOUT,
            cover_width: preferences::DEFAULT_COVER_WIDTH,
            cover_unit: std::cell::Cell::new(crate::library_layout::DEFAULT_COVER_WIDTH),
            cover_span: std::cell::Cell::new(crate::library_layout::shelf_available(1000.)),
            artwork_cache: crate::artwork::ArtworkCache::new(cx),
            cover_upscale: crate::upscale::Factor::from_raw(preferences::DEFAULT_COVER_UPSCALE),
            cover_engine: crate::upscale::Engine::from_raw(preferences::DEFAULT_COVER_UPSCALE_ENGINE),
            backdrop_blur: preferences::DEFAULT_BACKDROP_BLUR,
            cover_derivatives: RefCell::new(HashMap::new()),
            cover_jobs: RefCell::new(std::collections::HashSet::new()),
            cover_batch: None,
            cover_batch_generation: 0,
            cover_batch_cancel: None,
            cover_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            hovered: None,
            ambient_index: 0,
            immersive_slide: 0,
            immersive_hero_hovered: false,
            immersive_carousel_epoch: Default::default(),
            immersive_carousel_running: Default::default(),
            immersive_row_scrolls: Default::default(),
            detail: None,
            detail_open: false,
            detail_open_generation: 0,
            detail_return_focus: None,
            detail_character: None,
            detail_tags_expanded: false,
            appearance,
            language,
            rendered_locale,
            notice: None,
            runtime: RuntimeTable::default(),
            active_backends: HashMap::new(),
            active_executables: HashMap::new(),
            clipboard_prompt: None,
            legacy_fullscreen_prompt: None,
            bridge,
            bridge_status: BridgeStatus::Locating,
            service_methods: BTreeSet::new(),
            bridge_dialog: false,
            _subscriptions: vec![
                subscription,
                dependency_subscription,
                activation,
                appearance_subscription,
            ],
        }
    }
    pub(crate) fn bridge_retry(&mut self, cx: &mut Context<Self>) {
        self.bridge.retry();
        self.bridge_status = BridgeStatus::Locating;
        self.bridge_dialog = false;
        self.pending.clear();
        self.failures.clear();
        self.load_error = None;
        self.loading = false;
        self.runtime = RuntimeTable::default();
        self.configured.clear();
        self.rating_edits.clear();
        self.hidden_edits.clear();
        self.clipboard_prompt = None;
        self.legacy_fullscreen_prompt = None;
        self.active_executables.clear();
        self.bottle_dialog = None;
        self.bottle_operation = None;
        self.pending_path_action = None;
        self.select_after_refresh = None;
        self.reset_bottle_surfaces();
        cx.notify();
    }

    /// Drop the bottle-page caches: a new helper session may have a different
    /// configuration file, folder list, and environment.
    fn reset_bottle_surfaces(&mut self) {
        self.config = None;
        self.config_writes.clear();
        self.config_text_pending.clear();
        self.config_text_generation = self.config_text_generation.wrapping_add(1);
        self.preview.config_inputs.clear();
        self.preview.folder_inputs.clear();
        self.preview.program_inputs.clear();
        self.preview.config_resets.clear();
        self.program_settings = ProgramSettingsPage::default();
        self.move_entry = None;
        self.game_info = None;
        if matches!(
            self.preview.sheet,
            Some(
                PreviewSheet::MoveEntry(_)
                    | PreviewSheet::GameInfo(_)
                    | PreviewSheet::GameTags(_)
            )
        ) {
            self.preview.sheet = None;
            self.preview.sheet_history.clear();
        }
        self.folders.clear();
        self.folders_pending.clear();
        self.folders_error.clear();
        self.folders_reload_after_write.clear();
        self.auto_synced.clear();
        self.scan_root_picker = None;
        self.launcher_prefs.clear();
        self.launchers_pending.clear();
        // `launcher_autostarted` survives: a reconnect must not relaunch what
        // the session already started.
        self.executable_picker = None;
        self.launch_chooser = None;
        self.run_settings_target_exe = None;
        self.tag_manager_rename_target = None;
        self.tag_edits.clear();
        self.launch_options_edits.clear();
        self.environments.clear();
        self.environment_error.clear();
        self.env_original = None;
        self.env_clear_on_reply = false;
        self.env_fill = None;
        self.diagnostics_report = None;
        self.diagnostics_pending = false;
        self.diagnostics_error = None;
        self.diagnostic_checks = None;
        self.checks_pending = false;
        self.audio_result = None;
        self.audio_pending = false;
        self.audio_error = None;
        self.pending_path_action = None;
        self.programs = None;
        self.processes = None;
        self.winetricks = WinetricksState::default();
        self.stop_process_polling();
        if self.scan.as_ref().is_some_and(ScanState::running) {
            // The job lived in the previous session.
            self.scan = None;
        }
    }

    /// Route a supervisor event into the library view model. Errors never panic
    /// and never discard entries that are already on screen.
    pub(crate) fn apply_bridge_event(&mut self, event: BridgeEvent, cx: &mut Context<Self>) {
        match event {
            BridgeEvent::ShutdownFailed { reason } => {
                self.quitting = false;
                self.app_update.pending = None;
                self.quit_error = Some(reason);
                self.bridge_dialog = true;
                cx.notify();
            }
            BridgeEvent::Status(status) => {
                if let BridgeStatus::Connected {
                    helper,
                    version,
                    methods,
                } = &status
                {
                    eprintln!(
                        "whisky-bridge: connected to {helper} ({version}, {methods} methods)"
                    );
                }
                let connected = matches!(&status, BridgeStatus::Connected { .. });
                let unavailable =
                    matches!(&status, BridgeStatus::Failed { .. } | BridgeStatus::Stopped);
                let reason = match &status {
                    BridgeStatus::Failed { reason, .. } => Some(reason.clone()),
                    BridgeStatus::Stopped => {
                        Some(crate::i18n::s().state.bridge_not_running.to_owned())
                    }
                    _ => None,
                };
                let stopped = matches!(&status, BridgeStatus::Stopped);
                self.bridge_status = status;
                if unavailable {
                    self.translation_page.bridge_disconnected(reason.as_deref());
                    self.capture.disconnected(reason.as_deref());
                    cx.notify();
                }
                if !connected {
                    self.update_preferences_loaded = false;
                    self.app_update.busy = false;
                    self.app_update.version = None;
                    self.app_update.downloaded = None;
                    self.service_methods.clear();
                    self.setup_status = None;
                    self.gptk_status = None;
                    self.gptk_importing = false;
                    self.gptk_error = None;
                }
                if connected && !self.demo {
                    // One read per connected session; the reply carries both
                    // the stored values and the writable-key schema.
                    self.bridge.load_preferences();
                    self.bridge.load_system_info();
                }
                if stopped && self.quitting {
                    cx.quit();
                    return;
                }
                if unavailable && !self.demo {
                    let st = &crate::i18n::s().state;
                    self.catalogue_ready = false;
                    self.external_opens.extend(self.desktop_resolutions.drain().map(|(_, file)| file));
                    self.host_terminals = None;
                    self.host_terminal_error = None;
                    self.desktop_status = None;
                    self.desktop_error = None;
                    self.desktop_installing = false;
                    if let Some(page) = self.game_info.as_mut().filter(|page| page.matches.is_none()) {
                        page.config_error = Some(st.bridge_reconnect_gamedb.into());
                    }
                    if let Some(state) = self.import_bottles.as_mut() {
                        state.failed(state.request, st.bridge_disconnected_rescan.into());
                    }
                    self.pending.clear();
                    self.configured.clear();
                    self.rating_edits.clear();
                    self.hidden_edits.clear();
                    // An approval request in flight has no session left to
                    // answer it; its queued write must not fire on reconnect.
                    self.pending_engine_action = None;
                    // A pending clipboard token belongs to a session that is
                    // gone; re-launching from the card gets a fresh one.
                    self.clipboard_prompt = None;
                    self.legacy_fullscreen_prompt = None;
                    self.active_executables.clear();
                    if self.entries.is_empty() {
                        self.load_error = reason;
                    }
                }
            }
            BridgeEvent::TranslationReply { request, method, result } => {
                let cache_cleared_ok = method == "translation.cache.clear" && result.is_ok();
                self.translation_page
                    .reply(request, &method, result.clone(), &self.bridge, cx);
                self.capture.reply(request, &method, result, &self.bridge, cx);
                if cache_cleared_ok
                    && self.capture.panel_open
                    && !self.capture.is_pending("translation.history")
                {
                    self.capture.refresh_history(&self.bridge, 0);
                }
                if matches!(
                    method.as_str(),
                    "translation.providers/list"
                        | "translation.providers/save"
                        | "translation.providers/delete"
                        | "translation.capture.preset/get"
                        | "translation.capture.preset/save"
                ) {
                    self.sync_capture_preset_and_provider();
                }
            }
            BridgeEvent::Services(methods) => {
                self.service_methods = methods.into_iter().collect();
                self.maybe_check_updates();
                self.localize_native_capture_popup();
                if self.service_available("gameConfig.match") {
                    if let Some(page) = self.game_info.as_mut().filter(|page| page.matches.is_none() && page.config_error.is_some()) {
                        page.config_error = None;
                        self.bridge.match_game_config(&page.container, &page.entry_id);
                    }
                }
            }
            BridgeEvent::Containers(containers) => {
                if !self.demo {
                    let previous = self
                        .selected_bottle
                        .and_then(|index| self.bottles.get(index).map(|bottle| bottle.path.clone()));
                    self.bottles = containers
                        .into_iter()
                        .map(|container| BottleVm {
                            path: container.path,
                            name: container.name,
                            available: container.available,
                        })
                        .collect();
                    // A reconnect keeps the selected bottle when it still exists.
                    self.selected_bottle = previous
                        .and_then(|path| self.bottles.iter().position(|bottle| bottle.path == path));
                    self.bottle_entries.clear();
                    self.entries.clear();
                    self.failures.clear();
                    self.load_error = None;
                    self.pending.clear();
                    // The helper keeps no runtime session across processes.
                    self.runtime = RuntimeTable::default();
                    self.configured.clear();
                    self.rating_edits.clear();
                    self.hidden_edits.clear();
                    self.reset_bottle_surfaces();
                    self.invalidate_view();
                    self.request_all();
                    // A reconnect must re-configure, or every card would stay
                    // disabled with no way back short of reselecting the bottle.
                    if let Some(path) = self
                        .selected_bottle
                        .and_then(|index| self.bottles.get(index))
                        .filter(|bottle| bottle.available)
                        .map(|bottle| bottle.path.clone())
                        && self.configured.ensure(&path)
                    {
                        self.bridge.configure_runtime(&path);
                    }
                    self.catalogue_ready = true;
                    while let Some(file) = self.external_opens.pop_front() {
                        self.receive_external_file(file, cx);
                    }
                }
            }
            BridgeEvent::DesktopResolved { request, result, retryable } => {
                if let Some(original) = self.desktop_resolutions.remove(&request) {
                    match result {
                        Ok(file) => self.receive_external_file(file, cx),
                        Err(_) if retryable => { self.catalogue_ready = false; self.external_opens.push_back(original); }
                        Err(reason) => self.show_notice(reason, cx),
                    }
                }
            }
            BridgeEvent::HostDesktop { install, result } => {
                if install { self.desktop_installing = false; }
                match result {
                    Ok(status) => {
                        if install {
                            let st = &crate::i18n::s().state;
                            self.show_notice(
                                if status.cache_updated == Some(false) {
                                    st.desktop_installed_cache_pending
                                } else if status.updated {
                                    st.desktop_updated
                                } else {
                                    st.desktop_added
                                },
                                cx,
                            );
                        }
                        self.desktop_status = Some(status);
                        self.desktop_error = None;
                    }
                    Err(reason) => self.desktop_error = Some(reason),
                }
            }
            BridgeEvent::HostTerminals { result } => {
                match result {
                    Ok(terminals) => { self.host_terminals = Some(terminals); self.host_terminal_error = None; }
                    Err(reason) => { self.host_terminals = None; self.host_terminal_error = Some(reason); }
                }
            }
            BridgeEvent::ImportCandidates { request, candidates } => {
                if let Some(state) = self.import_bottles.as_mut() { state.loaded(request, candidates); }
            }
            BridgeEvent::ImportCandidatesFailed { request, reason } => {
                if let Some(state) = self.import_bottles.as_mut() { state.failed(request, reason); }
            }
            BridgeEvent::CandidatesImported { request, paths, count } => {
                if let Some(state) = self.import_bottles.as_mut() { state.imported(request, &paths, count); }
            }
            BridgeEvent::ContainersRefreshed(containers) => {
                if !self.demo {
                    self.apply_refreshed_containers(containers);
                    if let Some(path) = self.select_after_refresh.take() {
                        if let Some(index) = self
                            .bottles
                            .iter()
                            .position(|bottle| bottle.path == path)
                        {
                            self.select_bottle(Some(index), cx);
                        }
                    }
                    if self
                        .bottle_dialog
                        .as_ref()
                        .is_some_and(|dialog| {
                            !self
                                .bottles
                                .iter()
                                .any(|bottle| bottle.path == dialog.container)
                        })
                    {
                        self.bottle_dialog = None;
                    }
                }
            }
            BridgeEvent::ActionFailed { reason } => {
                if !self.demo {
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::FolderChosen { path } => {
                if self.engine_folder_pending {
                    self.engine_folder_pending = false;
                    if let Some(path) = path {
                        self.bridge.register_wine_engine(&path, None);
                    }
                } else if let Some(path) = path {
                    self.default_bottle_location = Some(path.clone());
                    self.persist(preferences::DEFAULT_BOTTLE_LOCATION, json!(path), cx);
                    self.show_notice(crate::i18n::s().state.default_bottle_location_updated, cx);
                }
            }
            BridgeEvent::FolderChooseFailed { reason } => {
                if !self.demo {
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::Runtime(update) => {
                if !self.demo {
                    // A poll for the previous container must not leak into the
                    // new one; `resync` already clears, this guards the delta.
                    if self
                        .selected_bottle
                        .and_then(|index| self.bottles.get(index))
                        .is_none_or(|bottle| bottle.path == update.container)
                    {
                        // `libraryRefreshAfterExit`: the activity store gains
                        // `lastSessionEndedAt` when a session ends, and the
                        // recent sort only sees it after the entries are
                        // re-read (Swift: `LibraryModel.consume`).
                        let exited_ids: Vec<String> = update
                            .entries
                            .iter()
                            .filter(|entry| {
                                RuntimeState::from_wire(&entry.state) == RuntimeState::Idle
                                    && self
                                        .runtime
                                        .entry(&entry.id)
                                        .is_some_and(|previous| previous.state.is_active())
                            })
                            .map(|entry| entry.id.clone())
                            .collect();
                        let container = update.container.clone();
                        let idle_ids: Vec<String> = update
                            .entries
                            .iter()
                            .filter(|entry| entry.state == "idle")
                            .map(|entry| entry.id.clone())
                            .collect();
                        for entry_id in &idle_ids {
                            self.active_backends.remove(entry_id);
                        }
                        self.runtime.apply(update);
                        // Each finished run re-reads its settings: the newest
                        // run-history entry carries the display-mode failure
                        // flag, and the probe only prompts when the fix is
                        // neither applied nor permanently dismissed.
                        for entry_id in &exited_ids {
                            if let Some(executable) = self.resolve_exited_executable(entry_id) {
                                self.bridge
                                    .probe_legacy_fullscreen(&container, &executable, entry_id);
                            }
                        }
                        for entry_id in &idle_ids {
                            self.active_executables.remove(entry_id);
                        }
                        if !exited_ids.is_empty()
                            && self
                                .preview
                                .toggles
                                .get("refresh")
                                .copied()
                                .unwrap_or(preferences::DEFAULT_REFRESH_AFTER_EXIT)
                        {
                            self.bridge.load_entries(&container);
                        }
                    }
                }
            }
            BridgeEvent::RuntimeLaunched {
                container,
                entry_id,
                backend,
                backend_from,
            } => {
                if !self.demo {
                    #[cfg(target_os = "linux")]
                    {
                        self.capture.presets_by_game.remove(&(container.clone(), entry_id.clone()));
                        self.capture.hud.launch_requested = Some((container.clone(), entry_id.clone()));
                        self.capture.load_preset_for_game(&self.bridge, &container, &entry_id);
                    }
                    #[cfg(not(target_os = "linux"))]
                    let _ = container;
                    // The badge keeps the effective backend alone; the toast
                    // spells out a fallback when one was taken.
                    let strings = crate::i18n::s();
                    self.active_backends.insert(
                        entry_id.clone(),
                        backend
                            .clone()
                            .unwrap_or_else(|| strings.runtime.by_launch_config.to_owned()),
                    );
                    let backend_label = backend_label(backend, backend_from);
                    let title = self
                        .entries
                        .iter()
                        .find(|e| e.id == entry_id)
                        .map(|e| e.title.as_str())
                        .unwrap_or(strings.state.fallback_game);
                    self.show_notice(
                        crate::i18n::s().state.notice_launching_with_backend(&backend_label, title),
                        cx,
                    );
                }
            }
            BridgeEvent::ProgramRunStarted {
                name,
                backend,
                backend_from,
            } => {
                if !self.demo {
                    let backend_label = backend_label(backend, backend_from);
                    self.show_notice(
                        crate::i18n::s().state.notice_launching_with_backend(&backend_label, &name),
                        cx,
                    );
                }
            }
            BridgeEvent::ClipboardPrompt(prompt) => {
                if !self.demo {
                    self.clipboard_prompt = Some(prompt);
                }
            }
            BridgeEvent::LegacyFullscreenPrompt {
                container,
                executable,
                program_name,
                entry_id,
            } => {
                if !self.demo {
                    self.legacy_fullscreen_prompt = Some(LegacyFullscreenPrompt {
                        container,
                        executable,
                        program_name,
                        entry_id,
                        enable: true,
                    });
                }
            }
            BridgeEvent::RuntimeFailed { reason } => {
                if !self.demo {
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::ConfigureFailed { container, reason } => {
                if !self.demo {
                    // The set only records successes; let the next Play retry.
                    self.configured.forget(&container);
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::Entries { container, entries } => {
                if !self.demo
                    && let Some(index) = self
                        .bottles
                        .iter()
                        .position(|bottle| bottle.path == container)
                {
                    let slot = index + 1;
                    let games: Vec<Game> = entries
                        .into_iter()
                        .map(|entry| {
                            let mut game = entry.into_game(slot);
                            // Tag by the bottle that answered the request, not
                            // by a lookup that could race a catalogue change.
                            game.container = container.clone();
                            game
                        })
                        .collect();
                    self.bottle_entries.insert(container.clone(), games);
                    self.rebuild_entries();
                    self.maybe_start_tag_bar_hint(cx);
                    self.pending.remove(&index);
                    self.failures.remove(&index);
                    self.maybe_auto_sync_metadata(&container);
                    self.ensure_launchers(&container);
                    self.maybe_autostart_launchers(&container, cx);
                }
            }
            BridgeEvent::Launchers { container, auto_start } => {
                if !self.demo {
                    self.launchers_pending.remove(&container);
                    self.launcher_prefs
                        .insert(container.clone(), auto_start.into_iter().collect());
                    self.maybe_autostart_launchers(&container, cx);
                    cx.notify();
                }
            }
            BridgeEvent::LaunchersFailed { container, reason } => {
                if !self.demo {
                    self.launchers_pending.remove(&container);
                    self.launcher_prefs.entry(container).or_default();
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::LoadFailed { container, reason } => {
                if !self.demo {
                    let index = container.and_then(|container| {
                        self.bottles
                            .iter()
                            .position(|bottle| bottle.path == container)
                    });
                    match index {
                        Some(index) => {
                            self.pending.remove(&index);
                            self.failures.insert(index, reason);
                        }
                        None => {
                            self.pending.clear();
                            self.load_error = Some(reason);
                        }
                    }
                }
            }
            BridgeEvent::Preferences { values, schema } => {
                if !self.demo {
                    self.prefs_supported = schema.keys().map(str::to_owned).collect();
                    if self.sidebar_dirty {
                        // Changes made before the schema arrived were dropped
                        // by `persist`; flush the live mode now that the key
                        // is known so storage matches what the user sees.
                        self.persist(preferences::SIDEBAR_MODE, json!(self.sidebar.raw()), cx);
                        self.persist(
                            preferences::SIDEBAR_COLLAPSED_STYLE,
                            json!(self.sidebar_collapsed_style.raw()),
                            cx,
                        );
                    }
                    self.apply_preferences(&values);
                    self.update_preferences_loaded = true;
                    self.maybe_check_updates();
                    if !self.onboarding_checked {
                        self.onboarding_checked = true;
                        if values.get(crate::onboarding::COMPLETED).and_then(serde_json::Value::as_bool) == Some(false)
                            && self.service_available("preferences.setMany")
                        {
                            self.open_onboarding(true, cx);
                        }
                    }
                }
            }
            BridgeEvent::MetadataPreferencesSaved { result, onboarding } => {
                match result {
                    Ok(values) => {
                        self.apply_preferences(&values);
                        if onboarding {
                            self.onboarding = None;
                            self.maybe_start_tag_bar_hint(cx);
                        }
                    }
                    Err(error) => {
                        if onboarding && let Some(state) = &mut self.onboarding {
                            state.saving = false;
                            state.error = Some(error);
                        } else {
                            self.show_notice(&crate::i18n::s().state.notice_metadata_unsaved(&error), cx);
                            self.bridge.load_preferences();
                        }
                    }
                }
            }
            BridgeEvent::SystemInfo { info } => {
                self.app_info = Some(info);
            }
            BridgeEvent::AppUpdate { action, result } => self.app_update_reply(action, result),
            BridgeEvent::AppUpdateQuitAccepted => {
                if let Some(pending) = self.app_update.pending.take()
                    && let Err(error) = pending.authorize()
                {
                    eprintln!("maltoria: could not authorize app update: {error}");
                }
            }
            BridgeEvent::EntryRating {
                container,
                entry_id,
                activity,
            } => {
                if !self.demo {
                    let key = entry_key(&container, &entry_id);
                    self.rating_edits.succeed(&key);
                    // The reply is the activity record itself; never re-derive
                    // the value from what was requested.
                    self.update_entry(&container, &entry_id, |game| {
                        game.rating = activity.rating();
                        game.play_time = activity.play_time();
                        game.daily_play_time = activity.daily_play_time.clone();
                        game.last_played = activity.last_played();
                        game.last_session_ended_at = activity.last_session_ended_at();
                    });
                    self.rebuild_entries();
                }
            }
            BridgeEvent::EntryHidden {
                container,
                entry_id,
                hidden,
            } => {
                if !self.demo {
                    let key = entry_key(&container, &entry_id);
                    self.hidden_edits.succeed(&key);
                    self.update_entry(&container, &entry_id, |game| game.hidden = hidden);
                    self.rebuild_entries();
                }
            }
            BridgeEvent::EntryTagsSaved {
                container,
                entry_id,
                tags,
            } => {
                if !self.demo {
                    let key = entry_key(&container, &entry_id);
                    self.tag_edits.succeed(&key);
                    let normalized = library_view::normalize_tags(tags);
                    self.update_entry(&container, &entry_id, |game| {
                        game.tags = normalized.clone();
                    });
                    self.rebuild_entries();
                }
            }
            BridgeEvent::EntryLaunchOptionsSaved {
                container,
                entry_id,
                multi_executable,
                executables,
                last_executable,
            } => {
                if !self.demo {
                    let key = entry_key(&container, &entry_id);
                    self.launch_options_edits.succeed(&key);
                    self.update_entry(&container, &entry_id, |game| {
                        game.multi_executable = multi_executable;
                        game.launch_executables = executables.clone();
                        game.last_executable = last_executable.clone();
                        if !executables.is_empty() {
                            game.needs_executable = false;
                            let current_is_valid = game.executable.as_ref().is_some_and(|path| {
                                path.extension()
                                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                            });
                            if !current_is_valid {
                                let fallback = last_executable
                                    .as_deref()
                                    .unwrap_or(&executables[0]);
                                game.executable = Some(std::path::PathBuf::from(fallback));
                            }
                        }
                    });
                    if let Some(chooser) = self.launch_chooser.as_mut()
                        && chooser.container == container
                        && chooser.entry_id == entry_id
                    {
                        chooser.executables = executables.clone();
                        chooser.last_executable = last_executable.clone();
                    }
                    self.rebuild_entries();
                }
            }
            BridgeEvent::TagRenamed {
                old_tag,
                new_tag,
                updated,
            } => {
                if !self.demo {
                    self.apply_local_tag_rename(&old_tag, &new_tag);
                    self.rebuild_entries();
                    if updated > 0 {
                        self.show_notice(
                            crate::i18n::s().state.notice_tag_renamed(&new_tag, &old_tag),
                            cx,
                        );
                    }
                }
            }
            BridgeEvent::TagDeleted { tag, updated } => {
                if !self.demo {
                    self.apply_local_tag_delete(&tag);
                    self.rebuild_entries();
                    if updated > 0 {
                        self.show_notice(crate::i18n::s().state.notice_tag_deleted(&tag), cx);
                    }
                }
            }
            BridgeEvent::TagOperationFailed { reason } => {
                if !self.demo {
                    self.show_notice(reason, cx);
                    self.refresh_library(cx);
                }
            }
            BridgeEvent::EntryFailed {
                mutation,
                container,
                entry_id,
                reason,
            } => {
                if !self.demo {
                    self.roll_back(mutation, &container, &entry_id);
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::Configuration {
                container,
                capabilities,
                schema,
                values,
            } => {
                if !self.demo {
                    let changed = self
                        .config
                        .as_ref()
                        .is_none_or(|page| page.container != container);
                    if changed {
                        // A different bottle means different input entities.
                        self.preview.config_inputs.clear();
                        self.preview.config_resets.clear();
                        self.config_writes.clear();
                        self.config_text_pending.clear();
                        self.config_text_generation =
                            self.config_text_generation.wrapping_add(1);
                    }
                    self.config = Some(ConfigPage {
                        container,
                        capabilities,
                        fields: schema,
                        values,
                        loading: false,
                        error: None,
                    });
                }
            }
            BridgeEvent::ConfigurationSaved {
                container,
                field,
                values,
            } => {
                if !self.demo {
                    self.config_writes.succeed(&field);
                    if let Some(page) = self
                        .config
                        .as_mut()
                        .filter(|page| page.container == container)
                    {
                        // Apply only the field that was written: another
                        // debounced edit may still be in flight and must not be
                        // clobbered by this readback.
                        if let Some(value) = values.get(&field) {
                            page.values.insert(field, value.clone());
                        }
                    }
                }
            }
            BridgeEvent::ConfigurationFailed {
                container,
                field,
                reason,
            } => {
                if !self.demo {
                    match field {
                        None => {
                            if let Some(page) = self
                                .config
                                .as_mut()
                                .filter(|page| page.container == container)
                            {
                                page.loading = false;
                                page.error = Some(reason.clone());
                            }
                        }
                        Some(field) => {
                            if let Some(previous) = self.config_writes.fail(&field) {
                                if let Some(page) = self
                                    .config
                                    .as_mut()
                                    .filter(|page| page.container == container)
                                {
                                    page.values.insert(field.clone(), previous.clone());
                                    self.preview.config_resets.insert(field);
                                }
                            }
                        }
                    }
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::Candidates { candidates } => {
                if let Some(picker) = self.executable_picker.as_mut() {
                    // The helper echoes the folder it walked; the reply may
                    // also be for a folder the user has since left, so both
                    // come from the reply rather than from the request.
                    if !candidates.folder.is_empty() {
                        picker.folder = candidates.folder;
                    }
                    picker.loading = false;
                    picker.error = None;
                    picker.candidates = candidates.candidates;
                }
            }
            BridgeEvent::CandidatesFailed { folder, reason } => {
                if let Some(picker) = self.executable_picker.as_mut()
                    && (picker.folder == folder || folder.is_empty())
                {
                    picker.loading = false;
                    picker.error = Some(reason.clone());
                }
                self.show_notice(reason, cx);
            }
            BridgeEvent::WineEngines { list } => {
                self.wine_engines = list.engines;
                self.wine_engines_error = list.load_error;
                if let Some(status) = self.setup_status.as_mut() {
                    status.any_engine_available = Some(
                        status.managed_installed.unwrap_or(status.installed)
                            || self.wine_engines.iter().any(|engine| engine.available),
                    );
                }
                let resolved = self.resolved_create_engine_id();
                if self
                    .wine_engines
                    .iter()
                    .any(|engine| engine.id == resolved && engine.available)
                {
                    self.preview.choices.insert("create-engine", resolved.into());
                }
                cx.notify();
            }
            BridgeEvent::WineEngineOpFailed { reason } => {
                if !self.demo {
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::EngineSelection {
                container,
                selection,
            } => {
                // The reply is tagged with its container: a stale readback for
                // a previously selected bottle lands in its own slot and never
                // leaks into the page the user is looking at.
                let applies_pending = self
                    .pending_engine_action
                    .as_ref()
                    .is_some_and(|pending| pending.container == container)
                    && self.engine_action_confirmed(&selection);
                let verdict = if applies_pending {
                    self.pending_engine_action.as_ref().map(|pending| {
                        self.pending_apply_verdict(&selection, &pending.apply)
                    })
                } else {
                    None
                };
                self.engine_selections.insert(container, *selection);
                match verdict {
                    Some(PendingVerdict::Ready) => {
                        if let Some(pending) = self.pending_engine_action.take() {
                            self.apply_pending_engine_action(pending, cx);
                        }
                    }
                    // One prerequisite approved but the value still needs
                    // another: re-open the confirmation for the NEXT unmet
                    // feature, the same apply stays queued behind it.
                    Some(PendingVerdict::NextApproval(next_feature)) => {
                        if let Some(pending) = self.pending_engine_action.take() {
                            let fingerprint = self
                                .engine_selections
                                .get(&pending.container)
                                .and_then(|selection| selection.engine.fingerprint.clone())
                                .unwrap_or_default();
                            self.engine_dialog = Some(EngineDialogState {
                                kind: EngineDialogKind::Feature {
                                    feature: next_feature.clone(),
                                    apply: pending.apply,
                                    reason: crate::i18n::s().state.engine_another_unverified.to_owned(),
                                    fingerprint,
                                },
                                container: pending.container,
                                engine_id: pending.engine_id,
                            });
                        }
                    }
                    // Hard-missing prerequisite surfaced after approval —
                    // nothing can make the write valid; drop it.
                    Some(PendingVerdict::Blocked) => {
                        self.pending_engine_action = None;
                        self.show_notice(crate::i18n::s().state.engine_missing_component.to_owned(), cx);
                    }
                    None => {}
                }
                cx.notify();
            }
            BridgeEvent::EngineSelectionFailed { container, reason } => {
                // A rejected approval (stale fingerprint, vanished payload)
                // must not apply the write it was guarding.
                if self
                    .pending_engine_action
                    .as_ref()
                    .is_some_and(|pending| pending.container == container)
                {
                    self.pending_engine_action = None;
                }
                if self
                    .selected_container()
                    .is_some_and(|selected| selected == container)
                {
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::Folders { container, folders } => {
                if !self.demo {
                    self.folders_pending.remove(&container);
                    self.folders_error.remove(&container);
                    let changed = self.folders_reload_after_write.remove(&container);
                    let automatic_turned_on = folders.automatic_metadata
                        && !self
                            .folders
                            .get(&container)
                            .is_some_and(|current| current.automatic_metadata);
                    self.folders.insert(container.clone(), folders);
                    if automatic_turned_on {
                        // Switching the library's own match back on earns a
                        // fresh pass over its pending entries.
                        self.auto_synced.remove(&container);
                        self.maybe_auto_sync_metadata(&container);
                    }
                    if changed {
                        // Roots, assignment, exclusion, visibility or the
                        // executable choice all change how entries are read.
                        self.bottle_entries.remove(&container);
                        self.bridge.load_entries(&container);
                    }
                }
            }
            BridgeEvent::FoldersFailed { container, reason } => {
                if !self.demo {
                    self.folders_pending.remove(&container);
                    self.folders_reload_after_write.remove(&container);
                    self.folders_error
                        .insert(container, reason.clone());
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::FoldersChosen { purpose, paths } => {
                if !self.demo && let Some(container) = self.scan_root_picker.take() {
                    let paths: Vec<String> = paths
                        .into_iter()
                        .filter(|path| path.starts_with('/'))
                        .collect();
                    if !paths.is_empty() {
                        self.folders_reload_after_write.insert(container.clone());
                        match purpose.as_str() {
                            "scan-roots" => self.bridge.add_scan_roots(&container, &paths),
                            // Standalone game folders assign rather than scan:
                            // the folder itself is the game, not its children.
                            "assign-folders" => {
                                self.bridge.assign_game_folders(&container, &paths);
                            }
                            _ => {}
                        }
                    }
                }
            }
            BridgeEvent::ScanRootMoved { source, target } => {
                if !self.demo {
                    self.show_notice(crate::i18n::s().state.scan_root_moved, cx);
                    self.folders_reload_after_write.remove(&source);
                    self.folders_reload_after_write.remove(&target);
                    self.refresh_library(cx);
                }
            }
            BridgeEvent::Environment { container, values } => {
                if !self.demo {
                    self.environment_error.remove(&container);
                    self.environments.insert(container, values);
                    if self.env_clear_on_reply {
                        self.env_clear_on_reply = false;
                        self.env_original = None;
                        // Applied on the next render, which has a window.
                        self.env_fill = Some((String::new(), String::new()));
                    }
                }
            }
            BridgeEvent::EnvironmentFailed { container, reason } => {
                if !self.demo {
                    self.env_clear_on_reply = false;
                    self.environment_error
                        .insert(container, reason.clone());
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::Programs {
                container,
                programs,
                settled,
            } => {
                if !self.demo {
                    if let Some(page) = self
                        .programs
                        .as_mut()
                        .filter(|page| page.container == container)
                    {
                        if let Some((mutation, path)) = &settled {
                            match mutation {
                                ProgramMutation::Pin => page.table.commit_pin(path),
                                ProgramMutation::Block => page.table.commit_block(path),
                            }
                        }
                        page.table.apply(programs);
                        page.loading = false;
                        page.error = None;
                    }
                    if settled.is_some() {
                        // Pinned programs appear in (and leave) the library:
                        // `workspace.entries` is built from the pin list.
                        self.bridge.load_entries(&container);
                    }
                }
            }
            BridgeEvent::ProgramsFailed { container, reason } => {
                if !self.demo {
                    if let Some(page) = self
                        .programs
                        .as_mut()
                        .filter(|page| page.container == container)
                    {
                        page.loading = false;
                        page.error = Some(reason.clone());
                    }
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::ProgramFailed {
                mutation,
                container,
                path,
                reason,
            } => {
                if !self.demo {
                    if let Some(page) = self
                        .programs
                        .as_mut()
                        .filter(|page| page.container == container)
                    {
                        match mutation {
                            ProgramMutation::Pin => {
                                page.table.rollback_pin(&path);
                            }
                            ProgramMutation::Block => {
                                page.table.rollback_block(&path);
                            }
                        }
                    }
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::Processes { container, processes } => {
                if !self.demo {
                    if let Some(page) = self
                        .processes
                        .as_mut()
                        .filter(|page| page.container == container)
                    {
                        page.table.apply(processes);
                        page.loading = false;
                        page.error = None;
                    }
                }
            }
            BridgeEvent::ProcessesFailed { container, reason } => {
                if !self.demo {
                    if let Some(page) = self
                        .processes
                        .as_mut()
                        .filter(|page| page.container == container)
                    {
                        page.loading = false;
                        page.error = Some(reason.clone());
                    }
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::ProcessKilled {
                container,
                pid,
                killed,
            } => {
                if !self.demo {
                    let restored = self
                        .processes
                        .as_mut()
                        .filter(|page| page.container == container)
                        .is_some_and(|page| {
                            matches!(page.table.finish_kill(pid, killed), KillOutcome::Restored(_))
                        });
                    if restored {
                        self.show_notice(crate::i18n::s().state.kill_process_failed, cx);
                    }
                }
            }
            BridgeEvent::ProcessKillFailed {
                container,
                pid,
                reason,
            } => {
                if !self.demo {
                    if let Some(page) = self
                        .processes
                        .as_mut()
                        .filter(|page| page.container == container)
                    {
                        let _ = page.table.finish_kill(pid, false);
                    }
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::Scan { container, update } => {
                if !self.demo {
                    match update {
                        ScanUpdate::Started { job_id } => {
                            let state = self.scan.get_or_insert_with(ScanState::default);
                            state.container = container;
                            state.job_id = Some(job_id);
                            state.status = Some(JobStatus::Queued);
                            state.stage = None;
                            state.error = None;
                            state.finished = false;
                        }
                        ScanUpdate::Progress {
                            status,
                            stage,
                            loaded,
                            total,
                        } => {
                            let state = self.scan.get_or_insert_with(ScanState::default);
                            state.container = container;
                            state.status = Some(status);
                            state.stage = stage;
                            if let Some(loaded) = loaded {
                                state.loaded = loaded;
                            }
                            if total.is_some() {
                                state.total = total;
                            }
                            state.error = None;
                        }
                        ScanUpdate::Completed { entries } => {
                            let state = self.scan.get_or_insert_with(ScanState::default);
                            state.container = container.clone();
                            state.status = Some(JobStatus::Completed);
                            state.stage = None;
                            state.loaded = entries.len();
                            state.total = Some(entries.len() as u64);
                            state.finished = true;
                            state.error = None;

                            let index = self
                                .bottles
                                .iter()
                                .position(|bottle| bottle.path == container);
                            let slot = index.map_or(0, |index| index + 1);
                            let games: Vec<Game> = entries
                                .into_iter()
                                .map(|entry| {
                                    let mut game = entry.into_game(slot);
                                    game.container = container.clone();
                                    game
                                })
                                .collect();
                            self.bottle_entries.insert(container.clone(), games);
                            // A bottle that vanished mid-scan must not clear
                            // another bottle's pending marker.
                            if let Some(index) = index {
                                self.pending.remove(&index);
                                self.failures.remove(&index);
                            }
                            self.rebuild_entries();
                            // New folders can be pending their first match
                            // even if this bottle already had its pass.
                            self.auto_synced.remove(&container);
                            self.maybe_auto_sync_metadata(&container);
                        }
                        ScanUpdate::Failed { status, reason } => {
                            let state = self.scan.get_or_insert_with(ScanState::default);
                            state.container = container;
                            state.status = Some(status);
                            state.stage = None;
                            state.finished = true;
                            state.error = Some(reason.clone());
                            if status != JobStatus::Cancelled {
                                self.show_notice(reason, cx);
                            }
                        }
                    }
                }
            }
            BridgeEvent::BottleCreate { update } => {
                if !self.demo {
                    let accepted = self
                        .create_bottle
                        .get_or_insert_with(CreateBottleState::default)
                        .apply(&update);
                    if accepted {
                        match update {
                            BottleCreateUpdate::Completed {
                                path,
                                name,
                                containers,
                            } => {
                                self.show_notice(crate::i18n::s().state.notice_bottle_created(&name), cx);

                                // Update the bottle catalogue
                                let previous = self
                                    .selected_bottle
                                    .and_then(|index| self.bottles.get(index).map(|bottle| bottle.path.clone()));
                                self.bottles = containers
                                    .into_iter()
                                    .map(|container| BottleVm {
                                        path: container.path,
                                        name: container.name,
                                        available: container.available,
                                    })
                                    .collect();
                                self.selected_bottle = previous
                                    .and_then(|path| self.bottles.iter().position(|bottle| bottle.path == path));
                                self.bottle_entries.clear();
                                self.entries.clear();
                                self.failures.clear();
                                self.load_error = None;
                                self.pending.clear();
                                self.runtime = RuntimeTable::default();
                                self.configured.clear();
                                self.rating_edits.clear();
                                self.hidden_edits.clear();
                                self.reset_bottle_surfaces();
                                self.invalidate_view();
                                self.request_all();

                                // Select the newly created bottle
                                if let Some(index) = self.bottles.iter().position(|bottle| bottle.path == path) {
                                    self.select_bottle(Some(index), cx);
                                }
                            }
                            BottleCreateUpdate::Failed { status, reason } => {
                                if status != JobStatus::Cancelled {
                                    self.show_notice(reason, cx);
                                }
                            }
                            BottleCreateUpdate::Started { .. }
                            | BottleCreateUpdate::Progress { .. } => {}
                        }
                    }
                }
            }
            BridgeEvent::WinetricksCatalog { catalog } => {
                if !self.demo {
                    self.winetricks.catalog = Some(catalog);
                    self.winetricks.catalog_pending = false;
                    self.winetricks.catalog_error = None;
                    self.winetricks.ensure_category();
                }
            }
            BridgeEvent::WinetricksCatalogFailed { reason } => {
                if !self.demo {
                    self.winetricks.catalog_pending = false;
                    self.winetricks.catalog_error = Some(reason);
                }
            }
            BridgeEvent::WinetricksInstalled { container, verbs } => {
                if !self.demo {
                    self.winetricks.set_installed(&container, verbs);
                }
            }
            BridgeEvent::WinetricksInstalledFailed { container, reason } => {
                if !self.demo {
                    self.winetricks.installed_pending.remove(&container);
                    self.winetricks.installed_error.insert(container, reason);
                }
            }
            BridgeEvent::WinetricksInstall { container, update } => {
                if !self.demo {
                    match update {
                        crate::winetricks::InstallUpdate::Started { job_id } => {
                            let mut job = InstallJob::new(&container);
                            job.job_id = Some(job_id);
                            job.status = Some(JobStatus::Queued);
                            self.winetricks.job = Some(job);
                        }
                        crate::winetricks::InstallUpdate::Progress { status, progress } => {
                            if let Some(job) = self
                                .winetricks
                                .job
                                .as_mut()
                                .filter(|job| job.container == container)
                            {
                                job.status = Some(status);
                                job.progress = progress;
                                job.error = None;
                            }
                        }
                        crate::winetricks::InstallUpdate::Completed {
                            installed,
                            succeeded,
                            failed,
                        } => {
                            self.winetricks.set_installed(&container, installed);
                            if let Some(job) = self
                                .winetricks
                                .job
                                .as_mut()
                                .filter(|job| job.container == container)
                            {
                                job.status = Some(JobStatus::Completed);
                                job.finished = true;
                                job.succeeded = succeeded;
                                job.failed = failed.clone();
                                job.error = None;
                            }
                            if !failed.is_empty() {
                                let reason = failed
                                    .first()
                                    .map(crate::winetricks::FailureDto::label)
                                    .unwrap_or_default();
                                self.show_notice(
                                    crate::i18n::s().state.notice_winetricks_failed(failed.len(), &reason),
                                    cx,
                                );
                            }
                        }
                        crate::winetricks::InstallUpdate::Failed { status, reason } => {
                            if let Some(job) = self
                                .winetricks
                                .job
                                .as_mut()
                                .filter(|job| job.container == container)
                            {
                                job.status = Some(status);
                                job.finished = true;
                                job.error = Some(reason.clone());
                            }
                            if status != JobStatus::Cancelled {
                                self.show_notice(reason, cx);
                            }
                        }
                    }
                }
            }
            BridgeEvent::ProgramSettings { settings } => {
                if self.program_target_matches(&settings.container, &settings.executable) {
                    let was_saving = self.program_settings.saving;
                    let close = self.program_settings.close_after_save;
                    let pending_switch = self.program_settings.pending_switch.take();
                    let saved_name = settings.program_name.clone();
                    self.program_settings.close_after_save = false;
                    self.program_settings.apply_settings(settings);
                    self.preview.program_inputs.clear();
                    if let Some((next_index, next_exe)) = pending_switch {
                        self.show_notice(crate::i18n::s().state.notice_program_saved(&saved_name), cx);
                        self.run_settings_target_exe = Some(next_exe);
                        self.open_program_settings(next_index, cx);
                    } else {
                        self.refresh_program_cache(cx);
                        if close {
                            self.preview.sheet = None;
                            self.preview.sheet_history.clear();
                            self.preview_sheet_return_captured_gen = 0;
                            self.show_notice(crate::i18n::s().state.program_settings_saved, cx);
                        } else if was_saving {
                            self.show_notice(crate::i18n::s().state.notice_program_saved(&saved_name), cx);
                        }
                    }
                }
            }
            BridgeEvent::ProgramSettingsFailed {
                container,
                executable,
                reason,
            } => {
                if self.program_target_matches(&container, &executable) {
                    let was_saving = self.program_settings.saving;
                    self.program_settings.pending_switch = None;
                    self.program_settings.close_after_save = false;
                    self.program_settings.fail(reason.clone());
                    if was_saving {
                        self.show_notice(reason, cx);
                    }
                }
            }
            BridgeEvent::ProgramHistory {
                container,
                executable,
                history,
            } => {
                if self.program_target_matches(&container, &executable) {
                    self.program_settings.apply_history(history);
                }
            }
            BridgeEvent::ProgramHistoryFailed {
                container,
                executable,
                reason,
            } => {
                if self.program_target_matches(&container, &executable) {
                    self.program_settings.history_loading = false;
                    self.program_settings.history_error = Some(reason);
                }
            }
            BridgeEvent::ProgramShortcutCreated {
                container,
                executable,
                path,
            } => {
                if self.program_target_matches(&container, &executable) {
                    self.program_settings.shortcut_path = Some(path);
                    self.program_settings.shortcut_error = None;
                    self.show_notice(crate::i18n::s().state.shortcut_created, cx);
                }
            }
            BridgeEvent::ProgramShortcutFailed {
                container,
                executable,
                reason,
            } => {
                if self.program_target_matches(&container, &executable) {
                    self.program_settings.shortcut_error = Some(reason.clone());
                    self.show_notice(reason, cx);
                }
            }
            BridgeEvent::ProgramRunFailed { name, reason } => {
                self.show_notice(crate::i18n::s().state.notice_launch_failed(&name, &reason), cx);
            }
            BridgeEvent::EntryMoved {
                source_container,
                target_container,
            } => {                let matches = self.move_entry.as_ref().is_some_and(|state| {
                    state.source_container == source_container
                        && state.target_container.as_deref() == Some(target_container.as_str())
                });
                if matches {
                    self.move_entry = None;
                    self.preview.sheet = None;
                    self.preview.sheet_history.clear();
                    // The move re-keyed the entry's id on the target
                    // container, so both sides need a fresh
                    // `runtime.configure` before the next Play
                    // (`retry_library` forgets the same way).
                    self.configured.forget(&source_container);
                    self.configured.forget(&target_container);
                    self.show_notice(crate::i18n::s().state.entry_moved_refreshing, cx);
                    self.refresh_library(cx);
                }
            }
            BridgeEvent::EntryMoveFailed {
                source_container,
                target_container,
                reason,
            } => {
                if let Some(state) = self.move_entry.as_mut()
                    && state.source_container == source_container
                    && state.target_container.as_deref() == Some(target_container.as_str())
                {
                    state.pending = false;
                    state.error = Some(reason.clone());
                }
                self.show_notice(reason, cx);
            }
            BridgeEvent::BottleOperation {
                operation,
                container,
                update,
            } => {
                if !self.demo {
                    match update {
                        BottleOperationUpdate::Started => {
                            self.bottle_operation = Some((operation, container));
                        }
                        BottleOperationUpdate::Completed { path, name } => {
                            if self
                                .bottle_operation
                                .as_ref()
                                .is_some_and(|(active, active_container)| {
                                    *active == operation && *active_container == container
                                })
                            {
                                self.bottle_operation = None;
                            }
                            if matches!(operation, BottleOperation::Move | BottleOperation::Duplicate)
                                && path.is_some()
                            {
                                self.select_after_refresh = path.clone();
                            }
                            let st = &crate::i18n::s().state;
                            let message = match operation {
                                BottleOperation::Rename => {
                                    crate::i18n::fmt::notice_bottle_renamed(name.as_deref())
                                }
                                BottleOperation::Remove => st.bottle_removed.to_owned(),
                                BottleOperation::Move => st.bottle_moved.to_owned(),
                                BottleOperation::Duplicate => {
                                    crate::i18n::fmt::notice_bottle_duplicated(name.as_deref())
                                }
                                BottleOperation::Export => {
                                    crate::i18n::fmt::notice_bottle_exported(path.as_deref())
                                }
                                BottleOperation::Repair => st.bottle_repaired.to_owned(),
                                BottleOperation::CleanCache => st.shader_cache_cleaned.to_owned(),
                            };
                            self.show_notice(message, cx);
                        }
                        BottleOperationUpdate::Failed { reason } => {
                            self.bottle_operation = None;
                            self.show_notice(reason, cx);
                        }
                    }
                }
            }
            BridgeEvent::PathChosen { purpose, path } => {
                if !self.demo {
                    if purpose == "discover-bottles" {
                        if let Some(root) = path { self.begin_import_scan(Some(root), cx);
                        }
                        cx.notify();
                        return;
                    }
                    if purpose == "import-gptk" {
                        if let Some(path) = path {
                            self.gptk_importing = true;
                            self.gptk_error = None;
                            self.show_notice(crate::i18n::s().state.importing_gptk, cx);
                            self.bridge.import_gptk(&path); }
                        cx.notify();
                        return;
                    }
                    let action = self.pending_path_action.take();
                    match (purpose.as_str(), action) {
                        ("run", Some(PathAction::Run { container })) => {
                            if let Some(path) = path {
                                let name = std::path::Path::new(&path)
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| path.clone());
                                self.show_notice(crate::i18n::s().state.notice_launching(&name), cx);
                                self.bridge.run_program(&container, &path, &name);
                            }
                        }
                        ("move", Some(PathAction::Move { container })) => {
                            if let Some(destination) = path {
                                self.bridge.bottle_operation(
                                    BottleOperation::Move,
                                    &container,
                                    None,
                                    Some(&destination),
                                    false,
                                );
                            }
                        }
                        ("export", Some(PathAction::Export { container })) => {
                            if let Some(destination) = path {
                                self.bridge.bottle_operation(
                                    BottleOperation::Export,
                                    &container,
                                    None,
                                    Some(&destination),
                                    false,
                                );
                            }
                        }
                        ("pin", Some(PathAction::Pin { container })) => {
                            if let Some(path) = path {
                                let name = std::path::Path::new(&path)
                                    .file_stem()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| crate::i18n::s().state.fallback_program.to_owned());
                                self.bridge
                                    .set_program_pinned(&container, &path, &name, true);
                            }
                        }
                        ("diagnostics-export", Some(PathAction::DiagnosticsExport { container })) => {
                            if let Some(destination) = path {
                                self.diagnostics_pending = true;
                                self.bridge
                                    .export_diagnostics(&container, &destination);
                            }
                        }
                        (
                            "metadata-cover",
                            Some(PathAction::MetadataCover {
                                container,
                                entry_id,
                            }),
                        ) => {
                            if let Some(path) = path {
                                if let Some(page) = self.game_info.as_mut() {
                                    page.applying = true;
                                    page.error = None;
                                }
                                self.bridge
                                    .import_metadata_cover(&container, &entry_id, &path);
                            }
                        }
                        _ => {}
                    }
                }
            }
            BridgeEvent::BottleToolOpened { container, tool } => {
                if !self.demo && self.selected_container().as_deref() == Some(container.as_str()) {
                    let b = &crate::i18n::s().bottle;
                    let label = match tool.as_str() {
                        "winecfg" => b.wine_config,
                        "regedit" => b.regedit,
                        "control" => b.control_panel,
                        "terminal" => b.terminal,
                        _ => b.wine_tool,
                    };
                    self.show_notice(crate::i18n::s().state.notice_opened_tool(label), cx);
                }
            }
            BridgeEvent::Diagnostics {
                kind,
                container,
                update,
            } => {
                if !self.demo {
                    let current = self.selected_container().as_deref() == Some(container.as_str());
                    match update {
                        DiagnosticUpdate::Started => match kind {
                            DiagnosticKind::Report | DiagnosticKind::Export => {
                                self.diagnostics_pending = true;
                                self.diagnostics_error = None;
                            }
                            DiagnosticKind::Checks => {
                                self.checks_pending = true;
                                self.diagnostics_error = None;
                            }
                            DiagnosticKind::Audio => {
                                self.audio_pending = true;
                                self.audio_error = None;
                            }
                        },
                        DiagnosticUpdate::Report { report } => {
                            self.diagnostics_pending = false;
                            self.diagnostics_error = None;
                            if current {
                                self.diagnostics_report = Some(report);
                            }
                        }
                        DiagnosticUpdate::Exported { path } => {
                            self.diagnostics_pending = false;
                            self.show_notice(crate::i18n::s().state.notice_diagnostics_exported(&path), cx);
                        }
                        DiagnosticUpdate::Checks { results } => {
                            self.checks_pending = false;
                            self.diagnostics_error = None;
                            let failures = results
                                .iter()
                                .filter(|check| check.outcome == "fail" || check.outcome == "error")
                                .count();
                            self.diagnostic_checks = Some(results);
                            if failures > 0 {
                                self.show_notice(
                                    crate::i18n::s().state.notice_checks_with_failures(failures),
                                    cx,
                                );
                            } else {
                                self.show_notice(crate::i18n::s().state.checks_clean, cx);
                            }
                        }
                        DiagnosticUpdate::Audio { result } => {
                            self.audio_pending = false;
                            self.audio_error = None;
                            let summary = result.summary.clone();
                            if current {
                                self.audio_result = Some(result);
                            }
                            self.show_notice(summary, cx);
                        }
                        DiagnosticUpdate::Failed { reason } => {
                            self.diagnostics_pending = false;
                            self.audio_pending = false;
                            self.checks_pending = false;
                            if kind == DiagnosticKind::Audio {
                                self.audio_error = Some(reason.clone());
                            } else {
                                self.diagnostics_error = Some(reason.clone());
                            }
                            self.show_notice(reason, cx);
                        }
                    }
                }
            }
            BridgeEvent::Metadata { update } => {
                if !self.demo {
                    match update {
                        MetadataUpdate::Record { container, entry_id, generation, operation, result } => {
                            self.metadata_record_result(container, entry_id, generation, operation, result, cx);
                        }
                        MetadataUpdate::SearchStarted {
                            container,
                            entry_id,
                            generation,
                            request,
                        } => {
                            if let Some(page) = self.game_info.as_mut().filter(|page| {
                                page.accepts_search(&container, &entry_id, generation, request)
                            }) {
                                page.searching = true;
                                page.error = None;
                                page.groups.clear();
                            }
                        }
                        MetadataUpdate::Results {
                            container,
                            entry_id,
                            generation,
                            request,
                            results,
                        } => {
                            let matches_source = self.game_info.as_ref().is_some_and(|page| {
                                page.accepts_search(&container, &entry_id, generation, request)
                                    && results.iter().all(|group| group.source == page.editor.search_source.as_str())
                            });
                            if matches_source {
                                if let Some(page) = self.game_info.as_mut() {
                                page.searching = false;
                                page.error = None;
                                page.groups = results
                                    .into_iter()
                                    .map(|MetadataSearchGroupDto { source, error, games }| {
                                        MetadataGroup {
                                            source,
                                            error,
                                            games: games
                                                .into_iter()
                                                .filter_map(MetadataCandidate::from_value)
                                                .collect(),
                                        }
                                    })
                                    .collect();
                                }
                            }
                        }
                        MetadataUpdate::SearchFailed {
                            container, entry_id, generation, request, reason,
                        } => {
                            if let Some(page) = self.game_info.as_mut().filter(|page| {
                                page.accepts_search(&container, &entry_id, generation, request)
                            }) {
                                page.searching = false;
                                page.error = Some(reason);
                            }
                        }
                        MetadataUpdate::Applied {
                            container,
                            entry_id,
                        } => {
                            if self.game_info.as_ref().is_some_and(|page| {
                                page.container == container && page.entry_id == entry_id
                            }) {
                                if let Some(page) = self.game_info.as_mut() {
                                    page.applying = false;
                                    page.error = None;
                                }
                                self.bridge.load_entries(&container);
                                self.show_notice(crate::i18n::s().state.metadata_updated, cx);
                            }
                        }
                        MetadataUpdate::Matches {
                            container,
                            entry_id,
                            matches,
                        } => {
                            if let Some(page) = self.game_info.as_mut().filter(|page| {
                                page.container == container && page.entry_id == entry_id
                            }) {
                                page.matches = Some(matches);
                                page.config_error = None;
                            }
                        }
                        MetadataUpdate::MatchesFailed { container, entry_id, reason } => {
                            if let Some(page) = self.game_info.as_mut().filter(|page| page.container == container && page.entry_id == entry_id) {
                                page.config_error = Some(reason);
                            }
                        }
                        MetadataUpdate::ConfigApplied {
                            container,
                            entry_id,
                            changes,
                            pending_verbs,
                        } => {
                            if let Some(page) = self.game_info.as_mut().filter(|page| {
                                page.container == container && page.entry_id == entry_id
                            }) {
                                page.applying = false;
                                let missing = (!pending_verbs.is_empty())
                                    .then(|| pending_verbs.join(", "));
                                page.config_message = Some(crate::i18n::fmt::notice_gamedb_applied(
                                    changes.len(),
                                    missing.as_deref(),
                                ));
                                self.bridge.load_entries(&container);
                                if self
                                    .config
                                    .as_ref()
                                    .is_some_and(|config| config.container == container)
                                {
                                    self.bridge.load_configuration(&container);
                                }
                                self.show_notice(crate::i18n::s().state.gamedb_applied, cx);
                            }
                        }
                        MetadataUpdate::EntrySynced { container, entry } => {
                            if !self.demo {
                                // Patch the finished card in place; the final
                                // `Synced` still re-reads the whole bottle.
                                let slot = self
                                    .bottles
                                    .iter()
                                    .position(|bottle| bottle.path == container)
                                    .map(|index| index + 1);
                                if let Some(slot) = slot {
                                    let entry_id = entry.id.clone();
                                    let mut game = entry.into_game(slot);
                                    game.container = container.clone();
                                    if let Some(games) =
                                        self.bottle_entries.get_mut(&container)
                                    {
                                        match games.iter_mut().find(|g| g.id == entry_id) {
                                            Some(existing) => *existing = game,
                                            None => games.push(game),
                                        }
                                        self.rebuild_entries();
                                    }
                                }
                            }
                        }
                        MetadataUpdate::Synced {
                            container,
                            matched,
                            failed,
                            automatic,
                        } => {
                            if !automatic {
                                self.bridge.load_entries(&container);
                                let message = crate::i18n::fmt::notice_sync_done(
                                    matched as usize,
                                    failed as usize,
                                );
                                self.show_notice(message, cx);
                            }
                            // Automatic runs already patched every matched
                            // card through `EntrySynced`; staying silent here
                            // also prevents the trigger loop that a
                            // whole-bottle re-read would cause.
                        }
                        MetadataUpdate::Failed { reason } => {
                            if let Some(page) = self.game_info.as_mut() {
                                page.applying = false;
                                page.error = Some(reason.clone());
                            }
                            self.show_notice(reason, cx);
                        }
                    }
                }
            }
            BridgeEvent::Setup { update } => {
                if !self.demo {
                    match update {
                        SetupUpdate::Status {
                            installed,
                            managed_installed,
                            any_engine_available,
                            version,
                            rosetta_installed,
                            apple_silicon,
                        } => {
                            // Older helpers report neither extra key; there
                            // `installed` IS the managed runtime's state.
                            self.setup_status = Some(SetupStatusDto {
                                installed,
                                managed_installed: Some(managed_installed.unwrap_or(installed)),
                                any_engine_available: Some(
                                    any_engine_available.unwrap_or(installed),
                                ),
                                version,
                                path: self
                                    .setup_status
                                    .as_ref()
                                    .map(|status| status.path.clone())
                                    .unwrap_or_default(),
                                rosetta_installed,
                                apple_silicon,
                            });
                            // Only force the managed-runtime install prompt
                            // when no engine at all can run a bottle: a
                            // registered external engine (`anyEngineAvailable`)
                            // is enough for external-only setups.
                            if Self::managed_install_prompt_needed(&self.setup_status)
                                && !self.setup_prompted
                                && self.service_available("setup.install")
                            {
                                self.setup_prompted = true;
                                self.setup_dialog = true;
                            }
                            self.maybe_check_updates();
                        }
                        SetupUpdate::Installing { stage, progress } => {
                            self.setup_dialog = true;
                            self.setup_installing = true;
                            self.setup_stage = stage;
                            self.setup_progress = progress;
                            self.setup_error = None;
                        }
                        SetupUpdate::Installed { version } => {
                            self.setup_installing = false;
                            self.setup_stage = None;
                            self.setup_progress = None;
                            self.setup_error = None;
                            self.setup_dialog = false;
                            let rosetta_installed = self
                                .setup_status
                                .as_ref()
                                .is_some_and(|status| status.rosetta_installed);
                            let apple_silicon = self
                                .setup_status
                                .as_ref()
                                .is_some_and(|status| status.apple_silicon);
                            self.setup_status = Some(SetupStatusDto {
                                installed: true,
                                managed_installed: Some(true),
                                any_engine_available: Some(true),
                                version: version.clone(),
                                path: String::new(),
                                rosetta_installed,
                                apple_silicon,
                            });
                            let message = crate::i18n::fmt::notice_wine_installed(version.as_deref());
                            self.show_notice(message, cx);
                            self.bridge.reload_library();
                        }
                        SetupUpdate::RosettaInstalled => {
                            self.setup_installing = false;
                            self.setup_stage = None;
                            self.setup_progress = None;
                            self.setup_error = None;
                            if let Some(status) = self.setup_status.as_mut() {
                                status.rosetta_installed = true;
                            }
                            self.show_notice(crate::i18n::s().state.rosetta_installed, cx);
                        }
                        SetupUpdate::WineUpdate { local, remote } => {
                            if !self.offline() && self.flag("updates") {
                                self.show_notice(crate::i18n::s().state.notice_wine_update_available(&local, &remote), cx);
                            }
                        }
                        SetupUpdate::Failed { reason } => {
                            self.setup_installing = false;
                            self.setup_error = Some(reason.clone());
                            self.show_notice(reason, cx);
                        }
                    }
                }
            }
            BridgeEvent::GptkStatus(status) => {
                self.gptk_status = Some(status);
                self.gptk_importing = false;
            }
            BridgeEvent::GptkImportSucceeded(status) => {
                self.gptk_status = Some(status);
                self.gptk_importing = false;
                self.gptk_error = None;
                self.show_notice(crate::i18n::s().state.gptk_import_succeeded, cx);
            }
            BridgeEvent::GptkImportFailed { reason } => {
                self.gptk_importing = false;
                self.gptk_error = Some(reason.clone());
                self.show_notice(crate::i18n::s().state.notice_import_failed(&reason), cx);
            }
            BridgeEvent::GptkRemoveSucceeded(status) => {
                self.gptk_status = Some(status);
                self.gptk_importing = false;
                self.gptk_error = None;
                self.show_notice(crate::i18n::s().state.gptk_removed, cx);
            }
            BridgeEvent::GptkRemoveFailed { reason } => {
                self.gptk_importing = false;
                self.show_notice(crate::i18n::s().state.notice_remove_failed(&reason), cx);
            }
        }
        self.loading = self.compute_loading();
        cx.notify();
    }

    /// Restore the last committed value for a rejected optimistic edit.
    fn roll_back(&mut self, mutation: EntryMutation, container: &str, entry_id: &str) {
        let key = entry_key(container, entry_id);
        match mutation {
            EntryMutation::Rating => {
                if let Some(previous) = self.rating_edits.fail(&key) {
                    self.update_entry(container, entry_id, |game| game.rating = previous);
                }
            }
            EntryMutation::Hidden => {
                if let Some(previous) = self.hidden_edits.fail(&key) {
                    self.update_entry(container, entry_id, |game| game.hidden = previous);
                }
            }
            EntryMutation::Tags => {
                if let Some(previous) = self.tag_edits.fail(&key) {
                    self.update_entry(container, entry_id, |game| game.tags = previous.clone());
                }
            }
            EntryMutation::LaunchOptions => {
                if let Some((multi, exes, last)) = self.launch_options_edits.fail(&key) {
                    self.update_entry(container, entry_id, |game| {
                        game.multi_executable = multi;
                        game.launch_executables = exes.clone();
                        game.last_executable = last.clone();
                    });
                }
            }
        }
        self.rebuild_entries();
    }

    /// Select a bottle (or `None` for the whole library). Entries are cached per
    /// bottle, so revisiting is instant; only a bottle without a cache issues a
    /// read. The runtime is configured once per session, lazily.
    pub(crate) fn select_bottle(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        if self.inline_settings_active() {
            self.settings = false;
        }
        self.selected_bottle = index;
        self.preview.page = if index.is_none() {
            PreviewPage::Library
        } else {
            PreviewPage::Bottle
        };
        self.preview.history.clear();
        self.stop_process_polling();
        self.detail_open = false;
        self.hovered = None;
        if let Some(index) = index
            && !self.demo
            && let Some(bottle) = self.bottles.get(index)
            && bottle.available
        {
            let path = bottle.path.clone();
            self.failures.remove(&index);
            if !self.bottle_entries.contains_key(&path) && !self.pending.contains(&index) {
                self.pending.insert(index);
                self.bridge.load_entries(&path);
            }
            // The runtime publishes one session at a time, so selecting a
            // bottle rebinds it before any card can offer Play.
            if self.configured.ensure(&path) {
                self.bridge.configure_runtime(&path);
            }
        }
        self.loading = self.compute_loading();
        self.invalidate_view();
        cx.notify();
    }

    /// `LibraryView.loadMetadata`: once a bottle's entries land with
    /// metadata still missing, the folder library's automatic initial match
    /// gets one pass per catalogue revision. The bridge honours the
    /// library's own switch and pending set, so this only schedules the
    /// job; `auto_synced` keeps repeated loads from spawning no-op jobs.
    fn maybe_auto_sync_metadata(&mut self, container: &str) {
        if self.demo || !self.onboarding_completed || self.onboarding.is_some()
            || self.auto_synced.contains(container) || self.offline() {
            return;
        }
        let needs_metadata = self.bottle_entries.get(container).is_some_and(|games| {
            games.iter().any(|game| {
                game.is_folder
                    && game.studio.is_none()
                    && game.description.is_none()
                    && game.release.is_none()
            })
        });
        if !needs_metadata {
            return;
        }
        let sources = self.metadata_sources();
        if sources.is_empty() {
            return;
        }
        self.auto_synced.insert(container.to_owned());
        self.bridge.sync_metadata(container, sources, true,
            self.preview.toggles.get("scrape-characters").copied().unwrap_or(true));
    }

    /// Retry the visible scope: re-read while connected, reconnect otherwise.
    pub(crate) fn retry_library(&mut self, cx: &mut Context<Self>) {
        self.load_error = None;
        self.failures.clear();
        if !self.demo {
            match self.bridge_status {
                BridgeStatus::Connected { .. } => {
                    self.pending.clear();
                    match self.selected_bottle {
                        Some(index) => {
                            if let Some(path) = self
                                .bottles
                                .get(index)
                                .filter(|bottle| bottle.available)
                                .map(|bottle| bottle.path.clone())
                            {
                                // A retry re-reads the bottle, so its runtime
                                // configuration must be refreshed as well.
                                self.bottle_entries.remove(&path);
                                self.configured.forget(&path);
                                self.pending.insert(index);
                                self.bridge.load_entries(&path);
                                if self.configured.ensure(&path) {
                                    self.bridge.configure_runtime(&path);
                                }
                            } else {
                                self.failures
                                    .insert(index, crate::i18n::s().state.bottle_currently_unavailable.to_owned());
                            }
                        }
                        None => {
                            // Keep the caches on screen while each bottle is
                            // re-read; `Entries` replaces them one at a time.
                            self.request_all();
                        }
                    }
                }
                _ => {
                    self.pending.clear();
                    self.bridge.retry();
                    self.bridge_status = BridgeStatus::Locating;
                }
            }
        }
        self.loading = self.compute_loading();
        self.invalidate_view();
        cx.notify();
    }

    fn request_all(&mut self) {
        for index in 0..self.bottles.len() {
            if !self.bottles[index].available {
                self.failures
                    .insert(index, crate::i18n::s().state.bottle_currently_unavailable.to_owned());
                continue;
            }
            let path = self.bottles[index].path.clone();
            self.pending.insert(index);
            self.bridge.load_entries(&path);
        }
    }

    fn compute_loading(&self) -> bool {
        if self.demo {
            return false;
        }
        match self.selected_bottle {
            Some(index) => self.pending.contains(&index),
            None => !self.pending.is_empty(),
        }
    }

    /// The flat grid list is rebuilt in bottle order so shelf layout is stable.
    /// Per-bottle caches keep every `(container, id)` record; only identical
    /// title+artwork cards are collapsed, and the first bottle wins.
    fn rebuild_entries(&mut self) {
        self.entries = library_view::merge_bottles(&self.bottles, &self.bottle_entries);
        self.invalidate_view();
    }

    /// Replace one entry in the per-bottle cache and in the display list.
    fn update_entry(&mut self, container: &str, entry_id: &str, mut update: impl FnMut(&mut Game)) {
        if let Some(games) = self.bottle_entries.get_mut(container) {
            for game in games.iter_mut().filter(|game| game.id == entry_id) {
                update(game);
            }
        }
        for game in self
            .entries
            .iter_mut()
            .filter(|game| game.container == container && game.id == entry_id)
        {
            update(game);
        }
    }

    /// Invalidate the memoised filter/order lists.
    fn invalidate_view(&mut self) {
        self.library_revision = self.library_revision.wrapping_add(1);
        *self.view_cache.get_mut() = ViewCache::default();
    }

    pub(crate) fn games(&self) -> &[Game] {
        if self.demo {
            demo_data()
        } else {
            &self.entries
        }
    }

    pub(crate) fn game(&self, index: usize) -> Option<&Game> {
        self.games().get(index)
    }

    /// One-based sidebar slot: `0` for the whole library.
    pub(crate) fn bottle_slot(&self) -> usize {
        self.selected_bottle.map_or(0, |index| index + 1)
    }

    pub(crate) fn bottle_name(&self) -> &str {
        self.selected_bottle
            .and_then(|index| self.bottles.get(index))
            .map(|bottle| bottle.name.as_str())
            .unwrap_or(crate::i18n::s().state.fallback_bottle)
    }

    pub(crate) fn bottle_name_at(&self, slot: usize) -> &str {
        slot.checked_sub(1)
            .and_then(|index| self.bottles.get(index))
            .map(|bottle| bottle.name.as_str())
            .unwrap_or("")
    }

    /// Entries in the visible scope before the search filter runs.
    pub(crate) fn scope_entry_count(&self) -> usize {
        match self.selected_bottle {
            Some(index) => self
                .games()
                .iter()
                .filter(|game| game.bottle == index + 1)
                .count(),
            None => self.games().len(),
        }
    }

    pub(crate) fn empty_library_message(&self) -> &'static str {
        empty_library_message(self.demo, self.bottles.len(), self.scope_entry_count())
    }

    /// The error surface is scoped: a broken bottle does not hide entries that
    /// loaded from another bottle on the Library page.
    pub(crate) fn visible_load_error(&self) -> Option<&str> {
        if self.demo {
            return None;
        }
        match self.selected_bottle {
            Some(index) => self.failures.get(&index).map(String::as_str),
            None => {
                if !self.entries.is_empty() {
                    return None;
                }
                self.load_error
                    .as_deref()
                    .or_else(|| self.failures.values().next().map(String::as_str))
            }
        }
    }
    pub(crate) fn timing(&self, ms: u64) -> Transition {
        Transition::new(Duration::from_millis(if self.motion { ms } else { 0 }))
    }
    pub(crate) fn open_detail_from(
        &mut self,
        index: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.detail_return_focus = window.focused(cx);
        self.open_detail(index, cx);
    }

    pub(crate) fn open_detail(&mut self, index: usize, cx: &mut Context<Self>) {
        self.preview.sheet = None;
        if self.detail != Some(index) {
            self.detail_scroll.set_offset(point(px(0.), px(0.)));
            self.detail_character = None;
        }
        self.detail = Some(index);
        self.detail_open = true;
        self.detail_tags_expanded = false;
        self.detail_open_generation = self.detail_open_generation.wrapping_add(1);
        self.hovered = None;
        self.notice = None;
        cx.notify();
    }

    pub(crate) fn close_detail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.detail_open = false;
        if let Some(handle) = self.detail_return_focus.take() {
            handle.focus(window, cx);
        } else {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }
    pub(crate) fn show_notice(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.notice = Some(message.into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let _ = this.update(cx, |this, cx| {
                this.notice = None;
                cx.notify();
            });
        })
        .detach();
    }

    fn view_key(&self, cx: &App) -> ViewKey {
        ViewKey {
            query: self.search.read(cx).value().trim().to_lowercase(),
            tag_filter: self.tag_filter.clone(),
            studio_filter: self.studio_filter.clone(),
            slot: self.bottle_slot(),
            show_hidden: self.show_hidden,
            revision: self.library_revision,
        }
    }

    /// Filtered indices, memoised per `(query, tag_filter, studio_filter, scope, hidden filter, revision)`.
    pub(crate) fn filtered(&self, cx: &App) -> Vec<usize> {
        let key = self.view_key(cx);
        {
            let cache = self.view_cache.borrow();
            if let Some((cached, indices)) = &cache.view
                && *cached == key
            {
                return indices.clone();
            }
        }
        let indices = library_view::filter_indices_with_filters(
            self.games(),
            &key.query,
            &key.tag_filter,
            &key.studio_filter,
            key.slot,
            key.show_hidden,
        );
        self.view_cache.borrow_mut().view = Some((key, indices.clone()));
        indices
    }

    /// Reverses the current order. Only the visual-test capture harness
    /// still triggers this; the user-facing entry points were removed.
    #[cfg(feature = "visual-test")]
    pub(crate) fn reorder_library(&mut self, cx: &mut Context<Self>) {
        self.reversed = !self.reversed;
        self.hovered = None;
        self.invalidate_view();
        cx.notify();
    }

    /// Filtered and sorted indices, memoised per
    /// `(query, scope, hidden filter, sort, reverse, revision)`.
    pub(crate) fn ordered(&self, cx: &App) -> Vec<usize> {
        let key = OrderKey {
            view: self.view_key(cx),
            sort: self.preview.sort,
            reversed: self.reversed,
        };
        {
            let cache = self.view_cache.borrow();
            if let Some((cached, indices)) = &cache.ordered
                && *cached == key
            {
                return indices.clone();
            }
        }
        let mut indices = self.filtered(cx);
        library_view::sort_indices(
            self.games(),
            &mut indices,
            Sort::from_index(self.preview.sort),
            &self.bottles,
            // `groupsLaunchers: displayMode != .timeline` (LibraryView.swift);
            // the staggered rail is a timeline too, so launchers interleave.
            ![
                preferences::mode_index("timeline"),
                Some(preferences::STAGGERED_MODE),
            ]
            .contains(&Some(self.mode)),
        );
        if self.reversed {
            indices.reverse();
        }
        self.view_cache.borrow_mut().ordered = Some((key, indices.clone()));
        indices
    }

    pub(crate) fn command_available(&self, command: Command) -> bool {
        let method = match command {
            Command::TestAudio => Some("audio.test"),
            Command::BottleRepair => Some("bottle.repair"),
            Command::CleanShaderCache => Some("bottle.cleanCache"),
            Command::Move(_) => Some("entry.move"),
            Command::OpenGameDirectory(_) => Some("system.open"),
            _ => None,
        };
        method.is_none_or(|method| self.service_available(method))
    }

    pub(crate) fn command(&mut self, command: Command, cx: &mut Context<Self>) {
        if !self.command_available(command) {
            self.show_notice(crate::i18n::s().state.feature_unavailable_platform, cx);
            return;
        }
        match command {
            Command::Page(page) => self.navigate(page, cx),
            Command::Sheet(sheet) => self.open_preview_sheet(sheet, cx),
            Command::Details(i) => self.open_detail(i, cx),
            Command::Configure(i) => {
                let slot = self.game(i).map_or(0, |game| game.bottle);
                self.selected_bottle = slot.checked_sub(1).filter(|index| *index < self.bottles.len());
                self.navigate(PreviewPage::ConfigureBottle, cx);
            }
            Command::Toggle(key) => self.toggle_option(key, cx),
            Command::CoverSize(width) => self.set_cover_width(width, cx),
            Command::Choose(key, value) => self.choose(key, value, cx),
            Command::Sort(i) => self.set_sort(i, cx),
            Command::Hide(i) => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else {
                    self.toggle_hidden(i, cx);
                }
            }
            Command::Notice => self.show_notice(demo_notice(), cx),
            Command::Runtime(i) => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else {
                    let action = self.primary_action(i).action;
                    self.perform_runtime_action(i, action, cx);
                }
            }
            Command::Executable(i) => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else {
                    self.open_executable_picker(i, false, cx);
                }
            }
            Command::LauncherAutoStart(i) => self.toggle_launcher_autostart(i, cx),
            Command::OpenGameDirectory(i) => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else if let Some(game) = self.game(i)
                    && let Some(path) = game.game_directory()
                {
                    self.bridge.open(&game.container, &path.to_string_lossy());
                } else {
                    self.show_notice(crate::i18n::s().state.game_dir_unavailable, cx);
                }
            }
            Command::Reveal(i) => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else if let Some(game) = self.game(i)
                    && let Some(path) = game.executable.as_ref()
                {
                    self.bridge
                        .reveal(&game.container, &path.to_string_lossy());
                }
            }
            Command::Shortcut => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else {
                    self.create_program_shortcut(cx);
                }
            }
            Command::Move(i) => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else {
                    self.open_preview_sheet(PreviewSheet::MoveEntry(i), cx);
                }
            }
            Command::OpenDrive => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else if let Some(bottle) = self.selected_bottle.and_then(|i| self.bottles.get(i)) {
                    let drive = std::path::Path::new(&bottle.path).join("drive_c");
                    self.bridge.open(&bottle.path, &drive.to_string_lossy());
                }
            }
            Command::RefreshLibrary => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else {
                    self.refresh_library(cx);
                }
            }
            Command::StopAllProcesses => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else {
                    self.stop_all_processes(cx);
                }
            }
            Command::BottleRename(i) => {
                self.open_bottle_dialog(BottleDialogKind::Rename, i, cx);
            }
            Command::BottleDuplicate(i) => {
                self.open_bottle_dialog(BottleDialogKind::Duplicate, i, cx);
            }
            Command::BottleRemove(i) => {
                self.open_bottle_dialog(BottleDialogKind::Remove, i, cx);
            }
            Command::BottleMove(i) => self.start_move_bottle(i, cx),
            Command::BottleExport(i) => self.start_export_bottle(i, cx),
            Command::BottleReveal(i) => self.reveal_bottle(i, cx),
            Command::BottleDialogCancel => self.dismiss_bottle_dialog(cx),
            Command::BottleDialogConfirm => self.confirm_bottle_dialog(cx),
            Command::BottleRun => self.start_bottle_run(cx),
            Command::AddProgram => self.start_pin_program(cx),
            Command::BottleTool(tool) => self.open_bottle_tool(tool, cx),
            Command::BottleRepair => self.maintain_bottle(BottleOperation::Repair, cx),
            Command::CleanShaderCache => self.maintain_bottle(BottleOperation::CleanCache, cx),
            Command::RunDiagnostics => self.load_diagnostics(cx),
            Command::ExportDiagnostics => self.export_diagnostics(cx),
            Command::TestAudio => self.test_audio(cx),
            Command::RunBottleChecks => self.run_bottle_checks(cx),
            Command::RefreshProcesses => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else {
                    self.refresh_processes(cx);
                }
            }
            Command::Translation(i) => self.open_translation_for_game(i, cx),
            Command::GameInfo(i) => self.open_game_info(i, cx),
            Command::MetadataSearch => self.search_game_info(cx),
            Command::MetadataApply(source, game) => {
                self.apply_game_info_metadata(source, game, cx);
            }
            Command::MetadataClear => self.clear_game_info_metadata(cx),
            Command::MetadataCover => self.choose_game_info_cover(cx),
            Command::MetadataCoverReset => self.reset_game_info_cover(cx),
            Command::GameDbApply(i) => self.apply_game_db_match(i, cx),
            Command::SyncMetadata => {
                if self.demo {
                    self.show_notice(demo_notice(), cx);
                } else if self.offline() {
                    self.show_notice(crate::i18n::s().state.offline_sync_blocked, cx);
                } else if let Some(container) = self.scan_container() {
                    self.show_notice(crate::i18n::s().state.scanning_and_syncing, cx);
                    let sources = self.metadata_sources();
                    if sources.is_empty() {
                        self.show_notice(crate::i18n::s().state.enable_one_metadata_source, cx);
                    } else {
                        self.bridge.sync_metadata(&container, sources, false,
                            self.preview.toggles.get("scrape-characters").copied().unwrap_or(true));
                    }
                } else {
                    self.show_notice(crate::i18n::s().state.select_bottle_sidebar_first, cx);
                }
            }
            Command::EditTags(i) => self.open_game_tags_sheet(i, cx),
            Command::ManageTags => self.open_manage_tags_sheet(cx),
            Command::FilterStudio(i) => {
                if let Some(studio) = self.game(i).and_then(|game| game.studio_name()) {
                    self.click_studio_filter(&studio, cx);
                }
            }
            Command::ReplayTagBarHint => self.replay_tag_bar_hint(cx),
            Command::InstallWineRuntime => self.install_wine_runtime(cx),
            Command::InstallRosetta => self.install_rosetta(cx),
            Command::DismissSetup => self.dismiss_setup(cx),
        }
        cx.notify();
    }

    pub(crate) fn stop_all_processes(&mut self, cx: &mut Context<Self>) {
        if let Some(bottle) = self.selected_bottle.and_then(|i| self.bottles.get(i)) {
            let container = bottle.path.clone();
            if let Some(page) = self.processes.as_mut().filter(|p| p.container == container) {
                page.table.apply(Vec::new());
            }
            self.bridge.kill_all_processes(&container);
            self.show_notice(crate::i18n::s().state.stop_all_requested, cx);
        }
    }

    /// Save the window frame, then let the helper apply the stored
    /// kill-on-terminate policy before the process exits.
    pub(crate) fn request_quit(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.request_quit_with_update(window, cx, false);
    }

    fn request_quit_with_update(&mut self, window: &Window, cx: &mut Context<Self>, for_app_update: bool) {
        if self.quitting {
            return;
        }
        if !matches!(self.bridge_status, BridgeStatus::Connected { .. }) {
            self.quit_error = Some(crate::i18n::s().state.bridge_quit_unavailable.into());
            self.bridge_dialog = true;
            cx.notify();
            return;
        }
        self.quitting = true;
        self.cancel_cover_jobs();
        self.quit_error = None;
        self.quit_generation = self.quit_generation.wrapping_add(1);
        let generation = self.quit_generation;
        let current = window.window_handle();
        let is_secondary = self
            .settings_window
            .handle()
            .is_some_and(|h| h.window_id() == current.window_id())
            || self
                .capture_window
                .handle()
                .is_some_and(|h| h.window_id() == current.window_id());
        if !is_secondary {
            crate::ui_state::save_window(window.window_bounds());
        }
        self.bridge.shutdown_application(for_app_update);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(65)).await;
            let _ = this.update(cx, |this, cx| {
                if this.quitting && this.quit_generation == generation {
                    this.quitting = false;
                    this.app_update.pending = None;
                    this.quit_error = Some(crate::i18n::s().state.quit_cleanup_timeout.into());
                    this.bridge_dialog = true;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn show_about(&mut self, cx: &mut Context<Self>) {
        Self::open_about_state(&mut self.about, &mut self.about_tab);
        self.bridge_dialog = false;
        cx.notify();
    }

    pub(crate) fn dismiss_about(&mut self) {
        Self::reset_about_state(&mut self.about, &mut self.about_tab);
    }

    pub(crate) fn open_about_state(about: &mut bool, about_tab: &mut AboutTab) {
        *about = true;
        *about_tab = AboutTab::Overview;
    }

    pub(crate) fn reset_about_state(about: &mut bool, about_tab: &mut AboutTab) {
        *about = false;
        *about_tab = AboutTab::Overview;
    }

    pub(crate) fn inline_settings_active(&self) -> bool {
        self.settings && !self.settings_window.is_active()
    }

    pub(crate) fn inline_capture_active(&self) -> bool {
        self.capture.panel_open && !self.capture_window.is_active()
    }

    pub(crate) fn can_navigate_back(&self) -> bool {
        !self.inline_settings_active()
            && !self.inline_capture_active()
            && matches!(
                self.preview.page,
                PreviewPage::ConfigureBottle
                    | PreviewPage::Programs
                    | PreviewPage::Processes
                    | PreviewPage::GameConfigs
            )
    }

    pub(crate) fn back_destination_label(&self) -> String {
        let strings = crate::i18n::s();
        match self.preview.history.last().copied().unwrap_or(PreviewPage::Bottle) {
            PreviewPage::Library => strings.library.title.into(),
            PreviewPage::Statistics => strings.library.statistics.into(),
            PreviewPage::Launchers => strings.library.launchers.into(),
            PreviewPage::Bottle => self.bottle_name().to_owned(),
            PreviewPage::ConfigureBottle => strings.bottle.configure_bottle.into(),
            PreviewPage::Programs => strings.bottle.installed_programs.into(),
            PreviewPage::Processes => strings.bottle.running_processes.into(),
            PreviewPage::GameConfigs => strings.bottle.game_configs.into(),
        }
    }

    fn clear_settings_scroll_handles(&self) {
        let mut handles = self.preview.scroll_handles.borrow_mut();
        handles.remove("app-settings-scroll");
        handles.remove("translation-settings-scroll");
        for section in SettingsSection::ALL {
            handles.remove(section.scroll_id());
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn ensure_settings_window(&mut self, cx: &mut Context<Self>) {
        match self.settings_window {
            DetachedWindow::Open(handle) => {
                let weak = cx.entity().downgrade();
                (**cx).defer(move |cx| {
                    let activated = handle
                        .update(cx, |_, window, _| {
                            window.activate_window();
                        })
                        .is_ok();
                    if !activated {
                        if let Some(app) = weak.upgrade() {
                            app.update(cx, |this, cx| {
                                this.settings_window = DetachedWindow::Opening;
                                cx.notify();
                            });
                            crate::windows::open_settings_window_if_needed(weak, cx);
                        }
                    }
                });
            }
            DetachedWindow::Opening => {}
            DetachedWindow::Closed => {
                self.settings_window = DetachedWindow::Opening;
                let weak = cx.entity().downgrade();
                (**cx).defer(move |cx| {
                    crate::windows::open_settings_window_if_needed(weak, cx);
                });
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn ensure_capture_window(&mut self, cx: &mut Context<Self>) {
        match self.capture_window {
            DetachedWindow::Open(handle) => {
                let weak = cx.entity().downgrade();
                (**cx).defer(move |cx| {
                    let activated = handle
                        .update(cx, |_, window, _| {
                            window.activate_window();
                        })
                        .is_ok();
                    if !activated {
                        if let Some(app) = weak.upgrade() {
                            app.update(cx, |this, cx| {
                                this.capture_window = DetachedWindow::Opening;
                                cx.notify();
                            });
                            crate::windows::open_capture_window_if_needed(weak, cx);
                        }
                    }
                });
            }
            DetachedWindow::Opening => {}
            DetachedWindow::Closed => {
                self.capture_window = DetachedWindow::Opening;
                let weak = cx.entity().downgrade();
                (**cx).defer(move |cx| {
                    crate::windows::open_capture_window_if_needed(weak, cx);
                });
            }
        }
    }

    /// Opens Preferences/Settings in an independent native window on macOS (or
    /// inline on Linux where X11 coordinate smoke tests target the main window).
    pub(crate) fn open_settings(&mut self, cx: &mut Context<Self>) {
        #[cfg(target_os = "linux")]
        {
            self.capture.panel_open = false;
            self.translation_page.dismiss();
            self.settings = true;
            self.detail_open = false;
            self.preview.sheet = None;
            self.preview.sheet_history.clear();
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.settings = true;
            if self.settings_section == SettingsSection::Translation
                && !self.translation_page.active
            {
                self.translation_page.open(&self.bridge);
            }
            self.ensure_settings_window(cx);
        }
        if !self.demo && self.service_available("setup.status") {
            self.bridge.load_setup_status();
            #[cfg(not(target_os = "linux"))]
            self.bridge.load_gptk_status();
        }
        if !self.demo && self.service_available("wine.engines.list") {
            self.bridge.load_wine_engines();
        }
        cx.notify();
    }

    pub(crate) fn open_settings_section(
        &mut self,
        section: SettingsSection,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if self.settings_section == SettingsSection::Translation
            && section != SettingsSection::Translation
        {
            if let Some(window) = window {
                self.translation_page.close(window, cx);
            } else {
                self.translation_page.dismiss();
                self.translation_page.clear_window_inputs();
            }
        }
        self.settings_section = section;
        if section == SettingsSection::Translation {
            self.translation_page.open(&self.bridge);
        }
        self.clear_settings_scroll_handles();
        cx.notify();
    }

    pub(crate) fn open_translation_settings(&mut self, cx: &mut Context<Self>) {
        let from_capture = self.capture.panel_open;
        #[cfg(target_os = "linux")]
        {
            self.capture.panel_open = false;
            self.open_settings(cx);
            self.translation_page.context = if from_capture {
                self.capture.context.clone()
            } else {
                None
            };
            self.translation_page.return_to_capture = from_capture;
            self.settings_section = SettingsSection::Translation;
            self.translation_page.open(&self.bridge);
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.settings_section = SettingsSection::Translation;
            self.translation_page.context = if from_capture {
                self.capture.context.clone()
            } else {
                None
            };
            self.translation_page.return_to_capture = false;
            self.open_settings(cx);
            self.translation_page.open(&self.bridge);
        }
        cx.notify();
    }

    pub(crate) fn close_settings(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.clear_settings_scroll_handles();
        let handle = self.settings_window.handle();
        self.settings_window = DetachedWindow::Closed;
        self.settings = false;
        if handle.is_none() {
            if let Some(window) = window {
                self.translation_page.close(window, cx);
            } else {
                self.translation_page.dismiss();
                self.translation_page.clear_window_inputs();
            }
        } else {
            self.translation_page.dismiss();
            self.translation_page.clear_window_inputs();
            if let Some(handle) = handle {
                (**cx).defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                });
            }
        }
        self.sync_capture_provider_selection();
        cx.notify();
    }

    pub(crate) fn handle_settings_window_closing(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_settings_scroll_handles();
        self.translation_page.close(window, cx);
        self.settings = false;
        self.settings_window = DetachedWindow::Closed;
        self.sync_capture_provider_selection();
        cx.notify();
    }

    pub(crate) fn sync_capture_provider_selection(&mut self) {
        if let Some(id) = self.capture.provider_id.as_deref() {
            if !self.translation_page.providers.iter().any(|p| p.id == id) {
                self.capture.provider_id = None;
            }
        }
        if self.capture.provider_id.is_none() && self.translation_page.providers.len() == 1 {
            self.capture.provider_id = Some(self.translation_page.providers[0].id.clone());
        }
    }

    pub(crate) fn capture_hud_available(&self) -> bool {
        if cfg!(target_os = "linux") {
            !self.demo && self.service_available("translation.capture.start")
        } else {
            self.service_available("translation.capture.hud/show")
        }
    }

    pub(crate) fn sync_capture_preset_and_provider(&mut self) {
        let providers_ready = (self.translation_page.providers_loaded
            || !self.translation_page.providers.is_empty())
            && !self.translation_page.is_pending("translation.providers/list");
        if providers_ready {
            self.capture
                .apply_pending_preset(&self.translation_page.providers);
        }
        self.sync_capture_provider_selection();
        if providers_ready {
            self.capture.maybe_auto_attach_on_preset_load(
                &self.bridge,
                self.translation_page.provider_probe_pending(),
            );
        }
    }

    pub(crate) fn close_translation_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_window.is_active() {
            self.close_settings(None, cx);
            return;
        }
        let return_to_capture = self.translation_page.return_to_capture;
        self.translation_page.close(window, cx);
        if return_to_capture {
            self.settings = false;
            self.capture.panel_open = true;
            self.sync_capture_provider_selection();
        }
        cx.notify();
    }

    pub(crate) fn open_translation_for_game(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(game) = self.game(index) {
            let context = (game.container.clone(), game.id.clone(), game.title.clone());
            self.capture.set_context(Some(context));
        }
        self.sync_capture_provider_selection();
        #[cfg(target_os = "linux")]
        {
            self.settings = false;
            self.translation_page.active = false;
            self.capture.panel_open = true;
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.capture.panel_open = true;
            self.ensure_capture_window(cx);
        }
        if self.translation_page.providers.is_empty() {
            self.translation_page.request(
                &self.bridge,
                "translation.providers/list",
                serde_json::json!({}),
            );
        }
        if self.capture.context.is_some() {
            self.capture.load_preset_for_context(&self.bridge);
        }
        if !self.demo && self.service_available("translation.overlay.read") {
            let _ = self.capture.request(
                &self.bridge,
                "translation.overlay.read",
                serde_json::json!({}),
            );
        }
        let scope_context = self
            .capture
            .context
            .as_ref()
            .map(|(container, entry_id, _)| (container.clone(), entry_id.clone()));
        let active_game_id = self
            .capture
            .snapshot
            .as_ref()
            .and_then(|s| s.game_id.clone());
        self.translation_page.cache.scope_context = scope_context.clone();
        self.translation_page.cache.scope_game_id = active_game_id.clone();
        self.translation_page.cache.only_current_game =
            active_game_id.is_some() || scope_context.is_some();
        if !self.demo && self.service_available("translation.cache/list") {
            self.translation_page
                .refresh_cache(&self.bridge, active_game_id);
        }
        if self.capture.history_owner == self.capture.context {
            self.capture.refresh_history(&self.bridge, 0);
        }
        cx.notify();
    }

    pub(crate) fn close_capture_window(&mut self, cx: &mut Context<Self>) {
        self.capture.panel_open = false;
        self.capture.clear_window_inputs();
        self.preview
            .scroll_handles
            .borrow_mut()
            .remove("capture-panel-scroll");
        let handle = self.capture_window.handle();
        self.capture_window = DetachedWindow::Closed;
        if let Some(handle) = handle {
            (**cx).defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            });
        }
        cx.notify();
    }

    pub(crate) fn handle_capture_window_closing(&mut self, cx: &mut Context<Self>) {
        self.capture.panel_open = false;
        self.capture.clear_window_inputs();
        self.capture_window = DetachedWindow::Closed;
        self.preview
            .scroll_handles
            .borrow_mut()
            .remove("capture-panel-scroll");
        cx.notify();
    }

    pub(crate) fn handle_secondary_window_closed(
        &mut self,
        closed_id: gpui_kit::WindowId,
        cx: &mut Context<Self>,
    ) {
        let mut changed = false;
        if self
            .settings_window
            .handle()
            .is_some_and(|h| h.window_id() == closed_id)
        {
            self.clear_settings_scroll_handles();
            self.settings_window = DetachedWindow::Closed;
            self.settings = false;
            self.translation_page.dismiss();
            self.translation_page.clear_window_inputs();
            self.sync_capture_provider_selection();
            changed = true;
        }
        if self
            .capture_window
            .handle()
            .is_some_and(|h| h.window_id() == closed_id)
        {
            self.capture_window = DetachedWindow::Closed;
            self.capture.panel_open = false;
            self.capture.clear_window_inputs();
            self.preview
                .scroll_handles
                .borrow_mut()
                .remove("capture-panel-scroll");
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn choose_gptk_image(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        self.bridge
            .choose_path("import-gptk", "open", None, None, &["dmg"]);
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn remove_gptk(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        self.gptk_importing = true;
        self.bridge.remove_gptk();
        cx.notify();
    }

    pub(crate) fn choose_bottle_location(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let current = self.default_bottle_location.clone().unwrap_or_default();
        self.bridge.choose_folder(&current);
    }

    /// Re-read `wine.engines.list`; called when Settings opens and after
    /// every management mutation.
    pub(crate) fn load_wine_engines(&mut self) {
        self.bridge.load_wine_engines();
    }

    /// Settings → 添加引擎: the folder reply registers the tree as an
    /// external engine. Detection and rejection happen helper-side.
    pub(crate) fn choose_wine_engine_directory(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        self.engine_folder_pending = true;
        self.bridge.choose_folder("");
    }

    /// Settings → 移除: drops the registration only; the engine's files are
    /// never deleted.
    pub(crate) fn remove_wine_engine(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        self.bridge.remove_wine_engine(id);
        cx.notify();
    }

    /// Ask to rebind a container to another engine. Changing an existing
    /// prefix goes through the warning dialog — the helper also refuses
    /// while the container is busy, and a failure keeps the old binding.
    pub(crate) fn request_engine_select(
        &mut self,
        container: &str,
        engine_id: &str,
        cx: &mut Context<Self>,
    ) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let current = self
            .engine_selections
            .get(container)
            .map(|selection| selection.engine_id.as_str());
        if current == Some(engine_id) {
            return;
        }
        let engine_name = self
            .wine_engines
            .iter()
            .find(|engine| engine.id == engine_id)
            .map(|engine| engine.name.clone())
            .unwrap_or_else(|| engine_id.to_owned());
        self.engine_dialog = Some(EngineDialogState {
            kind: EngineDialogKind::Select { engine_name },
            container: container.to_owned(),
            engine_id: engine_id.to_owned(),
        });
        cx.notify();
    }

    /// The dialog's confirm button: run the queued engine action.
    pub(crate) fn confirm_engine_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.engine_dialog.take() else {
            return;
        };
        match dialog.kind {
            EngineDialogKind::Select { .. } => {
                self.bridge.select_wine_engine(&dialog.container, &dialog.engine_id);
            }
            EngineDialogKind::Feature {
                feature,
                apply,
                fingerprint,
                ..
            } => {
                // Approval first, write second: the queued value is applied
                // only when the matching `EngineSelection` reply carries the
                // approval. A stale fingerprint, a vanished payload or a
                // session drop answers `EngineSelectionFailed` and the write
                // never happens.
                self.pending_engine_action = Some(PendingEngineAction {
                    container: dialog.container.clone(),
                    engine_id: dialog.engine_id.clone(),
                    feature: feature.clone(),
                    fingerprint: fingerprint.clone(),
                    apply,
                });
                self.bridge.set_engine_feature_override(
                    &dialog.container,
                    &feature,
                    true,
                    true,
                    &fingerprint,
                );
            }
        }
        cx.notify();
    }

    /// The `EngineSelection` reply a pending action waits for: same engine,
    /// the approval it asked for persisted, and the fingerprint the helper
    /// bound it to still being the engine's CURRENT fingerprint — an engine
    /// replaced between confirm and reply invalidates the approval.
    fn engine_action_confirmed(&self, selection: &WineEngineSelectionDto) -> bool {
        self.pending_engine_action.as_ref().is_some_and(|pending| {
            selection.engine_id == pending.engine_id
                && selection.engine.fingerprint.as_deref() == Some(pending.fingerprint.as_str())
                && selection
                    .feature_overrides
                    .get(&pending.feature)
                    .is_some_and(|approved| approved == &pending.fingerprint)
        })
    }

    /// What the fresh selection says about a queued apply: `Ready` when the
    /// requested value now passes policy, `NextApproval` when another
    /// prerequisite still wants its own confirmation, `Blocked` when a hard
    /// prerequisite is missing — the write must not happen either way but
    /// only `Blocked` drops the pending action for good.
    fn pending_apply_verdict(
        &self,
        selection: &WineEngineSelectionDto,
        apply: &PendingEngineApply,
    ) -> PendingVerdict {
        let (field, value) = match apply {
            PendingEngineApply::Config { field, value } => (field, value),
            PendingEngineApply::ProgramDraft { field, value, .. } => (field, value),
        };
        let Some(field_policy) = selection.field_policy.get(field) else {
            return PendingVerdict::Ready;
        };
        let Some((disabled, can_override, state, feature, _)) =
            Self::engine_policy_decision(field_policy, value)
        else {
            return PendingVerdict::Ready;
        };
        if !disabled {
            return PendingVerdict::Ready;
        }
        if can_override && state == "unverified" {
            if let Some(feature) = feature {
                return PendingVerdict::NextApproval(feature);
            }
        }
        PendingVerdict::Blocked
    }

    /// Applies the write a confirmed feature approval was guarding. The
    /// config-page mutation only happens when the page still shows the same
    /// container — the tagged RPC is always safe to send.
    fn apply_pending_engine_action(
        &mut self,
        pending: PendingEngineAction,
        cx: &mut Context<Self>,
    ) {
        match pending.apply {
            PendingEngineApply::Config { field, value } => {
                let current = self
                    .config
                    .as_ref()
                    .filter(|page| page.container == pending.container)
                    .and_then(|page| page.values.get(&field).cloned())
                    .unwrap_or(serde_json::Value::Null);
                self.config_writes.begin(&field, current);
                if let Some(page) = self
                    .config
                    .as_mut()
                    .filter(|page| page.container == pending.container)
                {
                    page.values.insert(field.clone(), value.clone());
                }
                self.bridge
                    .set_configuration(&pending.container, &field, value);
            }
            PendingEngineApply::ProgramDraft {
                field,
                value,
                executable,
            } => {
                // Program drafts live on the open settings page — only a page
                // still bound to the same container AND the same executable
                // gets the value, written ungated so the approval does not
                // re-open its own dialog.
                if self.program_settings.container.as_deref() == Some(&pending.container)
                    && self.program_settings.executable.as_deref() == Some(&executable)
                {
                    match value {
                        serde_json::Value::Bool(on) => {
                            self.write_program_override_bool(&field, on);
                        }
                        serde_json::Value::String(raw) => {
                            self.write_program_choice(&field, &raw);
                        }
                        _ => {}
                    }
                }
            }
        }
        cx.notify();
    }

    /// Cancel drops the pending action; nothing was sent to the helper.
    pub(crate) fn dismiss_engine_dialog(&mut self, cx: &mut Context<Self>) {
        self.engine_dialog = None;
        cx.notify();
    }

    /// Gate a pending config write against the selected engine's feature
    /// policy. Returns `true` when the write must not be sent now: either the
    /// confirmation dialog opened, or the target is unavailable outright.
    /// Disabling values (`false`, `"none"`, `"off"`) always pass.
    fn engine_gate_check(
        &mut self,
        container: &str,
        field: &str,
        next: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(selection) = self.engine_selections.get(container) else {
            return false;
        };
        let Some(field_policy) = selection.field_policy.get(field) else {
            return false;
        };
        let Some(decision) = Self::engine_policy_decision(field_policy, next) else {
            return false;
        };
        let (disabled, can_override, state, feature, reason) = decision;
        if !disabled {
            return false;
        }
        if can_override && state == "unverified" {
            if let Some(feature) = feature {
                self.engine_dialog = Some(EngineDialogState {
                    kind: EngineDialogKind::Feature {
                        feature,
                        apply: PendingEngineApply::Config {
                            field: field.to_owned(),
                            value: next.clone(),
                        },
                        reason: reason.unwrap_or_else(|| crate::i18n::s().state.engine_unverified_default.to_owned()),
                        fingerprint: selection.engine.fingerprint.clone().unwrap_or_default(),
                    },
                    container: container.to_owned(),
                    engine_id: selection.engine_id.clone(),
                });
                cx.notify();
                return true;
            }
        }
        // Unavailable or not confirmable — refuse the write entirely. The
        // control is already disabled; this guards direct calls.
        self.show_notice(
            reason.unwrap_or_else(|| crate::i18n::s().bottle.unsupported_by_engine.to_owned()),
            cx,
        );
        true
    }

    /// Whether `next` is a *disabling* write — off, none, false, empty —
    /// which an engine gate always lets through.
    fn engine_value_is_disabling(next: &serde_json::Value) -> bool {
        match next {
            serde_json::Value::Bool(v) => !*v,
            serde_json::Value::String(v) => v.is_empty() || v == "none" || v == "off",
            _ => false,
        }
    }

    /// Resolve a field policy for one candidate value: per-choice policy
    /// when the value names a choice, the field's own otherwise. `None`
    /// when the write is unrestricted (disabling writes always pass).
    fn engine_policy_decision(
        field_policy: &crate::bridge::dto::EngineFieldPolicyDto,
        next: &serde_json::Value,
    ) -> Option<(bool, bool, String, Option<String>, Option<String>)> {
        if Self::engine_value_is_disabling(next) {
            return None;
        }
        let decision = next
            .as_str()
            .and_then(|choice| field_policy.choices.get(choice))
            .map(|choice| {
                (
                    choice.disabled,
                    choice.can_override,
                    choice.state.clone(),
                    choice
                        .feature
                        .clone()
                        .or_else(|| field_policy.feature.clone()),
                    choice
                        .reason
                        .clone()
                        .or_else(|| field_policy.reason.clone()),
                )
            })
            .unwrap_or_else(|| {
                (
                    field_policy.disabled,
                    field_policy.can_override,
                    field_policy.state.clone(),
                    field_policy.feature.clone(),
                    field_policy.reason.clone(),
                )
            });
        Some(decision)
    }

    /// Same gate as `engine_gate_check` for the program-settings draft:
    /// the container is the page's own, and a confirmed write lands in the
    /// draft — never in the bottle configuration page.
    fn program_gate_check(
        &mut self,
        field: &str,
        next: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(container) = self.program_settings.container.clone() else {
            return false;
        };
        let Some(selection) = self.engine_selections.get(&container) else {
            return false;
        };
        let Some(field_policy) = selection.field_policy.get(field) else {
            return false;
        };
        let Some(decision) = Self::engine_policy_decision(field_policy, next) else {
            return false;
        };
        let (disabled, can_override, state, feature, reason) = decision;
        if !disabled {
            return false;
        }
        if can_override && state == "unverified" {
            if let Some(feature) = feature {
                self.engine_dialog = Some(EngineDialogState {
                    kind: EngineDialogKind::Feature {
                        feature,
                        apply: PendingEngineApply::ProgramDraft {
                            field: field.to_owned(),
                            value: next.clone(),
                            executable: self
                                .program_settings
                                .executable
                                .clone()
                                .unwrap_or_default(),
                        },
                        reason: reason.unwrap_or_else(|| crate::i18n::s().state.engine_unverified_default.to_owned()),
                        fingerprint: selection.engine.fingerprint.clone().unwrap_or_default(),
                    },
                    container,
                    engine_id: selection.engine_id.clone(),
                });
                cx.notify();
                return true;
            }
        }
        self.show_notice(
            reason.unwrap_or_else(|| crate::i18n::s().bottle.unsupported_by_engine.to_owned()),
            cx,
        );
        true
    }

    /// The explicit "confirm, then enable" action next to a gated control:
    /// opens the same feature dialog `engine_gate_check` would, with the
    /// value the user asked for queued behind the approval.
    pub(crate) fn request_feature_confirm(
        &mut self,
        container: &str,
        feature: &str,
        field: &str,
        value: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        let Some(selection) = self.engine_selections.get(container) else {
            return;
        };
        let reason = self
            .engine_field_policy(container, field)
            .and_then(|policy| policy.reason.clone())
            .unwrap_or_else(|| crate::i18n::s().state.engine_unverified_default.to_owned());
        self.engine_dialog = Some(EngineDialogState {
            kind: EngineDialogKind::Feature {
                feature: feature.to_owned(),
                apply: PendingEngineApply::Config {
                    field: field.to_owned(),
                    value,
                },
                reason,
                fingerprint: selection.engine.fingerprint.clone().unwrap_or_default(),
            },
            container: container.to_owned(),
            engine_id: selection.engine_id.clone(),
        });
        cx.notify();
    }

    /// The program-page counterpart of `request_feature_confirm`: same
    /// dialog, but the confirmed value lands in the open program draft —
    /// tagged with the executable it was opened for.
    pub(crate) fn request_program_feature_confirm(
        &mut self,
        container: &str,
        feature: &str,
        field: &str,
        value: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        let Some(selection) = self.engine_selections.get(container) else {
            return;
        };
        let reason = self
            .engine_field_policy(container, field)
            .and_then(|policy| policy.reason.clone())
            .unwrap_or_else(|| crate::i18n::s().state.engine_unverified_default.to_owned());
        self.engine_dialog = Some(EngineDialogState {
            kind: EngineDialogKind::Feature {
                feature: feature.to_owned(),
                apply: PendingEngineApply::ProgramDraft {
                    field: field.to_owned(),
                    value,
                    executable: self
                        .program_settings
                        .executable
                        .clone()
                        .unwrap_or_default(),
                },
                reason,
                fingerprint: selection.engine.fingerprint.clone().unwrap_or_default(),
            },
            container: container.to_owned(),
            engine_id: selection.engine_id.clone(),
        });
        cx.notify();
    }

    /// Revoke a persisted feature approval: `enabled=false` writes straight
    /// through — it never queues a pending action, and turning a gated
    /// value off separately stays possible because several settings can
    /// share one feature.
    pub(crate) fn revoke_engine_feature(&mut self, container: &str, feature: &str) {
        self.bridge
            .set_engine_feature_override(container, feature, false, false, "");
    }

    /// Field-level policy for `field` under `container`'s bound engine.
    pub(crate) fn engine_field_policy(
        &self,
        container: &str,
        field: &str,
    ) -> Option<&EngineFieldPolicyDto> {
        self.engine_selections
            .get(container)
            .and_then(|selection| selection.field_policy.get(field))
    }

    /// Policy for one `choice` inside `field`, if the bridge restricts it.
    pub(crate) fn engine_choice_policy(
        &self,
        container: &str,
        field: &str,
        choice: &str,
    ) -> Option<&crate::bridge::dto::EnginePolicyDto> {
        self.engine_field_policy(container, field)
            .and_then(|policy| policy.choices.get(choice))
    }

    /// The library's Refresh: SwiftUI reloads its bottles and re-reads every
    /// entry. Cards stay on screen while each bottle's reply replaces them.
    pub(crate) fn refresh_library(&mut self, cx: &mut Context<Self>) {
        match self.bridge_status {
            BridgeStatus::Connected { .. } => {
                self.load_error = None;
                self.bridge.reload_library();
            }
            // Without a session a refresh is a reconnect, as the retry path is.
            _ => self.retry_library(cx),
        }
    }

    /// Apply a catalogue re-read without discarding the runtime session:
    /// bottles keep their selection by path, caches of removed bottles are
    /// dropped, and every remaining bottle is re-read.
    fn apply_refreshed_containers(&mut self, containers: Vec<crate::bridge::dto::Container>) {
        let previous = self
            .selected_bottle
            .and_then(|index| self.bottles.get(index).map(|bottle| bottle.path.clone()));
        self.bottles = containers
            .into_iter()
            .map(|container| BottleVm {
                path: container.path,
                name: container.name,
                available: container.available,
            })
            .collect();
        let had_selection = previous.is_some();
        self.selected_bottle =
            previous.and_then(|path| self.bottles.iter().position(|bottle| bottle.path == path));
        if had_selection && self.selected_bottle.is_none() {
            // The selected bottle was removed or moved away: drop the bottle
            // page and stop polling a container that no longer exists.
            self.preview.page = PreviewPage::Library;
            self.preview.history.clear();
            self.detail_open = false;
            self.stop_process_polling();
        }
        let bottles = &self.bottles;
        self.bottle_entries.retain(|path, games| {
            match bottles.iter().position(|bottle| &bottle.path == path) {
                Some(index) => {
                    // Slots are positional, so a reordered catalogue renumbers.
                    for game in games.iter_mut() {
                        game.bottle = index + 1;
                    }
                    true
                }
                None => false,
            }
        });
        self.failures.clear();
        self.pending.clear();
        self.rebuild_entries();
        self.request_all();
    }

    /// Whether the setup status warrants forcing the managed-install
    /// prompt: only when NO engine at all can run a bottle. An external
    /// engine making `anyEngineAvailable` true suppresses the prompt even
    /// when the managed runtime is not installed.
    fn managed_install_prompt_needed(status: &Option<SetupStatusDto>) -> bool {
        !status
            .as_ref()
            .and_then(|status| status.any_engine_available)
            .or_else(|| status.as_ref().map(|status| status.installed))
            .unwrap_or(false)
    }

    pub(crate) fn service_available(&self, method: &str) -> bool {
        self.demo
            || (matches!(self.bridge_status, BridgeStatus::Connected { .. })
                && self.service_methods.contains(method))
    }

    pub(crate) fn toggle_available(&self, key: &str) -> bool {
        match key {
            "kill" => self.service_available("runtime.stop"),
            "refresh" => self.service_available("runtime.events"),
            "updates" => self.service_available("setup.checkUpdate"),
            "app-updates" => self.installed_app.is_some() && self.service_available("appUpdate.check"),
            _ => true,
        }
    }

    fn toggle_option(&mut self, key: &'static str, cx: &mut Context<Self>) {
        if self.quitting || !self.toggle_available(key) {
            return;
        }
        let value = match key {
            "mixed" => {
                self.mixed = !self.mixed;
                self.mixed
            }
            "motion" => {
                self.motion = !self.motion;
                self.motion
            }
            "lively" => {
                self.lively = !self.lively;
                self.lively
            }
            "show-hidden" => {
                self.show_hidden = !self.show_hidden;
                self.invalidate_view();
                self.show_hidden
            }
            "reverse-sort" => {
                self.reversed = !self.reversed;
                self.hovered = None;
                self.invalidate_view();
                self.reversed
            }
            "group-by-studio" => {
                self.toggle_group_by_studio(cx);
                return;
            }
            "tag-bar-pull-down" => {
                let next = match self.tag_bar_mode {
                    crate::ui_state::TagBarMode::PullDown => {
                        crate::ui_state::TagBarMode::AlwaysVisible
                    }
                    crate::ui_state::TagBarMode::AlwaysVisible => {
                        crate::ui_state::TagBarMode::PullDown
                    }
                };
                self.set_tag_bar_mode(next, cx);
                return;
            }
            "toggle-tag-drawer" => {
                self.set_tag_drawer_open(!self.pull_reveal.is_open(), cx);
                return;
            }
            _ => {
                let slot = self.preview.toggles.entry(key).or_default();
                *slot = !*slot;
                *slot
            }
        };
        if let Some(preference) = Self::toggle_preference(key) {
            if matches!(key, "offline" | "updates" | "app-updates") && self.prefs_supported.contains(preference) {
                self.bridge.set_preference(preference, json!(value));
                if key == "offline" && value {
                    self.app_update.message = None;
                }
                self.maybe_check_updates();
            } else {
                self.persist(preference, json!(value), cx);
            }
        }
        if METADATA_SOURCES.iter().any(|(toggle, _)| *toggle == key) {
            self.metadata_preferences.enabled_sources = METADATA_SOURCES.iter()
                .filter(|(toggle, _)| self.flag(toggle))
                .map(|(_, source)| (*source).to_owned()).collect();
            self.bridge.save_metadata_preferences(json!({
                crate::onboarding::ENABLED: self.metadata_preferences.enabled_sources.join(",")
            }), false);
        }
        cx.notify();
    }

    /// Persisted preference behind a toggle key; `None` means session-only.
    fn toggle_preference(key: &str) -> Option<&'static str> {
        match key {
            "refresh" => Some(preferences::REFRESH_AFTER_EXIT),
            "single-title" => Some(preferences::SINGLE_LINE_TITLES),
            "center-cover-text" => Some(preferences::CENTER_WIDE_COVER_TEXT),
            "blurred-fill" => Some(preferences::BLURRED_COVER_FILL),
            "backdrop-fill" => Some(preferences::BACKDROP_FILL),
            "scrape-characters" => Some(preferences::SCRAPE_CHARACTERS),
            "kill" => Some(preferences::KILL_ON_TERMINATE),
            "menubar" => Some(preferences::SHOW_MENU_BAR_EXTRA),
            "offline" => Some(preferences::OFFLINE_MODE),
            "updates" => Some(preferences::CHECK_UPDATES),
            "app-updates" => Some(preferences::CHECK_APP_UPDATES),
            "blur" => Some(preferences::DETAIL_BLURRED_BACKGROUND),
            "mixed" => Some(preferences::MIXED_COVER_LAYOUT),
            "show-hidden" => Some(preferences::SHOW_HIDDEN),
            _ => None,
        }
    }

    pub(crate) fn preview_input_specs() -> [(&'static str, &'static str); 11] {
        let strings = crate::i18n::s();
        [
            ("arguments", strings.program_settings.args_placeholder),
            ("env-key", strings.bottle.env_key_placeholder),
            ("env-value", strings.bottle.env_value_placeholder),
            ("scan-path", "~/Games"),
            ("bottle-name", ""),
            ("dll-name", strings.state.input_dll_example),
            ("build", "26100"),
            ("dpi", "96"),
            ("desktop-width", "1920"),
            ("desktop-height", "1080"),
            ("dependency-search", strings.common.searching),
        ]
    }

    pub(crate) fn set_appearance(
        &mut self,
        mode: crate::theme::AppearanceMode,
        cx: &mut Context<Self>,
    ) {
        self.appearance = mode;
        self.preview.choices.insert("appearance", mode.wire().into());
        crate::ui_state::save_appearance(mode);
        cx.set_window_appearance(mode.window_override());
        crate::theme::sync_theme(mode, None, cx);
        cx.notify();
    }

    pub(crate) fn cycle_appearance(&mut self, cx: &mut Context<Self>) {
        let next = self.appearance.next();
        self.set_appearance(next, cx);
        self.show_notice(crate::i18n::s().theme.appearance_toggled(next.label()), cx);
    }

    fn localize_native_capture_popup(&mut self) {
        if !self.demo && self.service_available("translation.capture.hud/localize") {
            let _ = self.bridge.translation_request(
                crate::translation::next_request_id(),
                "translation.capture.hud/localize",
                serde_json::json!({"strings": crate::capture::native_popup_strings()}),
            );
        }
    }

    pub(crate) fn set_language(
        &mut self,
        mode: crate::i18n::LanguageMode,
        cx: &mut Context<Self>,
    ) {
        self.language = mode;
        self.preview.choices.insert("language", mode.wire().into());
        crate::ui_state::save_language(mode);
        let _ = crate::i18n::set_language(mode);
        self.localize_native_capture_popup();
        #[cfg(not(test))]
        crate::install_menus(cx);
        self.show_notice(crate::i18n::s().settings.language_changed(mode.label()), cx);
        cx.notify();
    }

    /// Rebuilds static `InputState` instances when the resolved locale changes so
    /// placeholders match the active language without losing user input.
    pub(crate) fn sync_localized_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = crate::i18n::locale();
        if self.rendered_locale == current {
            return;
        }
        self.rendered_locale = current;
        let strings = crate::i18n::s();
        let search_value = self.search.read(cx).value().to_string();
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(strings.library.search_placeholder)
                .default_value(search_value)
        });
        let search_sub = cx.observe(&search, |_, _, cx| cx.notify());
        self.search = search;
        if !self._subscriptions.is_empty() {
            self._subscriptions[0] = search_sub;
        }

        let tag_val = self.tag_input.read(cx).value().to_string();
        let (tag_input, tag_sub) = Self::new_tag_input(tag_val, window, cx);
        self.tag_input = tag_input;
        self.tag_input_subscription = tag_sub;

        let mgr_val = self.tag_manager_input.read(cx).value().to_string();
        let (tag_manager_input, mgr_sub) = Self::new_tag_manager_input(mgr_val, window, cx);
        self.tag_manager_input = tag_manager_input;
        self.tag_manager_input_subscription = mgr_sub;

        for (key, placeholder) in Self::preview_input_specs() {
            let value = self
                .preview
                .inputs
                .get(key)
                .map(|input| input.read(cx).value().to_string())
                .unwrap_or_default();
            let entity = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .default_value(value)
            });
            if key == "dependency-search" && self._subscriptions.len() > 1 {
                self._subscriptions[1] = cx.observe(&entity, |_, _, cx| cx.notify());
            }
            self.preview.inputs.insert(key, entity);
        }
        self.preview.config_inputs.clear();
        self.preview.program_inputs.clear();
        if let Some(page) = self.game_info.as_mut()
            && let Some(existing) = page.input.take()
        {
            let value = existing.read(cx).value().to_string();
            page.input = Some(cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(strings.state.input_game_name_search)
                    .default_value(value)
            }));
        }
        if let Some(dialog) = self.bottle_dialog.as_mut()
            && let Some(existing) = dialog.input.take()
        {
            let value = existing.read(cx).value().to_string();
            dialog.input = Some(cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(strings.dialogs.bottle_name_placeholder)
                    .default_value(value)
            }));
        }
    }

    /// Select one option from a settings dropdown. Only keys the SwiftUI app
    /// persists are written through; the rest stay session-local.
    pub(crate) fn choose(&mut self, key: &'static str, value: &'static str, cx: &mut Context<Self>) {
        self.preview.choices.insert(key, value.into());
        match key {
            "appearance" => {
                self.set_appearance(crate::theme::AppearanceMode::from_wire(value), cx);
                return;
            }
            "language" => {
                self.set_language(crate::i18n::LanguageMode::from_wire(value), cx);
                return;
            }
            "terminal" => self.persist(preferences::PREFERRED_TERMINAL, json!(value), cx),
            "cover-fill" => self.persist(preferences::COVER_FILL_STYLE, json!(value), cx),
            "cover-upscale" => {
                self.cancel_cover_jobs();
                self.cover_upscale = crate::upscale::Factor::from_raw(value);
                // The pass count is part of the cache key, so every prepared
                // derivative has to be looked up again under the new factor.
                self.cover_derivatives.borrow_mut().clear();
                self.cover_jobs.borrow_mut().clear();
                self.cancel_cover_batch();
                self.persist(preferences::COVER_UPSCALE, json!(value), cx);
            }
            "cover-engine" => {
                self.cancel_cover_jobs();
                self.cover_engine = crate::upscale::Engine::from_raw(value);
                self.cover_derivatives.borrow_mut().clear();
                self.cover_jobs.borrow_mut().clear();
                self.cancel_cover_batch();
                self.persist(preferences::COVER_UPSCALE_ENGINE, json!(value), cx);
            }
            "detail-layout" => self.persist(preferences::DETAIL_LAYOUT_STYLE, json!(value), cx),
            "tag-bar-mode" => {
                self.set_tag_bar_mode(crate::ui_state::TagBarMode::from_wire(value), cx);
                return;
            }
            _ => {}
        }
        cx.notify();
    }

    /// Sidebar presentation changes go through here so the user's choice
    /// reaches `librarySidebarMode`, not just the frame's layout.
    pub(crate) fn set_sidebar(&mut self, mode: SidebarMode, cx: &mut Context<Self>) {
        self.sidebar = mode;
        self.sidebar_dirty = true;
        self.persist(preferences::SIDEBAR_MODE, json!(mode.raw()), cx);
        if mode != SidebarMode::Expanded && mode != self.sidebar_collapsed_style {
            self.sidebar_collapsed_style = mode;
            self.persist(preferences::SIDEBAR_COLLAPSED_STYLE, json!(mode.raw()), cx);
        }
        cx.notify();
    }

    /// Live value from the cover size control. `persist` debounces the write,
    /// so a drag across levels costs one call per quiet period, not per frame.
    pub(crate) fn set_cover_width(&mut self, value: f32, cx: &mut Context<Self>) {
        let chosen = crate::library_layout::chosen_cover_width(value)
            .unwrap_or(crate::library_layout::AUTOMATIC_COVER_WIDTH);
        if (self.cover_width - chosen).abs() < 0.01 {
            return;
        }
        self.cover_width = chosen;
        // Whole points, so the helper's decoder keeps it an integer and the
        // stored value does not depend on float formatting.
        self.persist(preferences::COVER_WIDTH, json!(chosen.round() as i64), cx);
        cx.notify();
    }

    /// Back to the filled shelf the control otherwise overrides.
    pub(crate) fn reset_cover_width(&mut self, cx: &mut Context<Self>) {
        self.set_cover_width(crate::library_layout::AUTOMATIC_COVER_WIDTH, cx);
    }

    /// Live value from the backdrop blur control; the debounced persist
    /// merges a drag into one write, and each level maps to a cached decode
    /// variant so the drag itself decodes nothing.
    pub(crate) fn set_backdrop_blur(&mut self, value: f64, cx: &mut Context<Self>) {
        let level = crate::artwork::blur_level(value);
        if self.backdrop_blur == level {
            return;
        }
        self.backdrop_blur = level;
        self.persist(preferences::BACKDROP_BLUR, json!(i64::from(level)), cx);
        cx.notify();
    }

    /// Back to the default blur level.
    pub(crate) fn reset_backdrop_blur(&mut self, cx: &mut Context<Self>) {
        self.set_backdrop_blur(f64::from(preferences::DEFAULT_BACKDROP_BLUR), cx);
    }

    /// The file the shelf should draw for a cover: a prepared derivative once
    /// one is ready, otherwise the original while a worker gets to it. Never
    /// blocks the frame, and never depends on how large the card is — the factor
    /// alone decides the passes, so resizing the shelf reuses the same file.
    pub(crate) fn cover_file(
        &self,
        source: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) -> std::path::PathBuf {
        if self.cover_upscale == crate::upscale::Factor::Off || !source.is_absolute() {
            return source;
        }
        let passes = self.cover_upscale.passes();
        let engine = self.cover_engine;
        let ready = {
            self.cover_derivatives
                .borrow()
                .get(&(source.clone(), passes, engine))
                .cloned()
        };
        if let Some(ready) = ready {
            return ready.unwrap_or(source);
        }
        self.prepare_cover(source.clone(), passes, engine, cx);
        source
    }

    fn cancel_cover_batch(&mut self) {
        if let Some(token) = self.cover_batch_cancel.take() {
            token.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if self.cover_batch.take().is_some() {
            self.cover_derivatives.borrow_mut().clear();
        }
    }

    pub(crate) fn artwork_image(&self, source: std::path::PathBuf) -> crate::artwork::Artwork {
        crate::artwork::Artwork::new(self.artwork_cache.clone(), source)
    }

    pub(crate) fn cover_image(
        &self,
        source: std::path::PathBuf,
        cx: &Context<Self>,
    ) -> crate::artwork::Artwork {
        let view = cx.entity().downgrade();
        self.artwork_image(source).with_resolver(move |source, cx| {
            view.update(cx, |this, cx| this.cover_file(source.to_path_buf(), cx))
                .unwrap_or_else(|_| source.to_path_buf())
        })
    }

    /// Derives a cover for every entry in the library at the widest card size,
    /// so a shelf resized to the maximum never shows a soft cover again.
    ///
    /// Running it a second time cancels the pass; whatever finished stays in the
    /// cache. One cover at a time, because the derivations share the worker lock
    /// with the on-demand path.
    pub(crate) fn start_cover_batch(&mut self, cx: &mut Context<Self>) {
        if self.quitting { return; }
        if self.cover_batch.is_some() {
            self.cancel_cover_batch();
            cx.notify();
            return;
        }
        if !crate::upscale::supported() {
            self.show_notice(crate::i18n::s().state.cover_upscale_unsupported, cx);
            return;
        }
        if !crate::upscale::ready() {
            self.show_notice(crate::i18n::s().state.cover_upscale_missing_helper, cx);
            return;
        }
        if self.cover_upscale == crate::upscale::Factor::Off {
            self.show_notice(crate::i18n::s().state.cover_upscale_pick_factor_first, cx);
            return;
        }
        let mut unique = std::collections::BTreeSet::new();
        for index in 0 .. self.games().len() {
            if let Some(path) = self.artwork(index) {
                unique.insert(path);
            }
        }
        if unique.is_empty() {
            self.show_notice(crate::i18n::s().state.cover_upscale_no_covers, cx);
            return;
        }
        let covers: Vec<_> = unique.into_iter().collect();
        self.cover_batch_generation = self.cover_batch_generation.wrapping_add(1);
        let generation = self.cover_batch_generation;
        self.cover_batch = Some(CoverBatch {
            done: 0,
            total: covers.len(),
            generation,
        });
        let factor = self.cover_upscale;
        let engine = self.cover_engine;
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.cover_batch_cancel = Some(cancel.clone());
        let worker = cx.spawn(async move |this, cx| {
            let mut succeeded = 0;
            for (position, cover) in covers.iter().enumerate() {
                let running = this
                    .update(cx, |this, _| {
                        this.cover_batch.map(|batch| batch.generation) == Some(generation)
                    })
                    .unwrap_or(false);
                if !running {
                    return;
                }
                let job = cover.clone();
                let flag = cancel.clone();
                let produced = cx.background_spawn(async move {
                    let passes = factor.passes();
                    if passes == 0 {
                        return false;
                    }
                    crate::upscale::upscaled_cover_with_cancel(&job, passes, engine, &flag)
                        .or_else(|| {
                            crate::upscale::engines()
                                .contains(&crate::upscale::Engine::Anime4K)
                                .then(|| {
                                    crate::upscale::upscaled_cover_with_cancel(
                                        &job,
                                        passes,
                                        crate::upscale::Engine::Anime4K,
                                        &flag,
                                    )
                                })?
                        })
                        .is_some()
                })
                .await;
                succeeded += usize::from(produced);
                this.update(cx, |this, cx| {
                    if !this.cover_batch.is_some_and(|batch| batch.generation == generation) {
                        return;
                    }
                    if let Some(batch) = this.cover_batch.as_mut() {
                        batch.done = position + 1;
                    }
                    cx.notify();
                })
                .ok();
            }
            this.update(cx, |this, cx| {
                if !this.cover_batch.is_some_and(|batch| batch.generation == generation) {
                    return;
                }
                let total = this.cover_batch.map_or(0, |batch| batch.total);
                this.cover_batch = None;
                this.cover_batch_cancel = None;
                // Prepared files are already on disk; looking them up again is
                // what makes the shelf switch over without a restart.
                this.cover_derivatives.borrow_mut().clear();
                let failed = total.saturating_sub(succeeded);
                this.show_notice(crate::i18n::s().settings.cover_batch_done(failed, succeeded), cx);
            })
            .ok();
        });
        worker.detach();
        cx.notify();
    }

    fn cancel_cover_jobs(&mut self) {
        self.cancel_cover_batch();
        self.cover_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        self.cover_cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.cover_jobs.borrow_mut().clear();
        self.cover_batch = None;
    }

    /// Derives one cover on a worker thread; platform-specific fallbacks live in the backend.
    fn prepare_cover(
        &self,
        source: std::path::PathBuf,
        passes: u8,
        engine: crate::upscale::Engine,
        cx: &mut Context<Self>,
    ) {
        if self.quitting || !crate::upscale::supported() || !crate::upscale::ready() {
            return;
        }
        let key = (source.clone(), passes, engine);
        if !self.cover_jobs.borrow_mut().insert(key.clone()) {
            return;
        }
        let cancel = self.cover_cancel.clone();
        let worker = cx.spawn(async move |this, cx| {
            let job = source.clone();
            let job_cancel = cancel.clone();
            let derived = cx
                .background_spawn(async move {
                    crate::upscale::upscaled_cover_with_cancel(&job, passes, engine, &job_cancel).or_else(|| {
                        (engine == crate::upscale::Engine::RealCugan
                            && crate::upscale::engines()
                                .contains(&crate::upscale::Engine::Anime4K))
                        .then(|| {
                            crate::upscale::upscaled_cover_with_cancel(
                                &job,
                                passes,
                                crate::upscale::Engine::Anime4K,
                                &job_cancel,
                            )
                        })?
                    })
                })
                .await;
            this.update(cx, |this, cx| {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                this.cover_jobs.borrow_mut().remove(&key);
                this.cover_derivatives.borrow_mut().insert(key, derived);
                cx.notify();
            })
            .ok();
        });
        worker.detach();
    }

    pub(crate) fn set_mode(&mut self, index: usize, cx: &mut Context<Self>) {
        let index = preferences::visible_mode_index(preferences::mode_value(index)).unwrap_or(0);
        self.mode = index;
        self.invalidate_view();
        self.hovered = None;
        self.immersive_hero_hovered = false;
        if index == 3 {
            self.immersive_slide = 0;
            self.ensure_immersive_carousel(cx);
        }
        self.persist(
            preferences::DISPLAY_MODE,
            json!(preferences::mode_value(index)),
            cx,
        );
        cx.notify();
    }

    pub(crate) fn select_gallery_game(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(game) = self.game(index) {
            self.gallery_selection = Some((game.container.clone(), game.id.clone()));
            self.gallery_detail_scroll.set_offset(point(px(0.), px(0.)));
            self.hovered = None;
            cx.notify();
        }
    }

    /// The staggered timeline keeps exactly one focused card, remembered by
    /// `(container, id)` so a re-sort cannot strand the selection. Clicking
    /// the same card again is a no-op — no redundant animation.
    pub(crate) fn select_staggered(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(game) = self.game(index) {
            let identity = (game.container.clone(), game.id.clone());
            if self.staggered_selection.as_ref() != Some(&identity) {
                self.staggered_selection = Some(identity);
                cx.notify();
            }
        }
    }

    pub(crate) fn clear_staggered(&mut self, cx: &mut Context<Self>) {
        if self.staggered_selection.take().is_some() {
            cx.notify();
        }
    }

    /// Resolves the stored selection to a card of the current view. When the
    /// entry was filtered out or its game disappeared this returns `None`,
    /// which simply renders as "nothing selected" — the identity is kept so
    /// the focus returns if the card re-enters the view.
    pub(crate) fn staggered_selected_index(&self, indices: &[usize]) -> Option<usize> {
        let (container, id) = self.staggered_selection.as_ref()?;
        indices.iter().copied().find(|&index| {
            self.game(index).is_some_and(|game| {
                game.container.as_str() == container.as_str() && game.id.as_str() == id.as_str()
            })
        })
    }

    /// Manual hero navigation: remembers the slide and restarts the
    /// auto-advance countdown by retiring the running loop (the epoch check
    /// below makes it exit on its next tick).
    pub(crate) fn set_immersive_slide(&mut self, slide: usize, cx: &mut Context<Self>) {
        self.immersive_slide = slide;
        self.immersive_carousel_epoch
            .set(self.immersive_carousel_epoch.get().wrapping_add(1));
        self.immersive_carousel_running.set(false);
        self.ensure_immersive_carousel(cx);
        cx.notify();
    }

    /// The single auto-advance loop for the hero carousel, started by
    /// `set_mode` and lazily by the first immersive render (a persisted
    /// mode=3 never passes `set_mode`). The loop exits on its next tick once
    /// another loop owns the epoch or the mode leaves immersive, so at most
    /// one loop ever runs.
    pub(crate) fn ensure_immersive_carousel(&self, cx: &mut Context<Self>) {
        if self.mode != 3 || self.immersive_carousel_running.replace(true) {
            return;
        }
        let epoch = self.immersive_carousel_epoch.get();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(CAROUSEL_SECS))
                    .await;
                let keep_running = this
                    .update(cx, |this, cx| {
                        if this.immersive_carousel_epoch.get() != epoch {
                            // A newer loop took over; it owns the flag.
                            return false;
                        }
                        if this.mode != 3 {
                            this.immersive_carousel_running.set(false);
                            return false;
                        }
                        let idle = this.preview.page == PreviewPage::Library
                            && !this.settings
                            && !this.detail_open
                            && this.preview.sheet.is_none()
                            && !this.immersive_hero_hovered;
                        if idle {
                            this.immersive_slide = this.immersive_slide.wrapping_add(1);
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }

    fn set_sort(&mut self, index: usize, cx: &mut Context<Self>) {
        self.preview.sort = index;
        self.persist(
            preferences::SORT,
            json!(preferences::sort_value(index)),
            cx,
        );
        self.invalidate_view();
        cx.notify();
    }

    /// Queue one preference write, coalescing bursts into a single call.
    fn persist(&mut self, key: &'static str, value: serde_json::Value, cx: &mut Context<Self>) {
        if !self.prefs_supported.contains(key) {
            // Not in `preferences.schema`: this key lives in memory only.
            return;
        }
        self.prefs_pending.insert(key.to_owned(), value);
        self.prefs_generation = self.prefs_generation.wrapping_add(1);
        let generation = self.prefs_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PREFERENCE_DEBOUNCE).await;
            let _ = this.update(cx, |this, _| {
                if this.prefs_generation != generation {
                    return;
                }
                for (key, value) in std::mem::take(&mut this.prefs_pending) {
                    this.bridge.set_preference(&key, value);
                }
            });
        })
        .detach();
    }

    /// Apply the stored values. Keys absent from the reply keep the SwiftUI
    /// defaults already in the state; keys absent from the schema are never
    /// written, so they also keep their default for the session.
    fn apply_preferences(&mut self, values: &serde_json::Value) {
        self.onboarding_completed = values.get(crate::onboarding::COMPLETED)
            .and_then(serde_json::Value::as_bool).unwrap_or(false);
        self.metadata_preferences = crate::onboarding::MetadataPreferences::read(values);
        for (toggle, source) in METADATA_SOURCES {
            self.preview.toggles.insert(toggle,
                self.metadata_preferences.enabled_sources.iter().any(|id| id == source));
        }
        if let Some(raw) = values
            .get(preferences::DISPLAY_MODE)
            .and_then(serde_json::Value::as_str)
            .and_then(preferences::visible_mode_index)
        {
            self.mode = raw;
        }
        if let Some(index) = values
            .get(preferences::SORT)
            .and_then(serde_json::Value::as_str)
            .and_then(preferences::sort_index)
        {
            self.preview.sort = index;
        }
        if let Some(flag) = values
            .get(preferences::SHOW_HIDDEN)
            .and_then(serde_json::Value::as_bool)
        {
            self.show_hidden = flag;
        }
        if let Some(flag) = values
            .get(preferences::MIXED_COVER_LAYOUT)
            .and_then(serde_json::Value::as_bool)
        {
            self.mixed = flag;
        }
        if let Some(number) = values
            .get(preferences::COVER_WIDTH)
            .and_then(serde_json::Value::as_f64)
        {
            self.cover_width = crate::library_layout::chosen_cover_width(number as f32)
                .unwrap_or(crate::library_layout::AUTOMATIC_COVER_WIDTH);
        }
        if let Some(number) = values
            .get(preferences::BACKDROP_BLUR)
            .and_then(serde_json::Value::as_f64)
        {
            self.backdrop_blur = crate::artwork::blur_level(number);
        }
        if let Some(raw) = values
            .get(preferences::COVER_UPSCALE)
            .and_then(serde_json::Value::as_str)
        {
            self.cover_upscale = crate::upscale::Factor::from_raw(raw);
            self.preview
                .choices
                .insert("cover-upscale", self.cover_upscale.raw().into());
        }
        if let Some(raw) = values
            .get(preferences::COVER_UPSCALE_ENGINE)
            .and_then(serde_json::Value::as_str)
        {
            self.cover_engine = crate::upscale::Engine::from_raw(raw);
            self.preview
                .choices
                .insert("cover-engine", self.cover_engine.raw().into());
        }
        for (preference, toggle) in [
            (preferences::SINGLE_LINE_TITLES, "single-title"),
            (preferences::CENTER_WIDE_COVER_TEXT, "center-cover-text"),
            (preferences::BLURRED_COVER_FILL, "blurred-fill"),
            (preferences::BACKDROP_FILL, "backdrop-fill"),
            (preferences::SCRAPE_CHARACTERS, "scrape-characters"),
            (preferences::REFRESH_AFTER_EXIT, "refresh"),
            (preferences::DETAIL_BLURRED_BACKGROUND, "blur"),
            (preferences::KILL_ON_TERMINATE, "kill"),
            (preferences::SHOW_MENU_BAR_EXTRA, "menubar"),
            (preferences::OFFLINE_MODE, "offline"),
            (preferences::CHECK_UPDATES, "updates"),
            (preferences::CHECK_APP_UPDATES, "app-updates"),
        ] {
            if let Some(flag) = values
                .get(preference)
                .and_then(serde_json::Value::as_bool)
            {
                self.preview.toggles.insert(toggle, flag);
            }
        }
        let string_choices: [(&str, &str, fn(&str) -> Option<usize>); 3] = [
            (
                preferences::PREFERRED_TERMINAL,
                "terminal",
                preferences::terminal_index,
            ),
            (
                preferences::COVER_FILL_STYLE,
                "cover-fill",
                preferences::cover_fill_index,
            ),
            (
                preferences::DETAIL_LAYOUT_STYLE,
                "detail-layout",
                preferences::detail_layout_index,
            ),
        ];
        for (preference, choice, valid) in string_choices {
            if let Some(raw) = values
                .get(preference)
                .and_then(serde_json::Value::as_str)
                .filter(|raw| valid(raw).is_some())
            {
                self.preview.choices.insert(choice, raw.into());
            }
        }
        if let Some(location) = values
            .get(preferences::DEFAULT_BOTTLE_LOCATION)
            .and_then(serde_json::Value::as_str)
        {
            self.default_bottle_location = Some(location.to_owned());
        }
        if !self.sidebar_dirty
            && let Some(mode) = values
                .get(preferences::SIDEBAR_MODE)
                .and_then(serde_json::Value::as_str)
                .and_then(SidebarMode::from_raw)
        {
            self.sidebar = mode;
        }
        if !self.sidebar_dirty
            && let Some(style) = values
                .get(preferences::SIDEBAR_COLLAPSED_STYLE)
                .and_then(serde_json::Value::as_str)
                .and_then(SidebarMode::from_raw)
            && style != SidebarMode::Expanded
        {
            self.sidebar_collapsed_style = style;
        }
        self.invalidate_view();
    }

    // MARK: - Bottle surfaces (configuration, environment, folders, scan)

    /// Default bottle location, matching the SwiftUI default.
    fn default_bottle_location() -> Option<String> {
        let home = std::env::var_os("HOME")?.to_string_lossy().into_owned();
        #[cfg(target_os = "macos")]
        {
            Some(format!("{}/Library/Containers/com.daydalek.Maltoria/Bottles", home))
        }
        #[cfg(not(target_os = "macos"))]
        {
            Some(format!("{}/.local/share/whisky/Bottles", home))
        }
    }

    /// The selected bottle when it is usable; the bottle page only exists for
    /// one of those.
    pub(crate) fn selected_container(&self) -> Option<String> {
        self.selected_bottle
            .and_then(|index| self.bottles.get(index))
            .filter(|bottle| bottle.available)
            .map(|bottle| bottle.path.clone())
    }

    /// Open the rename/duplicate/remove modal for one sidebar row.
    pub(crate) fn open_bottle_dialog(
        &mut self,
        kind: BottleDialogKind,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(bottle) = self.bottles.get(index) else {
            return;
        };
        let container = bottle.path.clone();
        let name = bottle.name.clone();
        let suggested = if kind == BottleDialogKind::Duplicate {
            next_duplicate_name(&name, &self.bottles)
        } else {
            name.clone()
        };
        self.bottle_dialog = Some(BottleDialogState {
            kind,
            container,
            name,
            suggested,
            input: None,
        });
        cx.notify();
    }

    /// The dialog's text field, created on first render because `InputState`
    /// needs a window. A new dialog always starts without one.
    pub(crate) fn bottle_dialog_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.bottle_dialog.as_ref().and_then(|dialog| dialog.input.clone()) {
            return input;
        }
        let initial = self
            .bottle_dialog
            .as_ref()
            .map(|dialog| dialog.suggested.clone())
            .unwrap_or_default();
        let entity = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::i18n::s().dialogs.bottle_name_placeholder)
                .default_value(initial)
        });
        if let Some(dialog) = self.bottle_dialog.as_mut() {
            dialog.input = Some(entity.clone());
        }
        entity
    }

    pub(crate) fn dismiss_bottle_dialog(&mut self, cx: &mut Context<Self>) {
        self.bottle_dialog = None;
        cx.notify();
    }

    pub(crate) fn confirm_bottle_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.bottle_dialog.take() else {
            return;
        };
        if dialog.kind == BottleDialogKind::Remove {
            self.bottle_dialog = Some(dialog);
            return;
        }
        let name = dialog
            .input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_else(|| dialog.suggested.clone());
        let name = name.trim().to_owned();
        if name.is_empty() {
            self.bottle_dialog = Some(dialog);
            self.show_notice(crate::i18n::s().state.bottle_name_empty, cx);
            return;
        }
        let operation = match dialog.kind {
            BottleDialogKind::Rename => BottleOperation::Rename,
            BottleDialogKind::Duplicate => BottleOperation::Duplicate,
            BottleDialogKind::Remove => unreachable!(),
        };
        self.bridge.bottle_operation(
            operation,
            &dialog.container,
            Some(&name),
            None,
            false,
        );
        cx.notify();
    }

    /// Remove the dialog's bottle; `delete_files` distinguishes "remove from
    /// the list" from "delete the prefix on disk".
    pub(crate) fn remove_bottle(&mut self, delete_files: bool, cx: &mut Context<Self>) {
        let Some(dialog) = self.bottle_dialog.take() else {
            return;
        };
        self.bridge.bottle_operation(
            BottleOperation::Remove,
            &dialog.container,
            None,
            None,
            delete_files,
        );
        cx.notify();
    }

    pub(crate) fn start_move_bottle(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(bottle) = self.bottles.get(index) else {
            return;
        };
        let container = bottle.path.clone();
        let parent = std::path::Path::new(&container)
            .parent()
            .map(|path| path.to_string_lossy().into_owned());
        self.pending_path_action = Some(PathAction::Move {
            container: container.clone(),
        });
        self.bridge
            .choose_path("move", "folder", parent.as_deref(), None, &[]);
        cx.notify();
    }

    pub(crate) fn start_export_bottle(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(bottle) = self.bottles.get(index) else {
            return;
        };
        let container = bottle.path.clone();
        let parent = std::path::Path::new(&container)
            .parent()
            .map(|path| path.to_string_lossy().into_owned());
        let filename = format!("{}.tar", bottle.name);
        self.pending_path_action = Some(PathAction::Export {
            container: container.clone(),
        });
        self.bridge.choose_path(
            "export",
            "save",
            parent.as_deref(),
            Some(&filename),
            &["tar"],
        );
        cx.notify();
    }

    pub(crate) fn reveal_bottle(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.bottles.get(index).map(|bottle| bottle.path.clone()) else {
            return;
        };
        self.bridge.reveal(&container, &container);
        cx.notify();
    }

    /// The bottle action bar's Run button: a native file picker whose reply
    /// starts the chosen Windows file in the selected bottle.
    pub(crate) fn start_bottle_run(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        let start = format!("{container}/drive_c");
        self.pending_path_action = Some(PathAction::Run {
            container: container.clone(),
        });
        self.bridge.choose_path(
            "run",
            "file",
            Some(&start),
            None,
            &["exe", "msi", "bat", "msix", "appx", "url"],
        );
        cx.notify();
    }

    /// Pin an arbitrary Windows file the installed-programs scan did not find.
    pub(crate) fn start_pin_program(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        let start = format!("{container}/drive_c");
        self.pending_path_action = Some(PathAction::Pin {
            container: container.clone(),
        });
        self.bridge.choose_path(
            "pin",
            "file",
            Some(&start),
            None,
            &["exe", "msi", "bat", "msix", "appx", "url"],
        );
        cx.notify();
    }

    pub(crate) fn open_bottle_tool(&mut self, tool: &'static str, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        self.bridge.open_bottle_tool(&container, tool);
        cx.notify();
    }

    /// Collect the stability report shown in the Diagnostics sheet.
    pub(crate) fn load_diagnostics(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        self.diagnostics_pending = true;
        self.diagnostics_error = None;
        self.diagnostics_report = None;
        self.bridge.load_diagnostics(&container);
        cx.notify();
    }

    /// Choose a file, then have the helper write the stability report there.
    pub(crate) fn export_diagnostics(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        let name = format!(
            "whisky-diagnostics-{}.txt",
            self.bottle_name().replace(['/', ':'], "-")
        );
        self.pending_path_action = Some(PathAction::DiagnosticsExport {
            container: container.clone(),
        });
        #[cfg(target_os = "linux")]
        let directory = std::path::Path::new(&container)
            .parent()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| container.clone());
        #[cfg(not(target_os = "linux"))]
        let directory = container.clone();
        self.bridge.choose_path(
            "diagnostics-export",
            "save",
            Some(&directory),
            Some(&name),
            &["txt"],
        );
        cx.notify();
    }

    /// Run the curated bottleneck checks (crash log, launcher, process, audio,
    /// GameDB availability) for the selected bottle.
    pub(crate) fn run_bottle_checks(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        self.checks_pending = true;
        self.diagnostics_error = None;
        self.diagnostic_checks = None;
        self.bridge.run_bottle_checks(&container);
        cx.notify();
    }

    /// Play the Wine audio probe tone in the selected bottle.
    pub(crate) fn test_audio(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        self.audio_pending = true;
        self.audio_error = None;
        self.audio_result = None;
        self.bridge.test_audio(&container, true);
        cx.notify();
    }

    /// The shared `offlineMode` default (`NetworkSettings.isOffline`). The
    /// bridge reads it too, but the original app warns before the network
    /// operation starts instead of surfacing a raw failure afterwards.
    pub(crate) fn offline(&self) -> bool {
        self.preview
            .toggles
            .get("offline")
            .copied()
            .unwrap_or(preferences::DEFAULT_OFFLINE_MODE)
    }

    fn maybe_check_updates(&mut self) {
        if !self.update_preferences_loaded || self.offline() || self.demo {
            return;
        }
        if self.setup_status.as_ref().is_some_and(|status| status.installed)
            && self.service_available("setup.checkUpdate")
            && !self.wine_update_checked
            && self.flag("updates")
        {
            self.wine_update_checked = true;
            self.bridge.check_wine_update();
        }
        if !self.app_update.checked && self.flag("app-updates") {
            self.check_app_update(true);
        }
    }

    pub(crate) fn can_check_app_update(&self) -> bool {
        self.update_preferences_loaded && !self.offline() && !self.app_update.busy
            && self.service_available("appUpdate.check") && self.installed_app.is_some()
    }

    pub(crate) fn check_app_update(&mut self, automatic: bool) {
        if !self.can_check_app_update() {
            return;
        }
        let Some(installed) = &self.installed_app else { return };
        self.app_update.checked = true;
        self.app_update.busy = true;
        self.app_update.version = None;
        self.app_update.downloaded = None;
        self.app_update.message = None;
        self.bridge.app_update(crate::app_update::Action::Check, json!({
            "currentVersion": installed.version, "format": installed.format.wire(), "automatic": automatic,
        }));
    }

    pub(crate) fn download_app_update(&mut self) {
        if self.offline() || self.app_update.busy || !self.service_available("appUpdate.download") {
            return;
        }
        if let Some(version) = &self.app_update.version {
            self.app_update.busy = true;
            self.app_update.message = None;
            self.bridge.app_update(crate::app_update::Action::Download, json!({ "version": version }));
        }
    }

    fn app_update_reply(&mut self, action: crate::app_update::Action, result: Result<serde_json::Value, String>) {
        use crate::app_update::Action;
        let strings = &crate::i18n::s().settings;
        self.app_update.busy = false;
        if self.offline() {
            return;
        }
        match (action, result) {
            (Action::Check, Ok(reply)) => {
                self.app_update.version = reply.get("availableVersion")
                    .and_then(serde_json::Value::as_str).map(str::to_owned);
                if self.app_update.version.is_none() && reply.get("skipped") != Some(&json!(true)) {
                    self.app_update.message = Some(strings.app_update_current.to_owned());
                }
            }
            (Action::Download, Ok(reply)) => {
                self.app_update.downloaded = serde_json::from_value(reply).ok();
                if self.app_update.downloaded.is_none() {
                    self.app_update.message = Some(strings.app_update_failed.to_owned());
                }
            }
            (_, Err(reason)) => {
                eprintln!("maltoria: app update failed: {reason}");
                self.app_update.message = Some(strings.app_update_failed.to_owned());
            }
        }
    }

    pub(crate) fn install_app_update(&mut self, window: &Window, cx: &mut Context<Self>) {
        let strings = &crate::i18n::s().settings;
        if self.offline() || self.quitting || self.app_update.busy {
            return;
        }
        if self.app_update.reset_if_package_missing() {
            cx.notify();
            return;
        }
        if self.setup_installing || self.runtime.any_active() {
            self.app_update.message = Some(strings.app_update_wine_busy.to_owned());
            cx.notify();
            return;
        }
        let (Some(installed), Some(downloaded)) = (self.installed_app.clone(), self.app_update.downloaded.clone()) else {
            return;
        };
        self.app_update.busy = true;
        self.app_update.message = None;
        let window = window.window_handle();
        let prepare = cx.background_executor().spawn(async move { installed.prepare(&downloaded) });
        cx.spawn(async move |this, cx| {
            let result = prepare.await;
            let _ = this.update(cx, |this, cx| {
                this.app_update.busy = false;
                cx.notify();
            });
            let _ = window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.offline() || this.quitting || !this.service_available("bridge.shutdown") {
                        return;
                    }
                    if this.setup_installing || this.runtime.any_active() {
                        this.app_update.message = Some(strings.app_update_wine_busy.to_owned());
                        cx.notify();
                        return;
                    }
                    match result.and_then(crate::app_update::PreparedReplacement::arm) {
                        Ok(pending) => {
                            this.app_update.pending = Some(pending);
                            this.request_quit_with_update(window, cx, true);
                        }
                        Err(error) => {
                            eprintln!("maltoria: could not stage app update: {error}");
                            if !this.app_update.reset_if_package_missing() {
                                this.app_update.message = Some(strings.app_update_failed.to_owned());
                            }
                            cx.notify();
                        }
                    }
                });
            });
        }).detach();
    }

    pub(crate) fn install_wine_runtime(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        if !self.service_available("setup.install") {
            self.show_notice(crate::i18n::s().state.runtime_install_unavailable, cx);
            return;
        }
        if self.offline() {
            self.show_notice(crate::i18n::s().state.offline_wine_download_blocked, cx);
            return;
        }
        self.setup_dialog = true;
        self.setup_installing = true;
        self.setup_stage = Some("checking".to_owned());
        self.setup_progress = None;
        self.setup_error = None;
        self.bridge.install_setup();
        cx.notify();
    }

    pub(crate) fn install_rosetta(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        if self.offline() {
            self.show_notice(crate::i18n::s().state.offline_rosetta_blocked, cx);
            return;
        }
        self.setup_dialog = true;
        self.setup_installing = true;
        self.setup_stage = Some("rosetta".to_owned());
        self.setup_error = None;
        self.bridge.install_rosetta();
        cx.notify();
    }

    pub(crate) fn dismiss_setup(&mut self, cx: &mut Context<Self>) {
        if self.setup_installing {
            return;
        }
        self.setup_dialog = false;
        cx.notify();
    }

    /// Open the metadata/cover/GameDB sheet for one library card.
    pub(crate) fn open_game_info(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(game) = self.game(index) else {
            return;
        };
        let container = game.container.clone();
        let entry_id = game.id.clone();
        let title = game.title.clone();
        if container.is_empty() {
            return;
        }
        self.game_info = Some(GameInfoPage {
            editor: crate::metadata_editor::MetadataEditor::default(),
            container: container.clone(),
            entry_id: entry_id.clone(),
            title,
            input: None,
            searching: false,
            searching_sources: Vec::new(),
            sources_expanded: false,
            applying: false,
            error: None,
            groups: Vec::new(),
            matches: None,
            config_message: None,
            config_error: None,
            selected_character: None,
        });
        self.bridge.match_game_config(&container, &entry_id);
        self.load_metadata_editor(cx);
        self.open_preview_sheet(PreviewSheet::GameInfo(index), cx);
        let needs_characters = self.entries.iter()
            .find(|game| game.container == container && game.id == entry_id)
            .and_then(|game| game.metadata.as_ref())
            .is_some_and(|metadata| metadata.characters.is_none()
                && matches!(metadata.extra.get("source").and_then(|value| value.as_str()), Some("vndb" | "bangumi")));
        if needs_characters && self.preview.toggles.get("scrape-characters").copied().unwrap_or(true) && !self.offline() {
            self.refresh_game_characters(cx);
        }
    }

    pub(crate) fn game_info_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.game_info.as_ref().and_then(|page| page.input.clone()) {
            return input;
        }
        let title = self
            .game_info
            .as_ref()
            .map(|page| page.title.clone())
            .unwrap_or_default();
        let entity = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::i18n::s().state.input_game_name_search)
                .default_value(title)
        });
        if let Some(page) = self.game_info.as_mut() {
            page.input = Some(entity.clone());
        }
        entity
    }

    pub(crate) fn search_game_info(&mut self, cx: &mut Context<Self>) {
        if self.game_info.as_ref().is_none_or(|page| page.searching) {
            return;
        }
        if !self.service_available("metadata.search") || self.metadata_sources().is_empty() {
            self.show_notice(crate::i18n::s().state.search_unavailable_config, cx);
            return;
        }
        if !self.game_info.as_ref().is_some_and(|page| self.metadata_sources().contains(&page.editor.search_source)) {
            self.show_notice(crate::i18n::s().state.search_platform_not_allowed, cx);
            return;
        }
        if self.offline() {
            self.show_notice(crate::i18n::s().state.offline_search_blocked, cx);
            return;
        }
        let Some(page) = self.game_info.as_mut() else {
            return;
        };
        if !page.editor.loaded || page.editor.busy {
            return;
        }
        let query = page
            .input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_else(|| page.title.clone());
        let query = query.trim().to_owned();
        if query.is_empty() {
            self.show_notice(crate::i18n::s().state.search_enter_game_name, cx);
            return;
        }
        page.editor.search_request += 1;
        page.searching = true;
        page.error = None;
        page.groups.clear();
        let (container, entry_id) = (page.container.clone(), page.entry_id.clone());
        let sources = vec![page.editor.search_source.clone()];
        self.bridge.search_metadata(
            &container,
            &entry_id,
            page.editor.generation,
            page.editor.search_request,
            &query,
            sources,
        );
        cx.notify();
    }

    pub(crate) fn apply_game_info_metadata(
        &mut self,
        source: usize,
        game: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.game_info.as_mut() else {
            return;
        };
        let Some(candidate) = page
            .groups
            .get(source)
            .and_then(|group| group.games.get(game))
        else {
            return;
        };
        if !page.editor.loaded || page.editor.busy { return; }
        let raw = candidate.raw.clone();
        let provider = page.groups[source].source.clone();
        page.error = page.editor.bind(&provider, raw).err();
        if page.error.is_none() {
            page.editor.message = Some(crate::i18n::s().state.staged_to_draft_save_to_apply.into());
        }
        cx.notify();
    }

    pub(crate) fn clear_game_info_metadata(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.game_info.as_mut() else {
            return;
        };
        page.applying = true;
        page.error = None;
        let (container, entry_id) = (page.container.clone(), page.entry_id.clone());
        self.bridge.clear_metadata(&container, &entry_id);
        cx.notify();
    }

    pub(crate) fn refresh_game_characters(&mut self, cx: &mut Context<Self>) {
        if self.offline() {
            self.show_notice(crate::i18n::s().state.offline_characters_blocked, cx);
            return;
        }
        let Some(page) = self.game_info.as_ref().filter(|page| !page.applying) else { return };
        let Some(metadata) = self.entries.iter()
            .find(|game| game.container == page.container && game.id == page.entry_id)
            .and_then(|game| game.metadata.as_ref()) else { return };
        let Ok(mut raw) = serde_json::to_value(metadata) else { return };
        raw["refreshCharacters"] = serde_json::Value::Bool(true);
        self.bridge.apply_metadata(&page.container, &page.entry_id, raw);
        if let Some(page) = self.game_info.as_mut() {
            page.applying = true;
            page.error = None;
        }
        cx.notify();
    }

    pub(crate) fn choose_game_info_cover(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.game_info.as_mut() else {
            return;
        };
        let (container, entry_id) = (page.container.clone(), page.entry_id.clone());
        page.applying = true;
        page.error = None;
        self.pending_path_action = Some(PathAction::MetadataCover {
            container: container.clone(),
            entry_id,
        });
        self.bridge.choose_path(
            "metadata-cover",
            "file",
            Some(&container),
            None,
            &["png", "jpg", "jpeg", "heic", "webp"],
        );
        cx.notify();
    }

    pub(crate) fn reset_game_info_cover(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.game_info.as_mut() else {
            return;
        };
        page.applying = true;
        page.error = None;
        let (container, entry_id) = (page.container.clone(), page.entry_id.clone());
        self.bridge.reset_metadata_cover(&container, &entry_id);
        cx.notify();
    }

    pub(crate) fn retry_game_db(&mut self, cx: &mut Context<Self>) {
        if let Some(page) = self.game_info.as_mut() {
            page.matches = None;
            page.config_error = None;
            self.bridge.match_game_config(&page.container, &page.entry_id);
            cx.notify();
        }
    }

    pub(crate) fn apply_game_db_match(&mut self, index: usize, cx: &mut Context<Self>) {
        let available = self.service_available("gameConfig.apply");
        if let Some(matched) = self.game_info.as_ref().and_then(|page| page.matches.as_ref()).and_then(|rows| rows.get(index)) {
            if !matched.apply_available(available) {
                let reason = if matched.availability_reason.is_empty() { crate::i18n::s().state.gamedb_session_reference.into() }
                             else { matched.availability_reason.clone() };
                self.show_notice(reason, cx);
                return;
            }
        }
        let Some(page) = self.game_info.as_mut() else {
            return;
        };
        let Some(matched) = page.matches.as_ref().and_then(|matches| matches.get(index)) else {
            return;
        };
        let game_id = matched
            .entry
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if game_id.is_empty() {
            self.show_notice(crate::i18n::s().state.gamedb_entry_invalid, cx);
            return;
        }
        let variant_id = matched.variant_id.clone();
        page.applying = true;
        page.error = None;
        page.config_message = None;
        let (container, entry_id) = (page.container.clone(), page.entry_id.clone());
        self.bridge
            .apply_game_config(&container, &entry_id, &game_id, &variant_id);
        cx.notify();
    }

    pub(crate) fn maintain_bottle(
        &mut self,
        operation: BottleOperation,
        cx: &mut Context<Self>,
    ) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        self.bridge
            .bottle_operation(operation, &container, None, None, false);
        cx.notify();
    }

    /// The install job only belongs to the sheet while its bottle is selected.
    pub(crate) fn winetricks_job(&self) -> Option<&InstallJob> {
        let container = self.selected_container()?;
        self.winetricks
            .job
            .as_ref()
            .filter(|job| job.container == container)
    }

    /// The sheet's bottle: the selection, or the first available bottle.
    pub(crate) fn scan_container(&self) -> Option<String> {
        self.selected_container().or_else(|| {
            self.bottles
                .iter()
                .find(|bottle| bottle.available)
                .map(|bottle| bottle.path.clone())
        })
    }

    /// Read the schema and values once per bottle, or again after a failure.
    pub(crate) fn ensure_configuration(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        if let Some(page) = &self.config
            && page.container == container
            && (page.loading || page.error.is_none())
        {
            return;
        }
        if self
            .config
            .as_ref()
            .is_none_or(|page| page.container != container)
        {
            // Input entities are per bottle: the same field id can hold a very
            // different value in another bottle.
            self.preview.config_inputs.clear();
            self.preview.config_resets.clear();
            self.config_writes.clear();
            self.config_text_pending.clear();
            self.config_text_generation = self.config_text_generation.wrapping_add(1);
            // A write queued behind a feature approval belonged to the other
            // bottle's page; navigating away drops it rather than letting a
            // late reply fire a write the user no longer sees.
            self.pending_engine_action = None;
        }
        self.config = Some(ConfigPage {
            container: container.clone(),
            capabilities: Vec::new(),
            fields: Vec::new(),
            values: BTreeMap::new(),
            loading: true,
            error: None,
        });
        self.bridge.load_configuration(&container);
        if self.service_available("wine.engines.selection") {
            self.bridge.load_engine_selection(&container);
            if self.wine_engines.is_empty() {
                self.bridge.load_wine_engines();
            }
        }
        if !self.environments.contains_key(&container) {
            self.bridge.load_environment(&container);
        }
        cx.notify();
    }

    /// Read the folder payload for the sheet's bottle.
    pub(crate) fn ensure_folders(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let Some(container) = self.scan_container() else {
            return;
        };
        if self.folders.contains_key(&container) || self.folders_pending.contains(&container) {
            return;
        }
        self.folders_pending.insert(container.clone());
        self.bridge.load_folders(&container);
        cx.notify();
    }

    /// Read the auto-start set for one bottle; fired once per container.
    fn ensure_launchers(&mut self, container: &str) {
        if self.demo
            || self.launcher_prefs.contains_key(container)
            || self.launchers_pending.contains(container)
        {
            return;
        }
        self.launchers_pending.insert(container.to_owned());
        self.bridge.load_launchers(container);
    }

    /// Every launcher entry across the bottles, in display order. The
    /// 启动器 page renders these; they never enter a game grid.
    pub(crate) fn launchers(&self) -> Vec<usize> {
        (0..self.games().len())
            .filter(|&index| self.games()[index].is_launcher)
            .collect()
    }

    /// Whether one launcher auto-starts when Whisky opens.
    pub(crate) fn launcher_auto_start(&self, index: usize) -> bool {
        self.game(index).is_some_and(|game| {
            self.launcher_prefs
                .get(&game.container)
                .is_some_and(|set| set.contains(&game.identity))
        })
    }

    /// Flip one launcher's "open with Whisky" flag; the bridge reply
    /// re-synchronises the whole set.
    pub(crate) fn toggle_launcher_autostart(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(game) = self.game(index) else { return };
        if self.demo || game.container.is_empty() {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let (container, entry_id, identity, enabled) = (
            game.container.clone(),
            game.id.clone(),
            game.identity.clone(),
            !self.launcher_auto_start(index),
        );
        let set = self.launcher_prefs.entry(container.clone()).or_default();
        if enabled {
            set.insert(identity);
        } else {
            set.remove(&identity);
        }
        self.bridge
            .set_launcher_autostart(&container, &entry_id, enabled);
        cx.notify();
    }

    /// Fire each flagged launcher once per app run, after the bottle's entries
    /// and its auto-start set have both landed.
    fn maybe_autostart_launchers(&mut self, container: &str, cx: &mut Context<Self>) {
        let Some(auto_start) = self.launcher_prefs.get(container) else {
            return;
        };
        let Some(games) = self.bottle_entries.get(container) else {
            return;
        };
        let launches: Vec<String> = games
            .iter()
            .filter(|game| {
                game.is_launcher && !game.needs_executable && auto_start.contains(&game.identity)
            })
            .map(|game| game.id.clone())
            .collect::<Vec<_>>()
            .into_iter()
            .filter(|entry_id| {
                self.launcher_autostarted.insert(entry_key(container, entry_id))
            })
            .collect();
        if launches.is_empty() {
            return;
        }
        // One configure for the whole batch; ordered commands keep it ahead
        // of the launches on the worker.
        if self.configured.ensure(container) {
            self.bridge.configure_runtime(container);
        }
        for entry_id in launches {
            self.bridge.launch(container, &entry_id, None);
        }
        cx.notify();
    }

    /// Retry a failed `folders.read`.
    pub(crate) fn reload_folders(&mut self, cx: &mut Context<Self>) {
        let Some(container) = self.scan_container() else {
            return;
        };
        self.folders_error.remove(&container);
        self.folders_pending.insert(container.clone());
        self.bridge.load_folders(&container);
        cx.notify();
    }

    /// Load the static catalog once and the selected bottle's installed set.
    pub(crate) fn ensure_winetricks(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        if self.winetricks.catalog.is_none()
            && !self.winetricks.catalog_pending
            && self.winetricks.catalog_error.is_none()
        {
            self.winetricks.catalog_pending = true;
            self.bridge.load_winetricks_catalog();
        }
        if !self.winetricks.installed.contains_key(&container)
            && !self.winetricks.installed_pending.contains(&container)
            && !self.winetricks.installed_error.contains_key(&container)
        {
            self.winetricks.installed_pending.insert(container.clone());
            self.bridge.load_installed_winetricks(&container);
        }
        cx.notify();
    }

    /// Retry `winetricks.catalog` after a failure.
    pub(crate) fn reload_winetricks_catalog(&mut self, cx: &mut Context<Self>) {
        self.winetricks.catalog_error = None;
        self.winetricks.catalog_pending = true;
        self.bridge.load_winetricks_catalog();
        cx.notify();
    }

    /// Retry the visible bottle's `winetricks.installed`.
    pub(crate) fn reload_installed_winetricks(&mut self, cx: &mut Context<Self>) {
        let Some(container) = self.selected_container() else {
            return;
        };
        self.winetricks.installed_error.remove(&container);
        self.winetricks.installed_pending.insert(container.clone());
        self.bridge.load_installed_winetricks(&container);
        cx.notify();
    }

    /// Install the selected verb. The helper refuses concurrent jobs; an
    /// optimistic queued row also stops the button from sending twice.
    pub(crate) fn start_winetricks_install(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        if self.winetricks.job.as_ref().is_some_and(InstallJob::running) {
            return;
        }
        if self.offline() {
            self.show_notice(crate::i18n::s().state.offline_winetricks_blocked, cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            self.show_notice(crate::i18n::s().state.select_bottle_first, cx);
            return;
        };
        let Some(verb) = self.winetricks.selected.clone() else {
            self.show_notice(crate::i18n::s().state.select_component_first, cx);
            return;
        };
        let mut job = InstallJob::new(&container);
        job.status = Some(JobStatus::Queued);
        self.winetricks.job = Some(job);
        self.bridge.install_winetricks(&container, &[verb]);
        cx.notify();
    }

    pub(crate) fn cancel_winetricks_install(&mut self, cx: &mut Context<Self>) {
        if self.winetricks.job.as_ref().is_some_and(InstallJob::running) {
            self.bridge.cancel_winetricks();
        }
        cx.notify();
    }

    /// Capture the entry the move picker will act on.
    pub(crate) fn prepare_move_entry(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let Some(game) = self.game(index).cloned() else {
            return;
        };
        if game.id.starts_with("steam:") || game.executable.is_none() {
            self.move_entry = None;
            self.preview.sheet = None;
            self.preview.sheet_history.clear();
            self.show_notice(crate::i18n::s().state.steam_cannot_move, cx);
            return;
        }
        self.move_entry = Some(MoveEntryState {
            title: game.title,
            source_container: game.container,
            entry_id: game.id,
            pending: false,
            target_container: None,
            error: None,
        });
        cx.notify();
    }

    pub(crate) fn start_move_entry(&mut self, target: String, cx: &mut Context<Self>) {
        let Some(state) = self.move_entry.as_mut() else {
            return;
        };
        if state.pending {
            return;
        }
        state.pending = true;
        state.error = None;
        state.target_container = Some(target.clone());
        let source = state.source_container.clone();
        let entry_id = state.entry_id.clone();
        self.bridge.move_entry(&source, &entry_id, &target);
        cx.notify();
    }

    /// A file handed over by Finder (`open`) or dragged onto the window.
    /// One bottle runs it directly; several open the bottle picker, as the
    /// SwiftUI Run panel does.
    pub(crate) fn open_external_file(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        self.receive_external_file(crate::desktop_open::ExternalOpen { path, container: None, container_id: None }, cx);
    }

    pub(crate) fn receive_external_file(&mut self, file: crate::desktop_open::ExternalOpen, cx: &mut Context<Self>) {
        if !self.demo && !self.catalogue_ready {
            if self.external_opens.len() + self.desktop_resolutions.len() < 64 { self.external_opens.push_back(file); }
            else { self.show_notice(crate::i18n::s().state.pending_files_limit, cx); }
            return;
        }
        if let Some(id) = &file.container_id {
            if self.external_opens.len() + self.desktop_resolutions.len() >= 64 { self.show_notice(crate::i18n::s().state.pending_shortcuts_limit, cx); return; }
            self.desktop_resolution_generation = self.desktop_resolution_generation.wrapping_add(1);
            let request = self.desktop_resolution_generation;
            self.bridge.resolve_desktop_open(request, id, &file.path.to_string_lossy());
            self.desktop_resolutions.insert(request, file);
            return;
        }
        let path = file.path;
        let runnable = ["exe", "msi", "bat", "msix", "appx", "url"];
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase)
            .unwrap_or_default();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        if !runnable.contains(&extension.as_str()) || !path.is_file() {
            self.show_notice(crate::i18n::s().state.notice_cannot_run_file(&name), cx);
            return;
        }
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        if self.bottles.is_empty() {
            self.show_notice(crate::i18n::s().state.create_bottle_first, cx);
            return;
        }
        if let Some(container) = file.container {
            let selected = std::fs::canonicalize(container).ok().and_then(|container| {
                self.bottles.iter().position(|bottle| std::fs::canonicalize(&bottle.path).ok().as_ref() == Some(&container))
            });
            if let Some(index) = selected {
                self.select_bottle(Some(index), cx);
                self.run_file_in(index, &path.to_string_lossy(), &name, cx);
            } else {
                self.show_notice(crate::i18n::s().state.shortcut_bottle_missing, cx);
            }
            return;
        }
        if self.bottles.len() == 1 {
            self.run_file_in(0, &path.to_string_lossy(), &name, cx);
            return;
        }
        self.open_file = Some(OpenFileState {
            path: path.to_string_lossy().into_owned(),
            name,
        });
        self.open_preview_sheet(PreviewSheet::OpenFile, cx);
    }

    pub(crate) fn start_open_file(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(file) = self.open_file.take() else {
            return;
        };
        self.run_file_in(index, &file.path, &file.name, cx);
        self.preview.sheet = None;
        self.preview.sheet_history.clear();
        self.preview_sheet_return_captured_gen = 0;
        cx.notify();
    }

    fn run_file_in(&mut self, index: usize, path: &str, name: &str, cx: &mut Context<Self>) {
        let Some(bottle) = self.bottles.get(index) else {
            return;
        };
        if !bottle.available {
            self.show_notice(crate::i18n::s().state.selected_bottle_unavailable, cx);
            return;
        }
        let container = bottle.path.clone();
        self.show_notice(crate::i18n::s().state.notice_launching(&name), cx);
        self.bridge.run_program(&container, path, name);
    }

    fn program_target_matches(&self, container: &str, executable: &str) -> bool {
        self.program_settings.container.as_deref() == Some(container)
            && self.program_settings.executable.as_deref() == Some(executable)
    }

    /// Open the real run-settings draft for the game at `index`.
    pub(crate) fn open_program_settings(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        self.preview.program_inputs.clear();
        let Some(game) = self.game(index).cloned() else {
            return;
        };
        if game.is_steam() || game.container.is_empty() {
            self.run_settings_target_exe = None;
            self.program_settings = ProgramSettingsPage::default();
            self.program_settings.title = game.title;
            self.program_settings.loading = false;
            self.program_settings.error =
                Some(crate::i18n::s().state.no_exe_cannot_edit_program_settings.to_owned());
            cx.notify();
            return;
        }
        let valid_exes: Vec<String> = {
            let mut list: Vec<String> = game
                .launch_executables
                .iter()
                .filter(|path| {
                    std::path::Path::new(path)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                })
                .cloned()
                .collect();
            if let Some(primary) = game
                .executable
                .as_ref()
                .filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                })
                .map(|path| path.to_string_lossy().into_owned())
            {
                if list.is_empty() || !game.multi_executable {
                    if !list.iter().any(|p| p == &primary) {
                        list.insert(0, primary);
                    }
                }
            }
            list
        };
        let preferred = self
            .run_settings_target_exe
            .as_ref()
            .filter(|target| valid_exes.iter().any(|p| p == *target))
            .cloned()
            .or_else(|| {
                game.last_executable
                    .as_ref()
                    .filter(|target| valid_exes.iter().any(|p| p == *target))
                    .cloned()
            })
            .or_else(|| valid_exes.first().cloned());
        self.run_settings_target_exe = preferred.clone();
        let Some(executable) = preferred else {
            self.program_settings = ProgramSettingsPage::default();
            self.program_settings.container = Some(game.container);
            self.program_settings.title = game.title;
            self.program_settings.loading = false;
            self.program_settings.history_loading = false;
            self.program_settings.error = None;
            cx.notify();
            return;
        };
        self.program_settings
            .start(&game.container, &executable, &game.title);
        self.bridge
            .load_program_settings(&game.container, &executable);
        self.bridge.load_program_history(&game.container, &executable);
        // The override rows gate on the container's engine policy — load it
        // even when the bottle configuration page was never opened.
        if self.service_available("wine.engines.selection")
            && !self.engine_selections.contains_key(&game.container)
        {
            self.bridge.load_engine_selection(&game.container);
        }
        cx.notify();
    }

    pub(crate) fn select_run_settings_executable(
        &mut self,
        index: usize,
        executable: String,
        cx: &mut Context<Self>,
    ) {
        if self.program_settings.executable.as_deref() == Some(executable.as_str())
            || self.program_settings.saving
        {
            return;
        }
        if !self.demo && self.program_settings_is_dirty(cx) {
            self.start_save_program_settings(false, Some((index, executable)), cx);
            return;
        }
        self.run_settings_target_exe = Some(executable);
        self.open_program_settings(index, cx);
    }

    /// Retry both reads after the sheet failed to open.
    pub(crate) fn retry_program_settings(&mut self, cx: &mut Context<Self>) {
        let (Some(container), Some(executable)) = (
            self.program_settings.container.clone(),
            self.program_settings.executable.clone(),
        ) else {
            return;
        };
        self.program_settings.loading = true;
        self.program_settings.saving = false;
        self.program_settings.error = None;
        self.program_settings.history_loading = true;
        self.program_settings.history_error = None;
        self.preview.program_inputs.clear();
        self.bridge.load_program_settings(&container, &executable);
        self.bridge.load_program_history(&container, &executable);
        cx.notify();
    }

    /// Measure the GDI upscaler disk cache for the program whose settings are
    /// open. The recursive walk runs on a background thread; a generation tag
    /// keeps results from a previous target or measurement out of the sheet.
    pub(crate) fn refresh_program_cache(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let dirs = self.program_settings.gdi_cache_dirs();
        if dirs.is_empty() {
            self.program_settings.cache_bytes = None;
            self.program_settings.cache_pending = false;
            return;
        }
        self.program_settings.cache_generation += 1;
        let generation = self.program_settings.cache_generation;
        self.program_settings.cache_pending = true;
        cx.spawn(async move |this, cx| {
            let bytes = cx
                .background_spawn(async move {
                    dirs.iter().map(|dir| crate::program_settings::dir_size(dir)).fold(0u64, u64::saturating_add)
                })
                .await;
            this.update(cx, |this, cx| {
                if this.program_settings.cache_generation == generation {
                    this.program_settings.cache_bytes = Some(bytes);
                    this.program_settings.cache_pending = false;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Empty the program's cache directory (the directory itself stays so a
    /// running game keeps its handle), then re-measure what remains.
    pub(crate) fn clear_program_cache(&mut self, cx: &mut Context<Self>) {
        let dirs = self.program_settings.gdi_cache_dirs();
        if dirs.is_empty() {
            return;
        }
        if self.program_settings.cache_clearing {
            return;
        }
        self.program_settings.cache_generation += 1;
        let generation = self.program_settings.cache_generation;
        self.program_settings.cache_clearing = true;
        self.program_settings.cache_pending = true;
        cx.spawn(async move |this, cx| {
            let (cleared, bytes) = cx
                .background_spawn(async move {
                    let cleared = dirs.iter().try_for_each(|dir| crate::program_settings::clear_dir(dir));
                    let bytes = dirs.iter().map(|dir| crate::program_settings::dir_size(dir)).fold(0u64, u64::saturating_add);
                    (cleared, bytes)
                })
                .await;
            this.update(cx, |this, cx| {
                if this.program_settings.cache_generation == generation {
                    this.program_settings.cache_clearing = false;
                    this.program_settings.cache_pending = false;
                    this.program_settings.cache_bytes = Some(bytes);
                    match cleared {
                        Ok(()) => this.show_notice(crate::i18n::s().state.disk_cache_cleared, cx),
                        Err(error) => {
                            this.show_notice(
                                crate::i18n::s().state.notice_clear_cache_failed(&error.to_string()),
                                cx,
                            )
                        }
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Open the program's cache directory in Finder, creating it when the game
    /// has not written any cache yet.
    pub(crate) fn open_program_cache_dir(&mut self) {
        let Some(dir) = self.program_settings.gdi_cache_dir() else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        let container = self.program_settings.container.clone().unwrap_or_default();
        self.bridge.open(&container, &dir.to_string_lossy());
    }

    pub(crate) fn open_legacy_program_cache_dir(&mut self) {
        if let Some(dir) = self.program_settings.legacy_gdi_cache_dir().filter(|dir| dir.is_dir()) {
            let container = self.program_settings.container.clone().unwrap_or_default();
            self.bridge.open(&container, &dir.to_string_lossy());
        }
    }

    fn build_program_settings_draft(
        &self,
        cx: &App,
    ) -> Result<
        (
            String,
            String,
            crate::program_settings::ProgramSettingsDto,
            String,
            BTreeMap<String, String>,
        ),
        String,
    > {
        let (Some(container), Some(executable), Some(mut settings)) = (
            self.program_settings.container.clone(),
            self.program_settings.executable.clone(),
            self.program_settings.settings.clone(),
        ) else {
            return Err(String::new());
        };
        if settings.display_enabled()
            && settings
                .overrides
                .as_ref()
                .and_then(|overrides| overrides.resolution_preset.as_deref())
                == Some("custom")
        {
            let width = self
                .preview
                .program_inputs
                .get("ps-res-width")
                .map(|input| input.read(cx).value().to_string())
                .unwrap_or_else(|| {
                    settings
                        .defaults
                        .custom_resolution_width
                        .unwrap_or(1920)
                        .to_string()
                });
            let height = self
                .preview
                .program_inputs
                .get("ps-res-height")
                .map(|input| input.read(cx).value().to_string())
                .unwrap_or_else(|| {
                    settings
                        .defaults
                        .custom_resolution_height
                        .unwrap_or(1080)
                        .to_string()
                });
            let (Ok(width), Ok(height)) = (width.trim().parse::<i64>(), height.trim().parse::<i64>())
            else {
                return Err(crate::i18n::s().state.custom_res_integer.to_owned());
            };
            if !(200..=16_384).contains(&width) || !(200..=16_384).contains(&height) {
                return Err(crate::i18n::s().state.custom_res_range.to_owned());
            }
            if let Some(overrides) = settings.overrides.as_mut() {
                overrides.custom_resolution_width = Some(width);
                overrides.custom_resolution_height = Some(height);
            }
        }
        let arguments = self
            .preview
            .program_inputs
            .get("ps-args")
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_else(|| settings.arguments.clone());
        let mut environment = BTreeMap::new();
        for row in self.program_settings.env_rows.clone() {
            if row.key == "WINE_GDI_UPSCALER_PICK" {
                environment.insert(row.key, row.value);
                continue;
            }
            let key = self
                .preview
                .program_inputs
                .get(&format!("ps-env-key-{}", row.id))
                .map(|input| input.read(cx).value().to_string())
                .unwrap_or(row.key);
            let value = self
                .preview
                .program_inputs
                .get(&format!("ps-env-value-{}", row.id))
                .map(|input| input.read(cx).value().to_string())
                .unwrap_or(row.value);
            let key = key.trim().to_owned();
            if key.is_empty() {
                continue;
            }
            if key.contains('=') || key.contains('\0') {
                return Err(crate::i18n::s().state.env_name_invalid_chars.to_owned());
            }
            if environment.insert(key.clone(), value).is_some() {
                return Err(crate::i18n::s().state.duplicate_env_key(&key));
            }
        }
        Ok((container, executable, settings, arguments, environment))
    }

    pub(crate) fn program_settings_is_dirty(&self, cx: &App) -> bool {
        let Some(baseline) = self.program_settings.baseline.as_ref() else {
            return false;
        };
        if self.program_settings.settings.is_none() {
            return false;
        }
        match self.build_program_settings_draft(cx) {
            Ok((_, _, settings, arguments, environment)) => {
                let current = crate::program_settings::normalized_patch_for_dirty_check(
                    &settings,
                    &arguments,
                    &environment,
                );
                let base = crate::program_settings::normalized_patch_for_dirty_check(
                    baseline,
                    &baseline.arguments,
                    &baseline.environment,
                );
                current != base
            }
            Err(msg) => !msg.is_empty(),
        }
    }

    fn start_save_program_settings(
        &mut self,
        close_after_save: bool,
        pending_switch: Option<(usize, String)>,
        cx: &mut Context<Self>,
    ) {
        let (container, executable, settings, arguments, environment) =
            match self.build_program_settings_draft(cx) {
                Ok(parts) => parts,
                Err(msg) => {
                    if !msg.is_empty() {
                        self.show_notice(msg, cx);
                    }
                    return;
                }
            };
        let patch = settings_patch(&settings, &arguments, &environment);
        self.program_settings.saving = true;
        self.program_settings.close_after_save = close_after_save;
        self.program_settings.pending_switch = pending_switch;
        self.program_settings.error = None;
        self.bridge
            .save_program_settings(&container, &executable, patch);
        cx.notify();
    }

    /// Read the text inputs, build one patch and save on the current target.
    pub(crate) fn save_program_settings(&mut self, cx: &mut Context<Self>) {
        self.start_save_program_settings(true, None, cx);
    }

    /// Save edits for the current `.exe` target without closing the sheet.
    pub(crate) fn save_program_settings_keep_open(&mut self, cx: &mut Context<Self>) {
        self.start_save_program_settings(false, None, cx);
    }

    pub(crate) fn create_program_shortcut(&mut self, cx: &mut Context<Self>) {
        let (Some(container), Some(executable)) = (
            self.program_settings.container.clone(),
            self.program_settings.executable.clone(),
        ) else {
            return;
        };
        let destination = self
            .preview
            .program_inputs
            .get("ps-shortcut-dest")
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_else(|| self.program_settings.shortcut_destination.clone());
        let destination = crate::program_settings::expand_home(destination.trim());
        if !destination.starts_with('/') {
            self.program_settings.shortcut_error =
                Some(crate::i18n::s().state.shortcut_target_absolute.to_owned());
            cx.notify();
            return;
        }
        self.program_settings.shortcut_destination = destination.clone();
        self.program_settings.shortcut_error = None;
        self.bridge
            .create_program_shortcut(&container, &executable, &destination);
        cx.notify();
    }

    pub(crate) fn delete_program_run(&mut self, run_id: String, cx: &mut Context<Self>) {
        let (Some(container), Some(executable)) = (
            self.program_settings.container.clone(),
            self.program_settings.executable.clone(),
        ) else {
            return;
        };
        self.program_settings.history_loading = true;
        self.bridge
            .delete_program_history(&container, &executable, Some(&run_id));
        cx.notify();
    }

    pub(crate) fn clear_program_history(&mut self, cx: &mut Context<Self>) {
        let (Some(container), Some(executable)) = (
            self.program_settings.container.clone(),
            self.program_settings.executable.clone(),
        ) else {
            return;
        };
        self.program_settings.history_loading = true;
        self.bridge
            .delete_program_history(&container, &executable, None);
        cx.notify();
    }

    pub(crate) fn reload_program_history(&mut self, cx: &mut Context<Self>) {
        let (Some(container), Some(executable)) = (
            self.program_settings.container.clone(),
            self.program_settings.executable.clone(),
        ) else {
            return;
        };
        self.program_settings.history_loading = true;
        self.program_settings.history_error = None;
        self.bridge.load_program_history(&container, &executable);
        cx.notify();
    }

    pub(crate) fn add_program_env_row(&mut self, cx: &mut Context<Self>) {
        let id = self.program_settings.next_env_id;
        self.program_settings.next_env_id += 1;
        self.program_settings.env_rows.push(crate::program_settings::EnvRow {
            id,
            key: String::new(),
            value: String::new(),
        });
        cx.notify();
    }

    pub(crate) fn remove_program_env_row(&mut self, id: u64, cx: &mut Context<Self>) {
        self.program_settings.env_rows.retain(|row| row.id != id);
        self.preview
            .program_inputs
            .remove(&format!("ps-env-key-{id}"));
        self.preview
            .program_inputs
            .remove(&format!("ps-env-value-{id}"));
        cx.notify();
    }

    pub(crate) fn set_program_choice(&mut self, field: &str, raw: &str, cx: &mut Context<Self>) {
        if field == "locale" {
            // The locale row has its own environment side effects; it is
            // handled by `set_program_locale` from the click listener.
            return;
        }
        if field == "dllNewMode" {
            self.program_settings.dll_new_mode = raw.to_owned();
            return;
        }
        // The bottle's engine policy applies to program overrides too — the
        // draft can hold an inactive preference, but an enabling write goes
        // through the same confirmation gate as the config page.
        if self.program_gate_check(field, &serde_json::Value::String(raw.to_owned()), cx) {
            return;
        }
        self.write_program_choice(field, raw);
    }

    /// The ungated draft write behind `set_program_choice`; a confirmed
    /// approval applies through here instead of re-opening its own dialog.
    fn write_program_choice(&mut self, field: &str, raw: &str) {
        let Some(settings) = self.program_settings.settings.as_mut() else {
            return;
        };
        let overrides = settings.overrides_mut();
        match field {
            "graphicsBackend" => overrides.graphics_backend = Some(raw.to_owned()),
            "dxvkHud" => overrides.dxvk_hud = Some(raw.to_owned()),
            "enhancedSync" => overrides.enhanced_sync = Some(raw.to_owned()),
            "resolutionPreset" => overrides.resolution_preset = Some(raw.to_owned()),
            "gdiUpscaler" => overrides.gdi_upscaler = Some(raw.to_owned()),
            "gdiUpscalerCache" => overrides.gdi_upscaler_cache = Some(raw.to_owned()),
            _ => {}
        }
    }

    pub(crate) fn set_program_override_bool(
        &mut self,
        field: &str,
        value: bool,
        cx: &mut Context<Self>,
    ) {
        if self.program_gate_check(field, &serde_json::Value::Bool(value), cx) {
            return;
        }
        self.write_program_override_bool(field, value);
    }

    /// The ungated draft write behind `set_program_override_bool`.
    fn write_program_override_bool(&mut self, field: &str, value: bool) {
        let Some(settings) = self.program_settings.settings.as_mut() else {
            return;
        };
        let overrides = settings.overrides_mut();
        match field {
            "dxvkAsync" => overrides.dxvk_async = Some(value),
            "metal4Enabled" => overrides.metal4_enabled = Some(value),
            "shaderCacheEnabled" => overrides.shader_cache_enabled = Some(value),
            "controllerCompatibilityMode" => overrides.controller_compatibility_mode = Some(value),
            "disableHIDAPI" => overrides.disable_hidapi = Some(value),
            "allowBackgroundEvents" => overrides.allow_background_events = Some(value),
            "disableControllerMapping" => overrides.disable_controller_mapping = Some(value),
            "useButtonLabels" => overrides.use_button_labels = Some(value),
            "emulateModeset" => overrides.emulate_modeset = Some(value),
            "virtualDesktopEnabled" => overrides.virtual_desktop_enabled = Some(value),
            _ => {}
        }
    }

    /// Turn a whole override group on with the bottle's current values, or
    /// clear every field in it. `groupName` matches the UI section.
    pub(crate) fn set_program_override_group(&mut self, group: &str, on: bool) {
        let Some(settings) = self.program_settings.settings.as_mut() else {
            return;
        };
        let defaults = settings.defaults.clone();
        let overrides = settings.overrides_mut();
        match group {
            "graphics" if on => {
                overrides.graphics_backend =
                    Some(defaults.graphics_backend.unwrap_or_else(|| "recommended".into()));
                overrides.dxvk = None;
                overrides.dxvk_async = defaults.dxvk_async;
                overrides.dxvk_hud = defaults.dxvk_hud;
                overrides.metal4_enabled = defaults.metal4_enabled;
            }
            "graphics" => {
                overrides.graphics_backend = None;
                overrides.dxvk = None;
                overrides.dxvk_async = None;
                overrides.dxvk_hud = None;
                overrides.metal4_enabled = None;
            }
            "sync" => {
                overrides.enhanced_sync = on.then_some(defaults.enhanced_sync).flatten();
            }
            "upscale" if on => {
                overrides.gdi_upscaler = Some(defaults.gdi_upscaler.unwrap_or_else(|| "off".into()));
                overrides.gdi_upscaler_cache =
                    Some(defaults.gdi_upscaler_cache.unwrap_or_else(|| "memory".into()));
            }
            "upscale" => {
                overrides.gdi_upscaler = None;
                overrides.gdi_upscaler_cache = None;
            }
            "performance" => {
                overrides.shader_cache_enabled =
                    on.then_some(defaults.shader_cache_enabled).flatten();
            }
            "input" if on => {
                overrides.controller_compatibility_mode = defaults.controller_compatibility_mode;
                overrides.disable_hidapi = defaults.disable_hidapi;
                overrides.allow_background_events = defaults.allow_background_events;
                overrides.disable_controller_mapping = defaults.disable_controller_mapping;
                overrides.use_button_labels = defaults.use_button_labels;
            }
            "input" => {
                overrides.controller_compatibility_mode = None;
                overrides.disable_hidapi = None;
                overrides.allow_background_events = None;
                overrides.disable_controller_mapping = None;
                overrides.use_button_labels = None;
            }
            "display" if on => {
                overrides.virtual_desktop_enabled =
                    Some(defaults.virtual_desktop_enabled.unwrap_or(false));
                overrides.resolution_preset =
                    defaults.resolution_preset.or_else(|| Some("r1920x1080".into()));
                overrides.custom_resolution_width = defaults.custom_resolution_width;
                overrides.custom_resolution_height = defaults.custom_resolution_height;
            }
            "display" => {
                overrides.virtual_desktop_enabled = None;
                overrides.resolution_preset = None;
                overrides.custom_resolution_width = None;
                overrides.custom_resolution_height = None;
            }
            "dll" => {
                overrides.dll_overrides = on.then(|| defaults.dll_overrides.unwrap_or_default());
            }
            _ => {}
        }
    }

    pub(crate) fn add_program_dll_override(&mut self, name: String, cx: &mut Context<Self>) {
        let name = name.trim().to_owned();
        if name.is_empty()
            || name.contains(char::is_whitespace)
            || name.contains('=')
            || name.contains(',')
        {
            self.show_notice(crate::i18n::s().state.dll_name_invalid, cx);
            return;
        }
        let Some(settings) = self.program_settings.settings.as_mut() else {
            return;
        };
        let overrides = settings.overrides_mut();
        overrides.dll_overrides.get_or_insert_with(Vec::new).push(
            crate::program_settings::DllOverrideDto {
                dll_name: name,
                mode: self.program_settings.dll_new_mode.clone(),
            },
        );
        self.preview.program_inputs.remove("ps-dll-name");
        cx.notify();
    }

    pub(crate) fn remove_program_dll_override(&mut self, index: usize) {
        let Some(settings) = self.program_settings.settings.as_mut() else {
            return;
        };
        let Some(overrides) = settings.overrides.as_mut() else {
            return;
        };
        let Some(entries) = overrides.dll_overrides.as_mut() else {
            return;
        };
        if index < entries.len() {
            entries.remove(index);
        }
    }

    /// Copy what the user typed in the environment inputs back into the draft
    /// rows, so rebuilding the inputs after a preset never drops edits.
    fn sync_env_rows_from_inputs(&mut self, cx: &mut Context<Self>) {
        for row in &mut self.program_settings.env_rows {
            if let Some(input) = self.preview.program_inputs.get(&format!("ps-env-key-{}", row.id)) {
                row.key = input.read(cx).value().to_string();
            }
            if let Some(input) = self.preview.program_inputs.get(&format!("ps-env-value-{}", row.id)) {
                row.value = input.read(cx).value().to_string();
            }
        }
    }

    /// Current value of one draft environment variable.
    pub(crate) fn program_env_value(&self, key: &str) -> Option<&str> {
        self.program_settings
            .env_rows
            .iter()
            .find(|row| row.key == key)
            .map(|row| row.value.as_str())
    }

    /// The WineD3D renderer named by `WINE_D3D_CONFIG`; `""` is the default.
    pub(crate) fn program_d3d_renderer(&self) -> Option<String> {
        let config = self.program_env_value("WINE_D3D_CONFIG").unwrap_or("");
        for entry in config.split(',') {
            if let Some((name, value)) = entry.split_once('=')
                && name.trim() == "renderer"
            {
                let value = value.trim();
                return match value {
                    "no3d" | "gdi" => Some("gdi".to_owned()),
                    "gl" | "vulkan" => Some(value.to_owned()),
                    _ => None,
                };
            }
        }
        Some(String::new())
    }

    /// Change only the renderer entry, keeping the user's other WineD3D options.
    pub(crate) fn set_program_renderer(&mut self, renderer: &str, cx: &mut Context<Self>) {
        self.sync_env_rows_from_inputs(cx);
        let existing = self.program_env_value("WINE_D3D_CONFIG").unwrap_or("").to_owned();
        let mut entries: Vec<String> = existing
            .split(',')
            .map(str::trim)
            .filter(|entry| {
                !entry.is_empty()
                    && entry.split_once('=').map(|(name, _)| name.trim()) != Some("renderer")
            })
            .map(str::to_owned)
            .collect();
        if !renderer.is_empty() {
            entries.push(format!("renderer={renderer}"));
        }
        self.program_settings.env_rows.retain(|row| row.key != "WINE_D3D_CONFIG");
        if !entries.is_empty() {
            let id = self.program_settings.next_env_id;
            self.program_settings.next_env_id += 1;
            self.program_settings.env_rows.push(crate::program_settings::EnvRow {
                id,
                key: "WINE_D3D_CONFIG".to_owned(),
                value: entries.join(","),
            });
        }
        self.preview
            .program_inputs
            .retain(|key, _| !key.starts_with("ps-env-"));
        cx.notify();
    }

    /// Locale presets also own LANG and LC_ALL, matching the SwiftUI editor.
    pub(crate) fn set_program_locale(&mut self, locale: String, cx: &mut Context<Self>) {
        self.sync_env_rows_from_inputs(cx);
        let Some(settings) = self.program_settings.settings.as_mut() else {
            return;
        };
        settings.locale = locale.clone();
        if locale.is_empty() {
            self.program_settings
                .env_rows
                .retain(|row| row.key != "LANG" && row.key != "LC_ALL");
        } else {
            for key in ["LANG", "LC_ALL"] {
                if let Some(row) = self
                    .program_settings
                    .env_rows
                    .iter_mut()
                    .find(|row| row.key == key)
                {
                    row.value = locale.clone();
                } else {
                    let id = self.program_settings.next_env_id;
                    self.program_settings.next_env_id += 1;
                    self.program_settings.env_rows.push(crate::program_settings::EnvRow {
                        id,
                        key: key.to_owned(),
                        value: locale.clone(),
                    });
                }
            }
        }
        self.preview
            .program_inputs
            .retain(|key, _| !key.starts_with("ps-env-"));
        cx.notify();
    }

    pub(crate) fn reload_config(&mut self, cx: &mut Context<Self>) {
        self.ensure_configuration(cx);
    }

    /// Toggle and choice rows commit immediately. The value is shown first and
    /// rolled back if the helper rejects it.
    pub(crate) fn set_config_value(
        &mut self,
        field: &str,
        next: serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.config.as_ref() else {
            return;
        };
        let Some(config_field) = page.fields.iter().find(|candidate| candidate.id == field) else {
            return;
        };
        if !config_field.enabled(&page.capabilities) {
            return;
        }
        let current = page
            .values
            .get(field)
            .cloned()
            .unwrap_or_else(|| config_field.default.clone());
        if current == next {
            return;
        }
        // Engine policy: a disabled-but-confirmable target needs the explicit
        // confirmation dialog first; an unavailable target is refused here so
        // even a direct call can't bypass the launch-side gate. Turning a
        // gated feature *off* is always allowed.
        let container = page.container.clone();
        if self.engine_gate_check(&container, field, &next, cx) {
            return;
        }
        self.config_writes.begin(field, current);
        if let Some(page) = self.config.as_mut() {
            page.values.insert(field.to_owned(), next.clone());
        }
        self.bridge.set_configuration(&container, field, next);
        cx.notify();
    }

    /// Called from the `InputState` observer on every notify; identical text is
    /// ignored, and the newest draft is sent after [`CONFIG_DEBOUNCE`].
    pub(crate) fn config_text_changed(&mut self, field: &str, text: &str, cx: &mut Context<Self>) {
        let Some(page) = self.config.as_ref().filter(|page| !page.loading) else {
            return;
        };
        let Some(config_field) = page.fields.iter().find(|candidate| candidate.id == field) else {
            return;
        };
        if !config_field.kind.is_textual() || !config_field.enabled(&page.capabilities) {
            return;
        }
        let committed = page
            .values
            .get(field)
            .and_then(|value| config::display_value(config_field, value));
        if committed.as_deref() == Some(text)
            || self.config_text_pending.get(field).map(String::as_str) == Some(text)
        {
            return;
        }
        self.config_text_pending
            .insert(field.to_owned(), text.to_owned());
        self.config_text_generation = self.config_text_generation.wrapping_add(1);
        let generation = self.config_text_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CONFIG_DEBOUNCE).await;
            let _ = this.update(cx, |this, cx| {
                if this.config_text_generation != generation {
                    return;
                }
                let pending = std::mem::take(&mut this.config_text_pending);
                for (field, raw) in pending {
                    this.commit_config_text(&field, &raw, cx);
                }
            });
        })
        .detach();
    }

    fn commit_config_text(&mut self, field: &str, raw: &str, cx: &mut Context<Self>) {
        let Some(page) = self.config.as_ref().filter(|page| !page.loading) else {
            return;
        };
        let Some(config_field) = page
            .fields
            .iter()
            .find(|candidate| candidate.id == field)
            .cloned()
        else {
            return;
        };
        let current = page
            .values
            .get(field)
            .cloned()
            .unwrap_or_else(|| config_field.default.clone());
        let container = page.container.clone();
        let value = match config::encode_edit(&config_field, raw) {
            Ok(value) => value,
            Err(reason) => {
                // The value never reached the helper: restore the last good one
                // and say why on the toast.
                self.preview.config_resets.insert(field.to_owned());
                self.show_notice(reason, cx);
                return;
            }
        };
        if value == current {
            return;
        }
        self.config_writes.begin(field, current);
        if let Some(page) = self.config.as_mut() {
            page.values.insert(field.to_owned(), value.clone());
        }
        // Trimmed numbers and a typed " 1920 " must not read back as a new
        // draft; normalize the field once the value is committed.
        if value.is_number() {
            self.preview.config_resets.insert(field.to_owned());
        }
        self.bridge.set_configuration(&container, field, value);
        cx.notify();
    }

    /// Stage an "编辑" row into the two environment inputs. `set_value` needs a
    /// window, which the click listener has.
    pub(crate) fn begin_environment_edit(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        // Applied by replacing the two inputs on the next render.
        self.env_fill = Some((key.to_owned(), value.to_owned()));
        self.env_original = Some(key.to_owned());
        cx.notify();
    }

    pub(crate) fn save_environment(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        let Some(container) = self.selected_container().or_else(|| self.scan_container()) else {
            return;
        };
        let key = match config::validate_environment_name(key) {
            Ok(key) => key.to_owned(),
            Err(reason) => {
                self.show_notice(reason, cx);
                return;
            }
        };
        let original = self.env_original.take();
        self.env_clear_on_reply = true;
        self.bridge
            .set_environment(&container, &key, Some(value), original.as_deref());
        cx.notify();
    }

    pub(crate) fn remove_environment(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(container) = self.selected_container().or_else(|| self.scan_container()) else {
            return;
        };
        if self.env_original.as_deref() == Some(key) {
            self.env_original = None;
        }
        self.env_clear_on_reply = false;
        self.bridge.set_environment(&container, key, None, None);
        cx.notify();
    }

    /// Native multi-select folder picker, starting inside the bottle's C drive.
    pub(crate) fn choose_scan_roots(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.scan_container() else {
            return;
        };
        self.scan_root_picker = Some(container.clone());
        let start = format!("{container}/drive_c");
        self.bridge.choose_folders("scan-roots", &start);
        cx.notify();
    }

    /// Native picker for standalone game folders — a folder that IS one game,
    /// unlike a scan root whose children become the games.
    pub(crate) fn choose_assign_folders(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.scan_container() else {
            return;
        };
        self.scan_root_picker = Some(container.clone());
        let start = std::env::var_os("HOME")
            .map(|home| home.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{container}/drive_c"));
        self.bridge.choose_folders("assign-folders", &start);
        cx.notify();
    }

    /// Typed-path fallback for adding one scan root.
    pub(crate) fn add_scan_root(&mut self, path: &str, cx: &mut Context<Self>) {
        let path = path.trim();
        if path.is_empty() {
            self.show_notice(crate::i18n::s().state.enter_folder_path, cx);
            return;
        }
        if !path.starts_with('/') {
            self.show_notice(crate::i18n::s().state.enter_absolute_path, cx);
            return;
        }
        self.edit_folder(FolderEdit::AddRoot(path.to_owned()), cx);
    }

    /// Move one scan root (and its games) into another bottle.
    pub(crate) fn move_scan_root(
        &mut self,
        root: &str,
        target_index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(source) = self.scan_container() else {
            return;
        };
        let Some(target) = self
            .bottles
            .get(target_index)
            .filter(|bottle| bottle.available && bottle.path != source)
            .map(|bottle| bottle.path.clone())
        else {
            return;
        };
        self.bridge.move_scan_root(&source, root, &target);
        cx.notify();
    }

    pub(crate) fn edit_folder(&mut self, edit: FolderEdit, cx: &mut Context<Self>) {
        let Some(container) = self.scan_container() else {
            return;
        };
        self.folders_reload_after_write.insert(container.clone());
        self.bridge.edit_folders(&container, edit);
        cx.notify();
    }

    pub(crate) fn start_scan(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.scan_container() else {
            return;
        };
        if let Some(active) = self.scan.as_ref().filter(|scan| scan.running()) {
            if active.container != container {
                self.show_notice(crate::i18n::s().state.other_bottle_scan_running, cx);
            }
            return;
        }
        self.scan = Some(ScanState {
            container: container.clone(),
            status: Some(JobStatus::Queued),
            ..ScanState::default()
        });
        self.bridge.start_scan(&container);
        cx.notify();
    }

    pub(crate) fn cancel_scan(&mut self, cx: &mut Context<Self>) {
        if self.scan.as_ref().is_some_and(ScanState::running) {
            self.bridge.cancel_scan();
            cx.notify();
        }
    }

    /// The scan this sheet shows, if it belongs to `container`.
    pub(crate) fn scan_for(&self, container: &str) -> Option<&ScanState> {
        self.scan
            .as_ref()
            .filter(|scan| scan.container == container)
    }

    // MARK: - Programs and processes

    /// Read the program list once per bottle, or again after a failure.
    pub(crate) fn ensure_programs(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        if let Some(page) = &self.programs
            && page.container == container
            && (page.loading || page.error.is_none())
        {
            return;
        }
        self.programs = Some(ProgramsPage {
            container: container.clone(),
            table: ProgramTable::default(),
            loading: true,
            error: None,
        });
        self.bridge.load_programs(&container);
        cx.notify();
    }

    /// Retry a failed `programs.list`.
    pub(crate) fn reload_programs(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        self.programs = Some(ProgramsPage {
            container: container.clone(),
            table: ProgramTable::default(),
            loading: true,
            error: None,
        });
        self.bridge.load_programs(&container);
        cx.notify();
    }

    /// Read the process list once, then let the poll loop keep it fresh.
    pub(crate) fn ensure_processes(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        if let Some(page) = &self.processes
            && page.container == container
            && (page.loading || page.error.is_none())
        {
            return;
        }
        self.processes = Some(ProcessesPage {
            container: container.clone(),
            table: ProcessTable::default(),
            loading: true,
            error: None,
        });
        self.bridge.load_processes(&container);
        cx.notify();
    }

    /// The "刷新" row: re-read now; the poll loop continues afterwards.
    pub(crate) fn refresh_processes(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        if let Some(page) = self
            .processes
            .as_mut()
            .filter(|page| page.container == container)
        {
            page.error = None;
        } else {
            self.processes = Some(ProcessesPage {
                container: container.clone(),
                table: ProcessTable::default(),
                loading: true,
                error: None,
            });
        }
        self.bridge.load_processes(&container);
        cx.notify();
    }

    /// Poll `processes.list` while the Processes page is open. The cadence
    /// follows [`processes::poll_interval`]; the loop exits on the next tick
    /// after [`Self::stop_process_polling`] bumps the generation.
    fn start_process_polling(&mut self, cx: &mut Context<Self>) {
        self.process_generation = self.process_generation.wrapping_add(1);
        let generation = self.process_generation;
        cx.spawn(async move |this, cx| {
            loop {
                let cadence = {
                    let mut scheduled = None;
                    let _ = this.update(cx, |this, _cx| {
                        if this.process_generation != generation {
                            return;
                        }
                        if let Some(container) = this.selected_container() {
                            this.bridge.load_processes(&container);
                        }
                        let any_alive = this
                            .processes
                            .as_ref()
                            .is_some_and(|page| page.table.any_alive());
                        scheduled = Some(processes::poll_interval(any_alive));
                    });
                    match scheduled {
                        Some(cadence) => cadence,
                        None => break,
                    }
                };
                cx.background_executor().timer(cadence).await;
            }
        })
        .detach();
    }

    /// Stop the poll loop at the end of the current interval (or now, on the
    /// loop's next generation check).
    fn stop_process_polling(&mut self) {
        self.process_generation = self.process_generation.wrapping_add(1);
    }

    /// Flip a program's pin state. The row changes at once and the reply's
    /// refreshed list settles it; a rejection rolls the flag back.
    pub(crate) fn toggle_program_pin(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        let Some(page) = self
            .programs
            .as_mut()
            .filter(|page| page.container == container)
        else {
            return;
        };
        let Some(row) = page.table.row(path) else {
            return;
        };
        let pinned = row.pinned;
        let name = row.name.clone();
        if !page.table.set_pinned(path, !pinned) {
            return;
        }
        self.bridge
            .set_program_pinned(&container, path, &name, !pinned);
        cx.notify();
    }

    /// Flip a program's block state; same optimistic contract as the pin.
    pub(crate) fn toggle_program_block(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        let Some(page) = self
            .programs
            .as_mut()
            .filter(|page| page.container == container)
        else {
            return;
        };
        let Some(row) = page.table.row(path) else {
            return;
        };
        let blocked = row.blocked;
        if !page.table.set_blocked(path, !blocked) {
            return;
        }
        self.bridge.set_program_blocked(&container, path, !blocked);
        cx.notify();
    }

    /// The library entry a program launches through, when one exists.
    ///
    /// Pinned programs gain an entry named `pinned:<path>`
    /// (`ContainerProgram.entry`); folder and store entries match by their
    /// launch executable. A program with no entry cannot be started through
    /// `runtime.launch`, which resolves an `entryID`, so those rows simply
    /// offer no Play.
    pub(crate) fn program_entry_id(
        &self,
        container: &str,
        program: &ProgramDto,
    ) -> Option<String> {
        let pinned_id = program.entry_id();
        let executable = std::path::Path::new(&program.path);
        self.bottle_entries.get(container)?.iter().find_map(|game| {
            (game.id == pinned_id || game.executable.as_deref() == Some(executable))
                .then(|| game.id.clone())
        })
    }

    /// Play a program through its library entry. The executable parameter
    /// names exactly what to run, matching `RuntimeService.launch`'s
    /// `executable` override.
    pub(crate) fn play_program(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        let program = self
            .programs
            .as_ref()
            .filter(|page| page.container == container)
            .and_then(|page| page.table.row(path))
            .cloned();
        let entry_id = program
            .as_ref()
            .and_then(|row| self.program_entry_id(&container, row));
        // Refresh the live entry set immediately before launch. The cached set
        // only suppresses redundant selection/connect work; a bottle configured
        // before this program was added would otherwise reject the launch.
        let _ = self.configured.ensure(&container);
        self.bridge.configure_runtime(&container);
        let program_name = program
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| {
                std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_owned())
            });
        if let Some(entry_id) = entry_id {
            self.show_notice(crate::i18n::s().state.notice_launching(&program_name), cx);
            self.runtime.set_state(&entry_id, RuntimeState::Preparing);
            self.bridge.launch(&container, &entry_id, Some(path));
        } else if let Some(program) = program.filter(|p| p.available && !p.blocked) {
            self.show_notice(crate::i18n::s().state.notice_launching(&program.name), cx);
            self.bridge.run_program(&container, &program.path, &program.name);
        } else {
            self.show_notice(crate::i18n::s().state.program_unavailable_or_blocked, cx);
            return;
        }
        cx.notify();
    }

    /// A folder entry names its folder, so the helper cannot run it until the
    /// user has chosen an executable for that folder.
    fn needs_executable(&self, index: usize) -> bool {
        self.game(index).is_some_and(|game| game.needs_executable)
    }

    /// Open the in-app picker over a folder entry's `.exe` files.
    ///
    /// The helper resolves an already-recorded executable on its own, so Play
    /// only reaches this for a folder that has none. `launch_after` starts the
    /// game once a choice is made, which is what Play does; the card menu
    /// records the choice only.
    pub(crate) fn open_executable_picker(
        &mut self,
        index: usize,
        launch_after: bool,
        cx: &mut Context<Self>,
    ) {
        if self.game(index).is_some_and(|game| game.multi_executable) {
            self.open_multi_executable_picker(index, launch_after, false, false, cx);
            return;
        }
        let details = self.game(index).map(|game| {
            (
                game.container.clone(),
                game.id.clone(),
                game.title.clone(),
                game.folder(),
            )
        });
        let Some((container, entry_id, title, Some(folder))) = details else {
            self.show_notice(crate::i18n::s().state.entry_no_selectable_program, cx);
            return;
        };
        if container.is_empty() {
            return;
        }
        let folder = folder.to_string_lossy().to_string();
        self.executable_picker_generation = self.executable_picker_generation.wrapping_add(1);
        self.executable_picker = Some(ExecutablePickerState {
            game_index: index,
            container: container.clone(),
            entry_id,
            title,
            folder: folder.clone(),
            launch_after,
            multi_select: false,
            selected: Vec::new(),
            return_to_run_settings: false,
            terminal: false,
            loading: true,
            error: None,
            candidates: Vec::new(),
        });
        self.bridge.load_candidates(&container, &folder);
        self.open_preview_sheet(PreviewSheet::Executable, cx);
    }

    pub(crate) fn open_multi_executable_picker(
        &mut self,
        index: usize,
        launch_after: bool,
        return_to_run_settings: bool,
        terminal: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(game) = self.game(index).cloned() else {
            return;
        };
        if self.demo || game.container.is_empty() {
            self.show_notice(demo_notice(), cx);
            return;
        }
        if !game.supports_multi_executable() {
            self.show_notice(crate::i18n::s().state.entry_no_multi_exe, cx);
            return;
        }
        let Some(folder) = game.game_directory() else {
            self.show_notice(crate::i18n::s().state.game_dir_unavailable, cx);
            return;
        };
        let mut selected = game.launch_executables.clone();
        if selected.is_empty()
            && let Some(exe) = game.executable.as_ref().filter(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            })
        {
            selected.push(exe.to_string_lossy().into_owned());
        }
        let folder = folder.to_string_lossy().to_string();
        self.executable_picker_generation = self.executable_picker_generation.wrapping_add(1);
        self.executable_picker = Some(ExecutablePickerState {
            game_index: index,
            container: game.container.clone(),
            entry_id: game.id.clone(),
            title: game.title.clone(),
            folder: folder.clone(),
            launch_after,
            multi_select: true,
            selected,
            return_to_run_settings,
            terminal,
            loading: true,
            error: None,
            candidates: Vec::new(),
        });
        self.bridge.load_candidates(&game.container, &folder);
        self.open_preview_sheet(PreviewSheet::Executable, cx);
    }

    /// Record the picked executable through `folders.executable`, then honour
    /// what opened the picker.
    pub(crate) fn choose_executable(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(picker) = self.executable_picker.take() else {
            return;
        };
        self.bridge.edit_folders(
            &picker.container,
            FolderEdit::Executable {
                path: picker.folder.clone(),
                executable: Some(path.to_owned()),
            },
        );
        let folder_path = std::path::PathBuf::from(&picker.folder);
        let exe_path = std::path::PathBuf::from(path);
        self.update_entry(&picker.container, &picker.entry_id, |game| {
            game.needs_executable = false;
            game.executable = Some(exe_path.clone());
            game.folder_path = Some(folder_path.clone());
        });
        if picker.launch_after {
            // The folder may have appeared after the bottle was configured, so
            // do not rely on the session cache before the ordered launch.
            let _ = self.configured.ensure(&picker.container);
            self.bridge.configure_runtime(&picker.container);
            let title = self
                .entries
                .iter()
                .find(|e| e.id == picker.entry_id)
                .map(|e| e.title.clone())
                .unwrap_or_else(|| {
                    std::path::Path::new(path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.to_owned())
                });
            self.show_notice(crate::i18n::s().state.notice_launching(&title), cx);
            self.active_executables
                .insert(picker.entry_id.clone(), path.to_owned());
            self.runtime
                .set_state(&picker.entry_id, RuntimeState::Preparing);
            self.bridge
                .launch(&picker.container, &picker.entry_id, Some(path));
        }
        self.preview.sheet = None;
        self.preview.sheet_history.clear();
        self.preview_sheet_return_captured_gen = 0;
        cx.notify();
    }

    /// Kill one process. The row leaves the list at once; the reply either
    /// settles the removal or rolls it back with the reason on the toast.
    pub(crate) fn kill_process(&mut self, pid: i64, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(container) = self.selected_container() else {
            return;
        };
        let Some(page) = self
            .processes
            .as_mut()
            .filter(|page| page.container == container)
        else {
            return;
        };
        if !page.table.begin_kill(pid) {
            return;
        }
        self.bridge.kill_process(&container, pid);
        cx.notify();
    }

    /// Container path that owns this card's entry.
    fn bottle_path(&self, index: usize) -> Option<String> {
        self.game(index)
            .map(|game| game.container.clone())
            .filter(|path| !path.is_empty())
    }
    /// Per-entry runtime state. Cards for entries the helper has never
    /// configured (the Library page before any bottle is opened, or the demo)
    /// have no state yet.
    pub(crate) fn runtime_state(&self, index: usize) -> Option<RuntimeState> {
        self.game(index)
            .and_then(|game| self.runtime.entry(&game.id))
            .map(|entry| entry.state)
    }

    /// Badge text showing the backend reported by the launch service.
    pub(crate) fn runtime_badge_label(&self, index: usize, base: &str) -> String {
        match self
            .game(index)
            .and_then(|g| self.active_backends.get(&g.id))
        {
            Some(backend) => format!("{base} · {backend}"),
            None => base.to_owned(),
        }
    }

    /// The card's primary action. Before a bottle is configured there is no
    /// runtime state, but the entry is launchable: the click configures the
    /// bottle first and the next snapshot replaces this optimistic action.
    pub(crate) fn primary_action(&self, index: usize) -> PrimaryAction {
        if let Some(entry) = self
            .game(index)
            .and_then(|game| self.runtime.entry(&game.id))
        {
            return primary_action_for(entry.state, entry.capabilities);
        }
        if !self.demo && self.bottle_path(index).is_some() {
            // Linux has no Steam launch adapter: a `steam:` entry's optimistic
            // Play would configure the bottle, then fail inside the helper.
            #[cfg(target_os = "linux")]
            if self
                .game(index)
                .is_some_and(|game| game.id.starts_with("steam:"))
            {
                return PrimaryAction::unavailable();
            }
            return PrimaryAction {
                label: crate::i18n::s().runtime.launch,
                action: Some(RuntimeAction::Launch),
                enabled: true,
                busy: false,
            };
        }
        PrimaryAction::unavailable()
    }

    /// Send the mutation and move the card into a plausible transitional state.
    /// The next snapshot is authoritative and corrects it.
    pub(crate) fn perform_runtime_action(
        &mut self,
        index: usize,
        action: Option<RuntimeAction>,
        cx: &mut Context<Self>,
    ) {
        let Some(action) = action else { return };
        let (Some(container), Some(entry_id)) = (
            self.bottle_path(index),
            self.game(index).map(|game| game.id.clone()),
        ) else {
            return;
        };
        match action {
            RuntimeAction::Launch => {
                // Always refresh the live entry set before a launch. The
                // worker handles commands in order, so this configure lands
                // before the launch; a bottle configured while empty must be
                // re-registered or the helper rejects it as not_configured.
                let _ = self.configured.ensure(&container);
                self.bridge.configure_runtime(&container);
                if let Some(game) = self.game(index).cloned()
                    && game.multi_executable
                {
                    if game.launch_executables.is_empty() {
                        self.open_multi_executable_picker(index, true, false, false, cx);
                    } else {
                        self.open_launch_chooser(index, false, cx);
                    }
                    return;
                }
                if self.needs_executable(index) {
                    self.open_executable_picker(index, true, cx);
                    return;
                }
                let title = self
                    .game(index)
                    .map(|g| g.title.clone())
                    .unwrap_or_default();
                self.show_notice(crate::i18n::s().state.notice_launching(&title), cx);
                self.runtime.set_state(&entry_id, RuntimeState::Preparing);
                self.bridge.launch(&container, &entry_id, None);
                if !cfg!(target_os = "linux") && self.capture.panel_open
                    && self
                        .capture
                        .context
                        .as_ref()
                        .is_some_and(|c| c.0 == container && c.1 == entry_id)
                {
                    self.capture.on_game_launched(
                        &self.bridge,
                        self.translation_page.provider_probe_pending(),
                        cx,
                    );
                }
            }
            RuntimeAction::Cancel => {
                // Cancel is only accepted while preparing/launching; showing the
                // pre-launch phase keeps the button unavailable until the next
                // event lands.
                self.runtime.set_state(&entry_id, RuntimeState::CheckingLaunch);
                self.bridge.cancel(&container, &entry_id);
            }
            RuntimeAction::Stop => {
                self.runtime.set_state(&entry_id, RuntimeState::Idle);
                self.bridge.stop(&container, &entry_id);
            }
        }
        cx.notify();
    }

    /// Set one rating (1–10). Choosing the current value clears it, which
    /// is how the detail panel expresses "unrated".
    pub(crate) fn set_rating(&mut self, index: usize, star: u8, cx: &mut Context<Self>) {
        let Some(game) = self.game(index) else { return };
        let previous = game.rating;
        let next = library_view::next_rating(previous, star);
        let (container, entry_id) = (game.container.clone(), game.id.clone());
        if self.demo || container.is_empty() {
            self.show_notice(demo_notice(), cx);
            return;
        }
        self.rating_edits.begin(&entry_key(&container, &entry_id), previous);
        self.update_entry(&container, &entry_id, |game| game.rating = next);
        self.rebuild_entries();
        self.bridge
            .set_rating(&container, &entry_id, next.map(i64::from));
        cx.notify();
    }

    /// Flip the visibility of one entry. The grid reacts immediately; the
    /// bridge reply (or an error rollback) settles it.
    pub(crate) fn toggle_hidden(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(game) = self.game(index) else { return };
        let (container, entry_id, previous) =
            (game.container.clone(), game.id.clone(), game.hidden);
        if self.demo || container.is_empty() {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let next = !previous;
        self.hidden_edits
            .begin(&entry_key(&container, &entry_id), previous);
        self.update_entry(&container, &entry_id, |game| game.hidden = next);
        self.rebuild_entries();
        self.bridge.set_hidden(&container, &entry_id, next);
        cx.notify();
    }

    /// Second launch call for a clipboard prompt. The token travels unchanged;
    /// dismissing the sheet calls `dismiss_clipboard` instead and never launches.
    pub(crate) fn resolve_clipboard(
        &mut self,
        decision: ClipboardDecision,
        cx: &mut Context<Self>,
    ) {
        let Some(prompt) = self.clipboard_prompt.take() else {
            return;
        };
        if let Some(executable) = prompt.executable.as_deref() {
            self.active_executables
                .insert(prompt.entry_id.clone(), executable.to_owned());
        }
        self.runtime
            .set_state(&prompt.entry_id, RuntimeState::Preparing);
        self.bridge.resolve_clipboard(prompt, decision);
        cx.notify();
    }

    pub(crate) fn dismiss_clipboard(&mut self, cx: &mut Context<Self>) {
        self.clipboard_prompt = None;
        cx.notify();
    }

    /// Apply the dialog's switch as the program override. The optional
    /// relaunch targets the entry whose finished run raised the suggestion.
    pub(crate) fn legacy_fullscreen_apply(&mut self, relaunch: bool, cx: &mut Context<Self>) {
        let Some(prompt) = self.legacy_fullscreen_prompt.take() else {
            return;
        };
        self.bridge.save_program_settings(
            &prompt.container,
            &prompt.executable,
            json!({ "overrides": { "emulateModeset": prompt.enable } }),
        );
        if relaunch {
            self.active_executables
                .insert(prompt.entry_id.clone(), prompt.executable.clone());
            self.runtime
                .set_state(&prompt.entry_id, RuntimeState::Preparing);
            self.bridge.launch(
                &prompt.container,
                &prompt.entry_id,
                Some(&prompt.executable),
            );
        }
        cx.notify();
    }

    /// Permanently silence the suggestion for this program. The manual switch
    /// in program settings stays reachable afterwards.
    pub(crate) fn legacy_fullscreen_dismiss_forever(&mut self, cx: &mut Context<Self>) {
        let Some(prompt) = self.legacy_fullscreen_prompt.take() else {
            return;
        };
        self.bridge.save_program_settings(
            &prompt.container,
            &prompt.executable,
            json!({ "legacyFullscreenDismissed": true }),
        );
        cx.notify();
    }

    pub(crate) fn dismiss_legacy_fullscreen(&mut self, cx: &mut Context<Self>) {
        self.legacy_fullscreen_prompt = None;
        cx.notify();
    }

    /// The dialog switch edits a draft; nothing is written until apply. The
    /// caller (`program_switch`) notifies.
    pub(crate) fn legacy_fullscreen_set_enable(&mut self, enable: bool) {
        if let Some(prompt) = self.legacy_fullscreen_prompt.as_mut() {
            prompt.enable = enable;
        }
    }
    pub(crate) fn navigate(&mut self, page: PreviewPage, cx: &mut Context<Self>) {
        if matches!(
            page,
            PreviewPage::ConfigureBottle
                | PreviewPage::Programs
                | PreviewPage::Processes
                | PreviewPage::GameConfigs
        ) {
            if page != self.preview.page {
                self.preview.history.push(self.preview.page);
            }
        } else {
            self.preview.history.clear();
        }
        self.preview.page = page;
        if matches!(page, PreviewPage::Statistics | PreviewPage::Launchers) {
            self.selected_bottle = None;
            if !self.demo {
                self.refresh_library(cx);
            }
        }
        if self.inline_settings_active() {
            self.settings = false;
        }
        self.detail_open = false;
        self.preview.sheet = None;
        self.hovered = None;
        self.apply_page_loads(cx);
        cx.notify();
    }

    /// The page-back button: pop history without pushing onto it.
    pub(crate) fn navigate_back(&mut self, cx: &mut Context<Self>) {
        self.preview.page = self.preview.history.pop().unwrap_or(PreviewPage::Bottle);
        self.apply_page_loads(cx);
        cx.notify();
    }

    /// Load what the new page shows and keep the process poll in step with
    /// the Processes page's visibility.
    fn apply_page_loads(&mut self, cx: &mut Context<Self>) {
        self.stop_process_polling();
        match self.preview.page {
            PreviewPage::ConfigureBottle => self.ensure_configuration(cx),
            PreviewPage::Programs => self.ensure_programs(cx),
            PreviewPage::Launchers => {
                // Entries and the auto-start set arrive on different replies;
                // cover containers whose entries landed before the page opened.
                let containers: Vec<String> = self.bottle_entries.keys().cloned().collect();
                for container in containers {
                    self.ensure_launchers(&container);
                }
            }
            PreviewPage::Processes => {
                self.ensure_processes(cx);
                self.start_process_polling(cx);
            }
            _ => {}
        }
    }
    pub(crate) fn open_preview_sheet(&mut self, sheet: PreviewSheet, cx: &mut Context<Self>) {
        let was_sheet_open = self.preview.sheet.is_some();
        let closing_detail = self.detail_open;
        if let Some(previous) = self.preview.sheet {
            self.preview.sheet_history.push(previous);
        } else {
            self.preview.sheet_history.clear();
        }
        self.preview.sheet = Some(sheet);
        self.detail_open = false;
        self.hovered = None;
        self.preview_sheet_generation = self.preview_sheet_generation.wrapping_add(1);
        if closing_detail {
            self.preview_sheet_return_focus = Some(
                self.detail_return_focus
                    .take()
                    .unwrap_or_else(|| self.focus_handle.clone()),
            );
            self.preview_sheet_return_captured_gen = self.preview_sheet_generation;
        } else if was_sheet_open {
            self.preview_sheet_return_captured_gen = self.preview_sheet_generation;
        } else {
            self.preview_sheet_return_focus = None;
        }
        if sheet == PreviewSheet::Scan {
            self.ensure_folders(cx);
        }
        if let PreviewSheet::RunSettings(index) = sheet {
            self.open_program_settings(index, cx);
            if !self.demo
                && let Some(game) = self.game(index).cloned()
                && !game.container.is_empty()
            {
                if self.service_available("translation.capture.preset/get") {
                    self.capture
                        .load_preset_for_game(&self.bridge, &game.container, &game.id);
                }
                if self.translation_page.providers.is_empty()
                    && self.service_available("translation.providers/list")
                {
                    self.translation_page.request(
                        &self.bridge,
                        "translation.providers/list",
                        serde_json::json!({}),
                    );
                }
            }
        }
        if let PreviewSheet::MoveEntry(index) = sheet {
            self.prepare_move_entry(index, cx);
        }
        if matches!(sheet, PreviewSheet::GameTags(_)) {
            self.tag_input_fill = Some(String::new());
        }
        if sheet == PreviewSheet::ManageTags {
            self.tag_manager_rename_target = None;
            self.tag_manager_fill = Some(String::new());
        }
        if sheet == PreviewSheet::CreateBottle {
            if self
                .create_bottle
                .as_ref()
                .is_some_and(|job| !job.running())
            {
                self.create_bottle = None;
            }
            if !self.wine_engines.is_empty() {
                let resolved = self.resolved_create_engine_id();
                if self
                    .wine_engines
                    .iter()
                    .any(|engine| engine.id == resolved && engine.available)
                {
                    self.preview.choices.insert("create-engine", resolved.into());
                }
            }
            if self.service_available("wine.engines.list") {
                self.bridge.load_wine_engines();
            }
        }
        if sheet == PreviewSheet::Winetricks {
            // A finished job is a result surface, not a permanent lock: opening
            // the sheet again starts from the catalog.
            if self.winetricks.job.as_ref().is_some_and(|job| job.finished) {
                self.winetricks.job = None;
            }
            self.ensure_winetricks(cx);
        }
        cx.notify();
    }
    pub(crate) fn preview_title(&self) -> SharedString {
        let strings = crate::i18n::s();
        if self.inline_settings_active() {
            return if self.translation_page.active {
                strings.capture.title.into()
            } else {
                strings.menus.settings.into()
            };
        }
        match self.preview.page {
            PreviewPage::Library => {
                if self.tag_bar_mode == crate::ui_state::TagBarMode::PullDown
                    && !self.pull_reveal.is_open()
                    && self.has_active_library_filter()
                {
                    let mut parts = Vec::new();
                    if self.studio_filter.is_active() {
                        parts.push(crate::i18n::s().library.title_studio(&self.studio_filter.display_label()));
                    }
                    if self.tag_filter.is_active() {
                        parts.push(crate::i18n::s().library.title_tag(&self.tag_filter.display_label()));
                    }
                    format!("{} · {}", strings.library.title, parts.join(" · ")).into()
                } else {
                    strings.library.title.into()
                }
            }
            PreviewPage::Statistics => strings.library.statistics.into(),
            PreviewPage::Launchers => strings.library.launchers.into(),
            PreviewPage::Bottle => self.bottle_name().to_owned().into(),
            PreviewPage::ConfigureBottle => strings.bottle.configure_bottle.into(),
            PreviewPage::Programs => strings.bottle.installed_programs.into(),
            PreviewPage::Processes => strings.bottle.running_processes.into(),
            PreviewPage::GameConfigs => strings.bottle.game_configs.into(),
        }
    }
    /// Enabled scraper sources, in provider order. Defaults to all of them.
    pub(crate) fn metadata_sources(&self) -> Vec<String> {
        let mut sources: Vec<String> = METADATA_SOURCES
            .iter()
            .filter(|(key, _)| self.flag(key))
            .map(|(_, source)| (*source).to_owned())
            .collect();
        sources.sort_by_key(|source| source != &self.metadata_preferences.default_source);
        sources
    }
    pub(crate) fn flag(&self, key: &'static str) -> bool {
        self.preview.toggles.get(key).copied().unwrap_or(false)
    }
    pub(crate) fn preview_single_line(&self) -> bool {
        self.flag("single-title")
    }
    /// Resolves the Wine engine id used when creating a new bottle: honors the
    /// current selection if it is available, otherwise prefers an available
    /// managed runtime or falls back to the first available external engine.
    pub(crate) fn resolved_create_engine_id(&self) -> String {
        if let Some(choice) = self
            .preview
            .choices
            .get("create-engine")
            .filter(|id| !id.is_empty())
        {
            if self.wine_engines.is_empty()
                || self
                    .wine_engines
                    .iter()
                    .any(|engine| engine.id == choice.as_str() && engine.available)
            {
                return choice.to_string();
            }
        }
        if self
            .wine_engines
            .iter()
            .any(|engine| engine.id == "managed" && engine.available)
        {
            return "managed".to_owned();
        }
        if let Some(first) = self.wine_engines.iter().find(|engine| engine.available) {
            return first.id.clone();
        }
        self.preview
            .choices
            .get("create-engine")
            .filter(|id| !id.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| "managed".to_owned())
    }

    pub(crate) fn preview_dismiss(&mut self, cx: &mut Context<Self>) {
        // While a bottle is being created the sheet is the only in-progress
        // cue; keep it on screen until the job settles (the button becomes
        // "关闭"), as the setup dialog does while installing.
        if self
            .create_bottle
            .as_ref()
            .is_some_and(crate::create_bottle::CreateBottleState::running)
        {
            return;
        }
        self.create_bottle = None;
        self.preview.sheet = None;
        self.preview.sheet_history.clear();
        self.preview_sheet_return_captured_gen = 0;
        self.program_settings.pending_switch = None;
        // A dismissed picker chose nothing, so nothing is recorded or launched.
        self.executable_picker = None;
        self.tag_manager_rename_target = None;
        self.tag_input_refocus = false;
        self.open_file = None;
        self.game_info = None;
        cx.notify();
    }
    /// Whether the `Hidden` filter currently includes hidden entries.
    pub(crate) fn show_hidden(&self) -> bool {
        self.show_hidden
    }
    /// Count of hidden entries in the current scope, for the filter chip.
    pub(crate) fn hidden_count(&self) -> usize {
        let slot = self.bottle_slot();
        self.games()
            .iter()
            .filter(|game| {
                (slot == 0 || game.bottle == slot) && game.hidden && !game.is_launcher
            })
            .count()
    }

    fn new_tag_input(
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<InputState>, Subscription) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::i18n::s().library.tag_input_placeholder)
                .default_value(value)
        });
        let sub = cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.submit_tag_input(None, cx);
            } else if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        (input, sub)
    }

    fn new_tag_manager_input(
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<InputState>, Subscription) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::i18n::s().library.tag_rename_placeholder)
                .default_value(value)
        });
        let sub = cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.confirm_rename_library_tag(None, cx);
            } else if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        (input, sub)
    }

    pub(crate) fn apply_pending_tag_inputs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(value) = self.tag_input_fill.take() {
            let (input, sub) = Self::new_tag_input(value, window, cx);
            self.tag_input = input;
            self.tag_input_subscription = sub;
            if std::mem::take(&mut self.tag_input_refocus) {
                let handle = self.tag_input.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
            }
        }
        if let Some(value) = self.tag_manager_fill.take() {
            let (input, sub) = Self::new_tag_manager_input(value, window, cx);
            self.tag_manager_input = input;
            self.tag_manager_input_subscription = sub;
        }
    }

    pub(crate) fn set_tag_filter(
        &mut self,
        filter: library_view::TagFilter,
        cx: &mut Context<Self>,
    ) {
        if self.tag_filter != filter {
            self.tag_filter = filter;
            if self.tag_filter.is_active() {
                self.filter_bar_tab = crate::ui_state::FilterBarTab::Tags;
                self.dismiss_tag_bar_hint_state();
                self.pull_reveal.set_open(true);
                self.save_library_ui_prefs();
            }
            self.hovered = None;
            self.invalidate_view();
            cx.notify();
        }
    }

    pub(crate) fn click_tag_filter(&mut self, tag: &str, multi: bool, cx: &mut Context<Self>) {
        if multi {
            self.tag_filter.toggle_multi(tag);
        } else {
            self.tag_filter.toggle_single(tag);
        }
        if self.tag_filter.is_active() {
            self.filter_bar_tab = crate::ui_state::FilterBarTab::Tags;
            self.dismiss_tag_bar_hint_state();
            self.pull_reveal.set_open(true);
            self.save_library_ui_prefs();
        }
        if self.settings || self.preview.page != PreviewPage::Library {
            self.settings = false;
            self.preview.page = PreviewPage::Library;
        }
        self.hovered = None;
        self.invalidate_view();
        cx.notify();
    }

    pub(crate) fn clear_tag_filter(&mut self, cx: &mut Context<Self>) {
        self.set_tag_filter(library_view::TagFilter::All, cx);
    }

    fn save_library_ui_prefs(&self) {
        if self.demo {
            return;
        }
        crate::ui_state::save_library_ui_prefs(&crate::ui_state::LibraryUiPrefs {
            tag_bar_mode: self.tag_bar_mode,
            tag_bar_hint_seen: self.tag_bar_hint_seen,
            group_by_studio: self.group_by_studio,
            filter_bar_tab: self.filter_bar_tab,
        });
    }

    pub(crate) fn has_active_library_filter(&self) -> bool {
        self.tag_filter.is_active() || self.studio_filter.is_active()
    }

    pub(crate) fn has_filter_bar_content(&self) -> bool {
        if self.has_active_library_filter() {
            return true;
        }
        let games = self.games();
        let show_hidden = self.show_hidden();
        !library_view::collect_tag_counts(games, show_hidden).is_empty()
            || !library_view::collect_studio_counts(games, self.bottle_slot(), show_hidden)
                .is_empty()
    }

    pub(crate) fn effective_filter_bar_tab(
        &self,
        has_tags: bool,
        has_studios: bool,
    ) -> crate::ui_state::FilterBarTab {
        match self.filter_bar_tab {
            crate::ui_state::FilterBarTab::Tags => {
                if !has_tags && !self.tag_filter.is_active() && has_studios {
                    crate::ui_state::FilterBarTab::Studios
                } else {
                    crate::ui_state::FilterBarTab::Tags
                }
            }
            crate::ui_state::FilterBarTab::Studios => {
                if !has_studios && !self.studio_filter.is_active() && has_tags {
                    crate::ui_state::FilterBarTab::Tags
                } else {
                    crate::ui_state::FilterBarTab::Studios
                }
            }
        }
    }

    pub(crate) fn has_library_overlay_open(&self) -> bool {
        self.detail_open
            || self.preview.sheet.is_some()
            || self.settings
            || self.bridge_dialog
            || self.bottle_dialog.is_some()
            || self.engine_dialog.is_some()
            || self.launch_chooser.is_some()
            || self.clipboard_prompt.is_some()
            || self.legacy_fullscreen_prompt.is_some()
            || self.setup_dialog
            || self.about
            || self.onboarding.is_some()
            || self.capture.panel_open
    }

    pub(crate) fn filter_bar_target_fraction(&self) -> f32 {
        if !self.has_filter_bar_content() {
            return 0.0;
        }
        if self.tag_bar_mode == crate::ui_state::TagBarMode::AlwaysVisible {
            return 1.0;
        }
        if self.pull_reveal.is_open() {
            return 1.0;
        }
        let pull = self.pull_reveal.pull_fraction();
        if pull > 0.0 {
            return pull;
        }
        if self.tag_bar_hint_peeking {
            return 0.56;
        }
        0.0
    }

    pub(crate) fn set_studio_filter(
        &mut self,
        filter: library_view::StudioFilter,
        cx: &mut Context<Self>,
    ) {
        let changed = self.studio_filter != filter;
        self.studio_filter = filter;
        if self.studio_filter.is_active() {
            self.filter_bar_tab = crate::ui_state::FilterBarTab::Studios;
            self.dismiss_tag_bar_hint_state();
            self.pull_reveal.set_open(true);
            self.save_library_ui_prefs();
        }
        if changed {
            self.hovered = None;
            self.invalidate_view();
        }
        cx.notify();
    }

    pub(crate) fn click_studio_filter(&mut self, studio: &str, cx: &mut Context<Self>) {
        self.studio_filter.toggle_studio(studio);
        if self.studio_filter.is_active() {
            self.filter_bar_tab = crate::ui_state::FilterBarTab::Studios;
            self.dismiss_tag_bar_hint_state();
            self.pull_reveal.set_open(true);
            self.save_library_ui_prefs();
        }
        if self.settings || self.preview.page != PreviewPage::Library {
            self.settings = false;
            self.preview.page = PreviewPage::Library;
        }
        self.hovered = None;
        self.invalidate_view();
        cx.notify();
    }

    pub(crate) fn toggle_unknown_studio_filter(&mut self, cx: &mut Context<Self>) {
        self.studio_filter.toggle_unknown();
        if self.studio_filter.is_active() {
            self.filter_bar_tab = crate::ui_state::FilterBarTab::Studios;
            self.dismiss_tag_bar_hint_state();
            self.pull_reveal.set_open(true);
            self.save_library_ui_prefs();
        }
        if self.settings || self.preview.page != PreviewPage::Library {
            self.settings = false;
            self.preview.page = PreviewPage::Library;
        }
        self.hovered = None;
        self.invalidate_view();
        cx.notify();
    }

    pub(crate) fn clear_studio_filter(&mut self, cx: &mut Context<Self>) {
        self.set_studio_filter(library_view::StudioFilter::All, cx);
    }

    pub(crate) fn clear_all_library_filters(&mut self, cx: &mut Context<Self>) {
        let changed = self.tag_filter.is_active() || self.studio_filter.is_active();
        self.tag_filter = library_view::TagFilter::All;
        self.studio_filter = library_view::StudioFilter::All;
        if changed {
            self.hovered = None;
            self.invalidate_view();
            cx.notify();
        }
    }

    pub(crate) fn toggle_group_by_studio(&mut self, cx: &mut Context<Self>) {
        self.group_by_studio = !self.group_by_studio;
        self.save_library_ui_prefs();
        self.hovered = None;
        self.invalidate_view();
        cx.notify();
    }

    pub(crate) fn toggle_studio_collapsed(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.collapsed_studios.remove(key) {
            self.collapsed_studios.insert(key.to_owned());
        }
        cx.notify();
    }

    pub(crate) fn set_filter_bar_tab(
        &mut self,
        tab: crate::ui_state::FilterBarTab,
        cx: &mut Context<Self>,
    ) {
        if self.filter_bar_tab != tab {
            self.filter_bar_tab = tab;
            self.save_library_ui_prefs();
            cx.notify();
        }
    }

    pub(crate) fn set_tag_bar_mode(
        &mut self,
        mode: crate::ui_state::TagBarMode,
        cx: &mut Context<Self>,
    ) {
        self.tag_bar_mode = mode;
        self.preview.choices.insert("tag-bar-mode", mode.wire().into());
        self.dismiss_tag_bar_hint_state();
        match mode {
            crate::ui_state::TagBarMode::AlwaysVisible => {
                self.pull_reveal.set_open(true);
            }
            crate::ui_state::TagBarMode::PullDown => {
                self.pull_reveal.set_open(self.has_active_library_filter());
            }
        }
        self.save_library_ui_prefs();
        cx.notify();
    }

    pub(crate) fn set_tag_drawer_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if open {
            self.dismiss_tag_bar_hint_state();
            self.tag_bar_hint_seen = true;
        }
        self.pull_reveal.set_open(open);
        self.save_library_ui_prefs();
        cx.notify();
    }

    fn dismiss_tag_bar_hint_state(&mut self) {
        self.tag_bar_hint_generation = self.tag_bar_hint_generation.wrapping_add(1);
        self.tag_bar_hint_peeking = false;
    }

    pub(crate) fn dismiss_tag_bar_hint(&mut self, open_drawer: bool, cx: &mut Context<Self>) {
        self.dismiss_tag_bar_hint_state();
        self.tag_bar_hint_seen = true;
        if open_drawer {
            self.pull_reveal.set_open(true);
        }
        self.save_library_ui_prefs();
        cx.notify();
    }

    pub(crate) fn handle_library_pull_wheel(
        &mut self,
        delta_y: f32,
        phase: library_view::PullWheelPhase,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.tag_bar_mode != crate::ui_state::TagBarMode::PullDown
            || !self.has_filter_bar_content()
            || self.has_library_overlay_open()
            || self.preview.page != PreviewPage::Library
        {
            return false;
        }
        let handle_index = self.mode.min(self.library_scroll.len() - 1);
        let scroll_y = f32::from(self.library_scroll[handle_index].offset().y);
        let now = cx.background_executor().now();
        let outcome = self.pull_reveal.on_wheel(
            delta_y,
            scroll_y,
            phase,
            now,
            self.has_active_library_filter(),
        );
        if outcome.opened || (outcome.changed && self.pull_reveal.pull_fraction() > 0.0) {
            let need_save = !self.tag_bar_hint_seen;
            if self.tag_bar_hint_peeking || !self.tag_bar_hint_seen {
                self.dismiss_tag_bar_hint_state();
                self.tag_bar_hint_seen = true;
                if need_save {
                    self.save_library_ui_prefs();
                }
            }
        }
        if outcome.needs_idle_reset {
            self.pull_idle_generation = self.pull_idle_generation.wrapping_add(1);
            let generation = self.pull_idle_generation;
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(360))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if !this.quitting
                        && this.pull_idle_generation == generation
                        && this.pull_reveal.reset_idle_pull()
                    {
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        if outcome.changed {
            cx.notify();
        }
        outcome.consume
    }

    pub(crate) fn maybe_start_tag_bar_hint(&mut self, cx: &mut Context<Self>) {
        if self.demo
            || crate::ui_state::capture_mode_active()
            || self.tag_bar_mode != crate::ui_state::TagBarMode::PullDown
            || self.tag_bar_hint_seen
            || self.tag_bar_hint_scheduled
            || self.pull_reveal.is_open()
            || self.has_library_overlay_open()
            || self.preview.page != PreviewPage::Library
            || !self.has_filter_bar_content()
        {
            return;
        }
        self.tag_bar_hint_scheduled = true;
        self.launch_tag_bar_hint_sequence(Duration::from_millis(450), cx);
    }

    pub(crate) fn replay_tag_bar_hint(&mut self, cx: &mut Context<Self>) {
        self.close_settings(None, cx);
        self.preview.page = PreviewPage::Library;
        if self.tag_bar_mode != crate::ui_state::TagBarMode::PullDown {
            self.tag_bar_mode = crate::ui_state::TagBarMode::PullDown;
            self.preview.choices.insert(
                "tag-bar-mode",
                crate::ui_state::TagBarMode::PullDown.wire().into(),
            );
        }
        self.pull_reveal.set_open(false);
        self.dismiss_tag_bar_hint_state();
        self.tag_bar_hint_scheduled = true;
        self.save_library_ui_prefs();
        if !self.has_filter_bar_content() {
            self.show_notice(crate::i18n::s().state.no_tags_or_studios, cx);
            cx.notify();
            return;
        }
        self.launch_tag_bar_hint_sequence(Duration::from_millis(220), cx);
        cx.notify();
    }

    fn launch_tag_bar_hint_sequence(
        &mut self,
        initial_delay: Duration,
        cx: &mut Context<Self>,
    ) {
        self.tag_bar_hint_generation = self.tag_bar_hint_generation.wrapping_add(1);
        let generation = self.tag_bar_hint_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(initial_delay).await;
            let started = this
                .update(cx, |this, cx| {
                    if this.quitting
                        || this.tag_bar_hint_generation != generation
                        || this.tag_bar_mode != crate::ui_state::TagBarMode::PullDown
                        || this.pull_reveal.is_open()
                        || this.has_library_overlay_open()
                        || this.preview.page != PreviewPage::Library
                        || !this.has_filter_bar_content()
                    {
                        return false;
                    }
                    this.tag_bar_hint_peeking = true;
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !started {
                return;
            }
            cx.background_executor()
                .timer(Duration::from_millis(2800))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.quitting || this.tag_bar_hint_generation != generation {
                    return;
                }
                this.tag_bar_hint_peeking = false;
                this.tag_bar_hint_seen = true;
                this.save_library_ui_prefs();
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn set_game_tags(
        &mut self,
        index: usize,
        tags: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(game) = self.game(index).cloned() else {
            return;
        };
        if self.demo || game.container.is_empty() {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let normalized = library_view::normalize_tags(tags);
        let key = entry_key(&game.container, &game.id);
        self.tag_edits.begin(&key, game.tags);
        self.update_entry(&game.container, &game.id, |entry| {
            entry.tags = normalized.clone();
        });
        if !self.bottle_entries.contains_key(&game.container) {
            for entry in self
                .entries
                .iter_mut()
                .filter(|e| e.container == game.container && e.id == game.id)
            {
                entry.tags = normalized.clone();
            }
            self.invalidate_view();
        } else {
            self.rebuild_entries();
        }
        self.bridge.set_tags(&game.container, &game.id, normalized);
        cx.notify();
    }

    pub(crate) fn toggle_game_tag(&mut self, index: usize, tag: &str, cx: &mut Context<Self>) {
        let Some(clean) = library_view::normalize_tag(tag) else {
            return;
        };
        let Some(game) = self.game(index).cloned() else {
            return;
        };
        let mut next = game.tags.clone();
        if game.has_tag(&clean) {
            let needle = clean.to_lowercase();
            next.retain(|item| item.to_lowercase() != needle);
        } else {
            next.push(clean);
        }
        self.set_game_tags(index, next, cx);
    }

    pub(crate) fn add_game_tags_from_text(
        &mut self,
        index: usize,
        raw: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let parsed: Vec<String> = raw
            .split([',', '，', '、'])
            .filter_map(library_view::normalize_tag)
            .collect();
        if parsed.is_empty() {
            return false;
        }
        let Some(game) = self.game(index).cloned() else {
            return false;
        };
        let mut next = game.tags;
        next.extend(parsed);
        self.set_game_tags(index, next, cx);
        true
    }

    pub(crate) fn submit_tag_input(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let target_index = match self.preview.sheet {
            Some(PreviewSheet::GameTags(idx)) => Some(idx),
            _ if self.detail_open => self.detail,
            _ => None,
        };
        let Some(index) = target_index else {
            return;
        };
        let raw = self.tag_input.read(cx).value().to_string();
        if self.add_game_tags_from_text(index, &raw, cx) {
            if let Some(window) = window {
                self.tag_input
                    .update(cx, |state, cx| state.set_value(String::new(), window, cx));
            } else {
                self.tag_input_fill = Some(String::new());
                self.tag_input_refocus = true;
            }
            cx.notify();
        }
    }

    pub(crate) fn open_game_tags_sheet(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        if self.game(index).is_some() {
            self.open_preview_sheet(PreviewSheet::GameTags(index), cx);
        }
    }

    pub(crate) fn open_manage_tags_sheet(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        self.open_preview_sheet(PreviewSheet::ManageTags, cx);
    }

    pub(crate) fn begin_rename_library_tag(
        &mut self,
        tag: &str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let Some(clean) = library_view::normalize_tag(tag) else {
            return;
        };
        self.tag_manager_rename_target = Some(clean.clone());
        if let Some(window) = window {
            self.tag_manager_input
                .update(cx, |state, cx| state.set_value(clean, window, cx));
        } else {
            self.tag_manager_fill = Some(clean);
        }
        cx.notify();
    }

    pub(crate) fn cancel_rename_library_tag(&mut self, cx: &mut Context<Self>) {
        self.tag_manager_rename_target = None;
        self.tag_manager_fill = Some(String::new());
        cx.notify();
    }

    pub(crate) fn confirm_rename_library_tag(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let Some(old_tag) = self.tag_manager_rename_target.clone() else {
            return;
        };
        let new_tag = self.tag_manager_input.read(cx).value().to_string();
        self.rename_library_tag(&old_tag, &new_tag, cx);
        if let Some(window) = window {
            self.tag_manager_input
                .update(cx, |state, cx| state.set_value(String::new(), window, cx));
        } else {
            self.tag_manager_fill = Some(String::new());
        }
    }

    pub(crate) fn rename_library_tag(
        &mut self,
        old_tag: &str,
        new_tag: &str,
        cx: &mut Context<Self>,
    ) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(old_clean) = library_view::normalize_tag(old_tag) else {
            return;
        };
        let Some(new_clean) = library_view::normalize_tag(new_tag) else {
            self.show_notice(crate::i18n::s().state.tag_name_empty, cx);
            return;
        };
        self.tag_manager_rename_target = None;
        self.apply_local_tag_rename(&old_clean, &new_clean);
        if self.bottle_entries.is_empty() {
            self.invalidate_view();
        } else {
            self.rebuild_entries();
        }
        self.bridge.rename_tag(None, &old_clean, &new_clean);
        cx.notify();
    }

    pub(crate) fn delete_library_tag(&mut self, tag: &str, cx: &mut Context<Self>) {
        if self.demo {
            self.show_notice(demo_notice(), cx);
            return;
        }
        let Some(clean) = library_view::normalize_tag(tag) else {
            return;
        };
        if self
            .tag_manager_rename_target
            .as_ref()
            .is_some_and(|target| target.to_lowercase() == clean.to_lowercase())
        {
            self.tag_manager_rename_target = None;
        }
        self.apply_local_tag_delete(&clean);
        if self.bottle_entries.is_empty() {
            self.invalidate_view();
        } else {
            self.rebuild_entries();
        }
        self.bridge.delete_tag(None, &clean);
        cx.notify();
    }

    fn apply_local_tag_rename(&mut self, old_tag: &str, new_tag: &str) {
        let Some(old_clean) = library_view::normalize_tag(old_tag) else {
            return;
        };
        let Some(new_clean) = library_view::normalize_tag(new_tag) else {
            return;
        };
        let old_key = old_clean.to_lowercase();
        let update_tags = |tags: &mut Vec<String>| {
            if tags.iter().any(|t| t.to_lowercase() == old_key) {
                let mapped: Vec<String> = tags
                    .iter()
                    .map(|t| {
                        if t.to_lowercase() == old_key {
                            new_clean.clone()
                        } else {
                            t.clone()
                        }
                    })
                    .collect();
                *tags = library_view::normalize_tags(mapped);
            }
        };
        for games in self.bottle_entries.values_mut() {
            for game in games.iter_mut() {
                update_tags(&mut game.tags);
            }
        }
        for game in &mut self.entries {
            update_tags(&mut game.tags);
        }
        if let library_view::TagFilter::Tags(active) = &mut self.tag_filter
            && active.iter().any(|t| t.to_lowercase() == old_key)
        {
            let mapped: Vec<String> = active
                .iter()
                .map(|t| {
                    if t.to_lowercase() == old_key {
                        new_clean.clone()
                    } else {
                        t.clone()
                    }
                })
                .collect();
            let normalized = library_view::normalize_tags(mapped);
            if normalized.is_empty() {
                self.tag_filter = library_view::TagFilter::All;
            } else {
                *active = normalized;
            }
        }
    }

    fn apply_local_tag_delete(&mut self, tag: &str) {
        let Some(clean) = library_view::normalize_tag(tag) else {
            return;
        };
        let target_key = clean.to_lowercase();
        for games in self.bottle_entries.values_mut() {
            for game in games.iter_mut() {
                game.tags.retain(|t| t.to_lowercase() != target_key);
            }
        }
        for game in &mut self.entries {
            game.tags.retain(|t| t.to_lowercase() != target_key);
        }
        if let library_view::TagFilter::Tags(active) = &mut self.tag_filter {
            active.retain(|t| t.to_lowercase() != target_key);
            if active.is_empty() {
                self.tag_filter = library_view::TagFilter::All;
            }
        }
    }

    pub(crate) fn set_multi_executable(
        &mut self,
        index: usize,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(game) = self.game(index).cloned() else {
            return;
        };
        if self.demo || game.container.is_empty() {
            self.show_notice(demo_notice(), cx);
            return;
        }
        if !game.supports_multi_executable() {
            self.show_notice(crate::i18n::s().state.multi_exe_unsupported, cx);
            return;
        }
        let key = entry_key(&game.container, &game.id);
        self.launch_options_edits.begin(
            &key,
            (
                game.multi_executable,
                game.launch_executables.clone(),
                game.last_executable.clone(),
            ),
        );
        self.update_entry(&game.container, &game.id, |entry| {
            entry.multi_executable = enabled;
        });
        if !self.bottle_entries.contains_key(&game.container) {
            for entry in self
                .entries
                .iter_mut()
                .filter(|e| e.container == game.container && e.id == game.id)
            {
                entry.multi_executable = enabled;
            }
            self.invalidate_view();
        } else {
            self.rebuild_entries();
        }
        if let Some(picker) = self.executable_picker.as_mut()
            && picker.container == game.container
            && picker.entry_id == game.id
        {
            picker.multi_select = enabled;
            if enabled && picker.selected.is_empty() {
                if !game.launch_executables.is_empty() {
                    picker.selected = game.launch_executables.clone();
                } else if let Some(exe) = game.executable.as_ref().filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                }) {
                    picker.selected.push(exe.to_string_lossy().into_owned());
                }
            }
        }
        self.bridge.set_launch_options(
            &game.container,
            &game.id,
            enabled,
            game.launch_executables,
            None,
        );
        cx.notify();
    }

    pub(crate) fn toggle_picker_executable(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(picker) = self.executable_picker.as_mut() else {
            return;
        };
        if !picker.multi_select {
            return;
        }
        if let Some(pos) = picker.selected.iter().position(|item| item == path) {
            picker.selected.remove(pos);
        } else {
            picker.selected.push(path.to_owned());
        }
        cx.notify();
    }

    pub(crate) fn cancel_executable_picker(&mut self, cx: &mut Context<Self>) {
        let return_index = self
            .executable_picker
            .as_ref()
            .filter(|picker| picker.return_to_run_settings)
            .map(|picker| picker.game_index);
        self.executable_picker = None;
        if let Some(index) = return_index {
            self.preview.sheet = Some(PreviewSheet::RunSettings(index));
            self.preview.sheet_history.clear();
            self.preview_sheet_generation = self.preview_sheet_generation.wrapping_add(1);
            self.preview_sheet_return_captured_gen = self.preview_sheet_generation;
            self.open_program_settings(index, cx);
        } else {
            self.preview.sheet = None;
            self.preview.sheet_history.clear();
            self.preview_sheet_return_captured_gen = 0;
        }
        cx.notify();
    }

    pub(crate) fn confirm_multi_executable_picker(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.executable_picker.as_ref() else {
            return;
        };
        if !picker.multi_select {
            return;
        }
        let mut seen = std::collections::HashSet::new();
        let selected: Vec<String> = picker
            .selected
            .iter()
            .filter(|path| seen.insert((*path).clone()))
            .cloned()
            .collect();
        if selected.is_empty() {
            self.show_notice(crate::i18n::s().state.select_one_exe_entry, cx);
            return;
        }
        let picker = self.executable_picker.take().unwrap();
        let prev = self.game(picker.game_index).map(|g| {
            (
                g.multi_executable,
                g.launch_executables.clone(),
                g.last_executable.clone(),
            )
        });
        let key = entry_key(&picker.container, &picker.entry_id);
        if let Some(prev) = prev {
            self.launch_options_edits.begin(&key, prev);
        }
        let next_last = self
            .game(picker.game_index)
            .and_then(|g| g.last_executable.clone())
            .filter(|last| selected.iter().any(|item| item == last));
        let fallback_exe = next_last.clone().unwrap_or_else(|| selected[0].clone());
        let apply_game = |entry: &mut Game| {
            entry.multi_executable = true;
            entry.launch_executables = selected.clone();
            entry.last_executable = next_last.clone();
            entry.needs_executable = false;
            let current_valid = entry.executable.as_ref().is_some_and(|path| {
                let text = path.to_string_lossy();
                selected.iter().any(|item| item == text.as_ref())
            });
            if !current_valid {
                entry.executable = Some(std::path::PathBuf::from(&fallback_exe));
            }
        };
        self.update_entry(&picker.container, &picker.entry_id, apply_game);
        if !self.bottle_entries.contains_key(&picker.container) {
            for entry in self
                .entries
                .iter_mut()
                .filter(|e| e.container == picker.container && e.id == picker.entry_id)
            {
                apply_game(entry);
            }
            self.invalidate_view();
        } else {
            self.rebuild_entries();
        }
        self.bridge.set_launch_options(
            &picker.container,
            &picker.entry_id,
            true,
            selected.clone(),
            None,
        );
        if picker.return_to_run_settings {
            self.preview.sheet = Some(PreviewSheet::RunSettings(picker.game_index));
            self.preview.sheet_history.clear();
            self.preview_sheet_generation = self.preview_sheet_generation.wrapping_add(1);
            self.preview_sheet_return_captured_gen = self.preview_sheet_generation;
            self.open_program_settings(picker.game_index, cx);
            cx.notify();
            return;
        }
        self.preview.sheet = None;
        self.preview.sheet_history.clear();
        self.preview_sheet_return_captured_gen = 0;
        if picker.launch_after {
            let _ = self.configured.ensure(&picker.container);
            self.bridge.configure_runtime(&picker.container);
            if selected.len() == 1 {
                self.launch_entry_directly(
                    &picker.container,
                    &picker.entry_id,
                    &picker.title,
                    &selected[0],
                    cx,
                );
            } else {
                self.launch_chooser_generation = self.launch_chooser_generation.wrapping_add(1);
                self.launch_chooser = Some(LaunchChooserState {
                    game_index: picker.game_index,
                    container: picker.container,
                    entry_id: picker.entry_id,
                    title: picker.title,
                    folder: picker.folder,
                    executables: selected,
                    last_executable: next_last,
                    terminal: picker.terminal,
                });
            }
        }
        cx.notify();
    }

    pub(crate) fn open_launch_chooser(
        &mut self,
        index: usize,
        terminal: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(game) = self.game(index).cloned() else {
            return;
        };
        if game.launch_executables.is_empty() {
            self.open_multi_executable_picker(index, true, false, terminal, cx);
            return;
        }
        let folder = game
            .game_directory()
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.launch_chooser_generation = self.launch_chooser_generation.wrapping_add(1);
        self.launch_chooser = Some(LaunchChooserState {
            game_index: index,
            container: game.container,
            entry_id: game.id,
            title: game.title,
            folder,
            executables: game.launch_executables,
            last_executable: game.last_executable,
            terminal,
        });
        self.hovered = None;
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn dismiss_launch_chooser(&mut self, cx: &mut Context<Self>) {
        self.launch_chooser = None;
        cx.notify();
    }

    pub(crate) fn restore_launch_chooser_focus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(handle) = self.launch_chooser_return_focus.take() {
            handle.focus(window, cx);
        } else {
            self.focus_handle.focus(window, cx);
        }
    }

    pub(crate) fn close_launch_chooser(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.launch_chooser = None;
        self.restore_launch_chooser_focus(window, cx);
        cx.notify();
    }

    pub(crate) fn restore_preview_sheet_focus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(handle) = self.preview_sheet_return_focus.take() {
            handle.focus(window, cx);
        } else {
            self.focus_handle.focus(window, cx);
        }
    }

    pub(crate) fn open_run_settings_for_executable(
        &mut self,
        index: usize,
        executable: String,
        cx: &mut Context<Self>,
    ) {
        let chooser_return = self.launch_chooser_return_focus.take();
        let was_detail_open = self.detail_open;
        self.launch_chooser = None;
        self.run_settings_target_exe = Some(executable);
        self.open_preview_sheet(PreviewSheet::RunSettings(index), cx);
        if !was_detail_open {
            self.preview_sheet_return_focus = chooser_return;
            self.preview_sheet_return_captured_gen = self.preview_sheet_generation;
        }
    }

    pub(crate) fn edit_launch_chooser_executables(&mut self, cx: &mut Context<Self>) {
        let Some(chooser) = self.launch_chooser.take() else {
            return;
        };
        let chooser_return = self.launch_chooser_return_focus.take();
        let was_detail_open = self.detail_open;
        self.open_multi_executable_picker(chooser.game_index, true, false, chooser.terminal, cx);
        if !was_detail_open {
            self.preview_sheet_return_focus = chooser_return;
            self.preview_sheet_return_captured_gen = self.preview_sheet_generation;
        }
    }

    pub(crate) fn launch_selected_entry(
        &mut self,
        executable: String,
        cx: &mut Context<Self>,
    ) {
        let Some(chooser) = self.launch_chooser.take() else {
            return;
        };
        let _ = self.configured.ensure(&chooser.container);
        self.bridge.configure_runtime(&chooser.container);
        let apply_last = |entry: &mut Game| {
            entry.last_executable = Some(executable.clone());
            entry.needs_executable = false;
        };
        self.update_entry(&chooser.container, &chooser.entry_id, apply_last);
        if !self.bottle_entries.contains_key(&chooser.container) {
            for entry in self
                .entries
                .iter_mut()
                .filter(|e| e.container == chooser.container && e.id == chooser.entry_id)
            {
                apply_last(entry);
            }
            self.invalidate_view();
        } else {
            self.rebuild_entries();
        }
        self.launch_entry_directly(
            &chooser.container,
            &chooser.entry_id,
            &chooser.title,
            &executable,
            cx,
        );
        cx.notify();
    }

    pub(crate) fn resolve_exited_executable(&mut self, entry_id: &str) -> Option<String> {
        if let Some(exe) = self.active_executables.remove(entry_id) {
            return Some(exe);
        }
        let entry = self.entries.iter().find(|entry| entry.id == entry_id)?;
        if entry.multi_executable {
            if let Some(last) = entry
                .last_executable
                .as_deref()
                .filter(|path| !path.is_empty())
            {
                return Some(last.to_owned());
            }
        }
        entry
            .executable
            .as_ref()
            .and_then(|path| path.to_str())
            .filter(|path| !path.is_empty())
            .map(ToOwned::to_owned)
    }

    fn launch_entry_directly(
        &mut self,
        container: &str,
        entry_id: &str,
        title: &str,
        executable: &str,
        cx: &mut Context<Self>,
    ) {
        let exe_name = std::path::Path::new(executable)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| executable.to_owned());
        self.show_notice(crate::i18n::s().state.notice_launching_entry(&exe_name, &title), cx);
        self.active_executables
            .insert(entry_id.to_owned(), executable.to_owned());
        self.runtime.set_state(entry_id, RuntimeState::Preparing);
        self.bridge.launch(container, entry_id, Some(executable));
        if !cfg!(target_os = "linux")
            && self.capture.panel_open
            && self
                .capture
                .context
                .as_ref()
                .is_some_and(|c| c.0 == container && c.1 == entry_id)
        {
            self.capture.on_game_launched(
                &self.bridge,
                self.translation_page.provider_probe_pending(),
                cx,
            );
        }
    }
}

/// Internal identity: the same bridge id in two bottles is two records.
fn entry_key(container: &str, entry_id: &str) -> String {
    format!("{container}\u{1}{entry_id}")
}

/// The backend a launch reply reports, annotated when the resolution fell
/// back: `backend` is always the backend the launch resolves to — configured
/// choice, per-program override, GameDB plan, steering and availability all
/// applied — and `backend_from` names the one that could not run.
fn backend_label(backend: Option<String>, backend_from: Option<String>) -> String {
    match (backend, backend_from) {
        (Some(effective), Some(from)) => crate::i18n::s().state.engine_fallback(&effective, &from),
        (Some(effective), None) => effective,
        (None, _) => crate::i18n::s().runtime.by_launch_config.to_owned(),
    }
}

/// Finder's duplicate naming: "Name Copy", then "Name Copy 2", …
fn next_duplicate_name(base: &str, bottles: &[BottleVm]) -> String {
    let candidate = format!("{base} Copy");
    if !bottles.iter().any(|bottle| bottle.name == candidate) {
        return candidate;
    }
    let mut counter = 2;
    loop {
        let candidate = format!("{base} Copy {counter}");
        if !bottles.iter().any(|bottle| bottle.name == candidate) {
            return candidate;
        }
        counter += 1;
    }
}

#[cfg(test)]
mod about_tab_tests {
    use super::{AboutTab, WhiskyApp};

    #[test]
    fn opening_and_resetting_about_tab_defaults_to_overview() {
        assert_eq!(AboutTab::default(), AboutTab::Overview);

        let mut about = false;
        let mut tab = AboutTab::Notices;
        WhiskyApp::open_about_state(&mut about, &mut tab);
        assert!(about);
        assert_eq!(tab, AboutTab::Overview);

        tab = AboutTab::License;
        WhiskyApp::reset_about_state(&mut about, &mut tab);
        assert!(!about);
        assert_eq!(tab, AboutTab::Overview);
    }
}
