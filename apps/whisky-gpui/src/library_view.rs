//! Pure library browsing: aggregation, query matching, and sort order.
//!
//! Nothing here touches a window or the bridge, so the rules can be tested
//! headlessly. The view layer only supplies the loaded entries and bottle
//! names, and caches the resulting index lists instead of recomputing them per
//! frame.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use crate::model::{BottleVm, Game};

pub(crate) const UNKNOWN_STUDIO_LABEL: &str = "未知厂商";
pub(crate) const PULL_OPEN_THRESHOLD: f32 = 40.0;
pub(crate) const PULL_CLOSE_THRESHOLD: f32 = 30.0;
pub(crate) const PULL_GESTURE_PAUSE_MS: u64 = 260;

/// Merge per-bottle caches in sidebar order.
///
/// Internal identity is `(container, entry id)`: the caches keep every bottle's
/// records separate. Bridge ids are themselves bottle-scoped (`pinned:` and
/// `steam:` both embed the path), so an id-only rule would never collapse a
/// cross-bottle duplicate. The display list is therefore de-duplicated on
/// `(title, artwork)` exactly as specified: the artwork key is its path
/// relative to the bottle, which is identical for the Steam cache and in-bottle
/// icons of the same game while remaining distinct for a separate install.
/// The first bottle in sidebar order wins.
pub(crate) fn merge_bottles(
    bottles: &[BottleVm],
    caches: &BTreeMap<String, Vec<Game>>,
) -> Vec<Game> {
    let mut seen: BTreeSet<(String, Option<String>)> = BTreeSet::new();
    let mut merged = Vec::new();
    for bottle in bottles {
        for game in caches.get(&bottle.path).into_iter().flatten() {
            if seen.insert((game.title.clone(), artwork_key(game))) {
                merged.push(game.clone());
            }
        }
    }
    merged
}

/// Artwork identity without the bottle prefix: `None` when there is no art.
fn artwork_key(game: &Game) -> Option<String> {
    let art = game.art.as_ref()?;
    let path = art.to_string_lossy();
    let container = game.container.trim_end_matches('/');
    if !container.is_empty()
        && let Some(relative) = path.strip_prefix(container)
    {
        return Some(relative.trim_start_matches('/').to_owned());
    }
    // Artwork outside the bottle (a test fixture, or a future shared cache):
    // the file name is the only stable part.
    art.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// `LibrarySort` (`WhiskyCore/LibraryOrdering.swift:21`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sort {
    Recent,
    Name,
    Bottle,
}

impl Sort {
    pub(crate) fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Name,
            2 => Self::Bottle,
            _ => Self::Recent,
        }
    }
}

/// Clicking the current star clears the rating; any other star selects it.
pub(crate) fn next_rating(current: Option<u8>, star: u8) -> Option<u8> {
    if current == Some(star) { None } else { Some(star) }
}

pub(crate) const MAX_TAGS_PER_ENTRY: usize = 32;
pub(crate) const MAX_TAG_LENGTH: usize = 32;

/// Normalize a user-supplied tag name matching Swift `LibraryTags.normalize`.
pub(crate) fn normalize_tag(raw: &str) -> Option<String> {
    if raw.contains('\0') {
        return None;
    }
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    let clamped: String = collapsed.chars().take(MAX_TAG_LENGTH).collect();
    let trimmed = clamped.trim().to_owned();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Normalize, deduplicate case-insensitively, and cap a tag list.
pub(crate) fn normalize_tags<I, S>(raw: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in raw {
        let Some(tag) = normalize_tag(item.as_ref()) else {
            continue;
        };
        if !seen.insert(tag.to_lowercase()) {
            continue;
        }
        out.push(tag);
        if out.len() >= MAX_TAGS_PER_ENTRY {
            break;
        }
    }
    out
}

/// Tag classification filter across library views.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum TagFilter {
    #[default]
    All,
    Untagged,
    Tags(Vec<String>),
}

impl TagFilter {
    pub(crate) fn is_active(&self) -> bool {
        !matches!(self, Self::All)
    }

    pub(crate) fn contains(&self, tag: &str) -> bool {
        let needle = tag.trim().to_lowercase();
        !needle.is_empty()
            && matches!(self, Self::Tags(tags) if tags.iter().any(|t| t.to_lowercase() == needle))
    }

    pub(crate) fn toggle_single(&mut self, tag: &str) {
        let Some(clean) = normalize_tag(tag) else {
            return;
        };
        let key = clean.to_lowercase();
        if matches!(self, Self::Tags(tags) if tags.len() == 1 && tags[0].to_lowercase() == key) {
            *self = Self::All;
        } else {
            *self = Self::Tags(vec![clean]);
        }
    }

    pub(crate) fn toggle_multi(&mut self, tag: &str) {
        let Some(clean) = normalize_tag(tag) else {
            return;
        };
        let key = clean.to_lowercase();
        let mut next = match std::mem::take(self) {
            Self::Tags(tags) => tags,
            Self::All | Self::Untagged => Vec::new(),
        };
        if let Some(pos) = next.iter().position(|t| t.to_lowercase() == key) {
            next.remove(pos);
        } else {
            next.push(clean);
        }
        *self = if next.is_empty() {
            Self::All
        } else {
            Self::Tags(next)
        };
    }

    pub(crate) fn display_label(&self) -> String {
        match self {
            Self::All => "全部".to_owned(),
            Self::Untagged => "未标记".to_owned(),
            Self::Tags(tags) => tags.join(" + "),
        }
    }
}

/// Case-insensitive match over the name, developer, release year, and tags.
pub(crate) fn matches_query(game: &Game, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    game.title.to_lowercase().contains(&query)
        || game.studio().to_lowercase().contains(&query)
        || game
            .year()
            .is_some_and(|year| year.to_lowercase().contains(&query))
        || game.tags.iter().any(|tag| tag.to_lowercase().contains(&query))
}

pub(crate) fn matches_tag_filter(game: &Game, filter: &TagFilter) -> bool {
    match filter {
        TagFilter::All => true,
        TagFilter::Untagged => game.tags.is_empty(),
        TagFilter::Tags(tags) => tags.iter().all(|tag| game.has_tag(tag)),
    }
}

/// Normalize a developer / studio display name by collapsing internal whitespace.
pub(crate) fn normalize_studio(raw: &str) -> Option<String> {
    if raw.contains('\0') {
        return None;
    }
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim().to_owned();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Studio / developer classification filter across library views.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum StudioFilter {
    #[default]
    All,
    Unknown,
    Studio(String),
}

impl StudioFilter {
    pub(crate) fn is_active(&self) -> bool {
        !matches!(self, Self::All)
    }

    pub(crate) fn matches(&self, game: &Game) -> bool {
        match self {
            Self::All => true,
            Self::Unknown => game.studio_name().is_none(),
            Self::Studio(studio) => game.has_studio(studio),
        }
    }

    pub(crate) fn is_studio(&self, studio: &str) -> bool {
        let Some(clean) = normalize_studio(studio) else {
            return false;
        };
        matches!(self, Self::Studio(active) if active.eq_ignore_ascii_case(&clean))
    }

    pub(crate) fn toggle_studio(&mut self, studio: &str) {
        let Some(clean) = normalize_studio(studio) else {
            return;
        };
        if self.is_studio(&clean) {
            *self = Self::All;
        } else {
            *self = Self::Studio(clean);
        }
    }

    pub(crate) fn toggle_unknown(&mut self) {
        *self = if *self == Self::Unknown {
            Self::All
        } else {
            Self::Unknown
        };
    }

    pub(crate) fn display_label(&self) -> String {
        match self {
            Self::All => "全部".to_owned(),
            Self::Unknown => UNKNOWN_STUDIO_LABEL.to_owned(),
            Self::Studio(studio) => studio.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StudioCluster {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) is_unknown: bool,
    pub(crate) indices: Vec<usize>,
}

/// Collect studio counts in the visible scope, ordered by count desc then natural name asc.
pub(crate) fn collect_studio_counts(
    games: &[Game],
    slot: usize,
    show_hidden: bool,
) -> Vec<(String, usize)> {
    let mut counts: std::collections::HashMap<String, (String, usize)> =
        std::collections::HashMap::new();
    for game in games {
        if (slot != 0 && game.bottle != slot)
            || game.is_launcher
            || (!show_hidden && game.hidden)
        {
            continue;
        }
        let Some(studio) = game.studio_name() else {
            continue;
        };
        let key = studio.to_lowercase();
        counts
            .entry(key)
            .and_modify(|(_, count)| *count += 1)
            .or_insert((studio, 1));
    }
    let mut list: Vec<(String, usize)> = counts.into_values().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| natural_cmp(&a.0, &b.0)));
    list
}

/// Count of visible non-launcher games without a studio name in the scope.
pub(crate) fn unknown_studio_count(games: &[Game], slot: usize, show_hidden: bool) -> usize {
    games
        .iter()
        .filter(|game| {
            (slot == 0 || game.bottle == slot)
                && !game.is_launcher
                && (show_hidden || !game.hidden)
                && game.studio_name().is_none()
        })
        .count()
}

/// Group ordered game indices by studio while preserving their relative order inside each cluster.
/// Known studios come first (larger clusters first, then alphabetically); unknown comes last.
pub(crate) fn group_indices_by_studio(games: &[Game], indices: &[usize]) -> Vec<StudioCluster> {
    let mut grouped: std::collections::HashMap<String, (String, Vec<usize>)> =
        std::collections::HashMap::new();
    let mut unknown = Vec::new();
    for &index in indices {
        let Some(game) = games.get(index) else {
            continue;
        };
        if let Some(studio) = game.studio_name() {
            let key = studio.to_lowercase();
            grouped
                .entry(key)
                .and_modify(|(_, items)| items.push(index))
                .or_insert_with(|| (studio, vec![index]));
        } else {
            unknown.push(index);
        }
    }
    let mut clusters: Vec<StudioCluster> = grouped
        .into_iter()
        .map(|(key, (label, indices))| StudioCluster {
            key,
            label,
            is_unknown: false,
            indices,
        })
        .collect();
    clusters.sort_by(|a, b| {
        b.indices
            .len()
            .cmp(&a.indices.len())
            .then_with(|| natural_cmp(&a.label, &b.label))
    });
    if !unknown.is_empty() {
        clusters.push(StudioCluster {
            key: String::new(),
            label: UNKNOWN_STUDIO_LABEL.to_owned(),
            is_unknown: true,
            indices: unknown,
        });
    }
    clusters
}

/// Scroll-wheel phase simplified for the pull-down drawer state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PullWheelPhase {
    Started,
    Moved,
    Ended,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PullRevealOutcome {
    pub(crate) changed: bool,
    pub(crate) consume: bool,
    pub(crate) opened: bool,
    pub(crate) closed: bool,
    pub(crate) needs_idle_reset: bool,
}

/// State machine implementing the two-step "second downward pull at top" drawer gesture.
#[derive(Clone, Debug, Default)]
pub(crate) struct PullRevealState {
    open: bool,
    pull_distance: f32,
    close_scroll_distance: f32,
    in_touch_gesture: bool,
    /// Blocks opening during the same scroll/momentum sequence that brought the viewport to top.
    blocked_until_next_gesture: bool,
    last_wheel_at: Option<Instant>,
}

impl PullRevealState {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn pull_fraction(&self) -> f32 {
        (self.pull_distance / PULL_OPEN_THRESHOLD).clamp(0.0, 1.0)
    }

    pub(crate) fn set_open(&mut self, open: bool) {
        self.open = open;
        self.pull_distance = 0.0;
        self.close_scroll_distance = 0.0;
        if !open {
            self.blocked_until_next_gesture = false;
        }
    }

    pub(crate) fn reset_idle_pull(&mut self) -> bool {
        if !self.open && !self.in_touch_gesture && self.pull_distance > 0.0 {
            self.pull_distance = 0.0;
            true
        } else {
            false
        }
    }

    pub(crate) fn on_wheel(
        &mut self,
        delta_y: f32,
        scroll_y: f32,
        phase: PullWheelPhase,
        now: Instant,
        has_active_filter: bool,
    ) -> PullRevealOutcome {
        let paused = self
            .last_wheel_at
            .replace(now)
            .is_some_and(|at| now.saturating_duration_since(at) >= Duration::from_millis(PULL_GESTURE_PAUSE_MS));
        let at_top = scroll_y >= -1.0;
        match phase {
            PullWheelPhase::Started => self.in_touch_gesture = true,
            PullWheelPhase::Ended => self.in_touch_gesture = false,
            PullWheelPhase::Moved => {}
        }

        let mut outcome = PullRevealOutcome::default();
        if !self.open {
            if !at_top {
                self.blocked_until_next_gesture = true;
                if self.pull_distance > 0.0 {
                    self.pull_distance = 0.0;
                    outcome.changed = true;
                }
                return outcome;
            }
            match phase {
                PullWheelPhase::Started => {
                    self.blocked_until_next_gesture = false;
                    if self.pull_distance > 0.0 {
                        self.pull_distance = 0.0;
                        outcome.changed = true;
                    }
                    return outcome;
                }
                PullWheelPhase::Ended => {
                    if !self.blocked_until_next_gesture && self.pull_distance >= PULL_OPEN_THRESHOLD {
                        self.open = true;
                        self.pull_distance = 0.0;
                        self.close_scroll_distance = 0.0;
                        outcome.opened = true;
                        outcome.changed = true;
                    } else if self.pull_distance > 0.0 {
                        self.pull_distance = 0.0;
                        outcome.changed = true;
                    }
                    return outcome;
                }
                PullWheelPhase::Moved => {
                    if self.blocked_until_next_gesture {
                        if paused {
                            self.blocked_until_next_gesture = false;
                        } else {
                            return outcome;
                        }
                    }
                    if delta_y > 0.0 {
                        self.pull_distance = (self.pull_distance + delta_y).min(PULL_OPEN_THRESHOLD);
                        outcome.consume = true;
                        outcome.changed = true;
                        if self.pull_distance >= PULL_OPEN_THRESHOLD {
                            self.open = true;
                            self.pull_distance = 0.0;
                            self.close_scroll_distance = 0.0;
                            outcome.opened = true;
                        } else if !self.in_touch_gesture {
                            outcome.needs_idle_reset = true;
                        }
                    } else if delta_y < 0.0 && self.pull_distance > 0.0 {
                        self.pull_distance = (self.pull_distance + delta_y).max(0.0);
                        outcome.consume = true;
                        outcome.changed = true;
                    }
                    return outcome;
                }
            }
        }

        if delta_y < 0.0 && !has_active_filter {
            if paused {
                self.close_scroll_distance = 0.0;
            }
            self.close_scroll_distance += -delta_y;
            if self.close_scroll_distance >= PULL_CLOSE_THRESHOLD || scroll_y < -20.0 {
                self.open = false;
                self.close_scroll_distance = 0.0;
                self.pull_distance = 0.0;
                self.blocked_until_next_gesture = !at_top;
                outcome.closed = true;
                outcome.changed = true;
            }
        } else if delta_y > 0.0 {
            self.close_scroll_distance = 0.0;
        }
        outcome
    }
}

/// Indices in the visible scope, before ordering. `slot` is one-based and `0`
/// means the whole library.
#[cfg(test)]
pub(crate) fn filter_indices(
    games: &[Game],
    query: &str,
    slot: usize,
    show_hidden: bool,
) -> Vec<usize> {
    filter_indices_with_filters(games, query, &TagFilter::All, &StudioFilter::All, slot, show_hidden)
}

#[cfg(test)]
pub(crate) fn filter_indices_with_tags(
    games: &[Game],
    query: &str,
    tag_filter: &TagFilter,
    slot: usize,
    show_hidden: bool,
) -> Vec<usize> {
    filter_indices_with_filters(games, query, tag_filter, &StudioFilter::All, slot, show_hidden)
}

pub(crate) fn filter_indices_with_filters(
    games: &[Game],
    query: &str,
    tag_filter: &TagFilter,
    studio_filter: &StudioFilter,
    slot: usize,
    show_hidden: bool,
) -> Vec<usize> {
    (0..games.len())
        .filter(|&index| {
            let game = &games[index];
            // Launchers are storefront clients, not games: they live on the
            // dedicated 启动器 page instead of any game grid.
            (slot == 0 || game.bottle == slot)
                && !game.is_launcher
                && (show_hidden || !game.hidden)
                && matches_query(game, query)
                && matches_tag_filter(game, tag_filter)
                && studio_filter.matches(game)
        })
        .collect()
}

pub(crate) fn collect_tag_counts(games: &[Game], show_hidden: bool) -> Vec<(String, usize)> {
    let mut counts: std::collections::HashMap<String, (String, usize)> =
        std::collections::HashMap::new();
    for game in games {
        if game.is_launcher || (!show_hidden && game.hidden) {
            continue;
        }
        let mut seen_in_game = std::collections::HashSet::new();
        for tag in &game.tags {
            let key = tag.to_lowercase();
            if !seen_in_game.insert(key.clone()) {
                continue;
            }
            counts
                .entry(key)
                .and_modify(|(_, count)| *count += 1)
                .or_insert_with(|| (tag.clone(), 1));
        }
    }
    let mut list: Vec<(String, usize)> = counts.into_values().collect();
    list.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    list
}

pub(crate) fn untagged_count(games: &[Game], show_hidden: bool) -> usize {
    games
        .iter()
        .filter(|game| !game.is_launcher && (show_hidden || !game.hidden) && game.tags.is_empty())
        .count()
}

/// Order `indices` in place. The tie-break is always the name, matching
/// `LibraryOrdering.byName`, so the result is deterministic. `groups_launchers`
/// mirrors `groupsLaunchers: displayMode != .timeline` (`LibraryView.swift`):
/// launchers sink below the games unless the timeline presentation is active.
pub(crate) fn sort_indices(
    games: &[Game],
    indices: &mut [usize],
    sort: Sort,
    bottles: &[BottleVm],
    groups_launchers: bool,
) {
    indices.sort_by(|&first, &second| {
        compare(games, bottles, sort, groups_launchers, first, second)
            .then_with(|| first.cmp(&second))
    });
}

fn compare(
    games: &[Game],
    bottles: &[BottleVm],
    sort: Sort,
    groups_launchers: bool,
    first: usize,
    second: usize,
) -> Ordering {
    let first_game = &games[first];
    let second_game = &games[second];
    if groups_launchers && first_game.is_launcher != second_game.is_launcher {
        // `LibraryOrdering.sorted`: non-launchers first, launchers last.
        return first_game.is_launcher.cmp(&second_game.is_launcher);
    }
    let primary = match sort {
        // `recent` puts the newest confirmed session first and dated entries
        // before undated ones (`LibraryOrdering.sorted`, case `.recent`).
        Sort::Recent => match (
            first_game.last_session_ended_at,
            second_game.last_session_ended_at,
        ) {
            (Some(lhs), Some(rhs)) if lhs != rhs => rhs.cmp(&lhs),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            _ => Ordering::Equal,
        },
        // `bottle` compares the bottle name, then the container path.
        Sort::Bottle => {
            match natural_cmp(bottle_name(first_game, bottles), bottle_name(second_game, bottles)) {
                Ordering::Equal => first_game.container.cmp(&second_game.container),
                ordering => ordering,
            }
        }
        Sort::Name => Ordering::Equal,
    };
    if primary != Ordering::Equal {
        return primary;
    }
    match natural_cmp(&first_game.title, &second_game.title) {
        Ordering::Equal => match first_game.container.cmp(&second_game.container) {
            Ordering::Equal => first_game.id.cmp(&second_game.id),
            ordering => ordering,
        },
        ordering => ordering,
    }
}

fn bottle_name<'a>(game: &Game, bottles: &'a [BottleVm]) -> &'a str {
    game.bottle
        .checked_sub(1)
        .and_then(|index| bottles.get(index))
        .map_or("", |bottle| bottle.name.as_str())
}

/// Finder-style comparison: case-insensitive and numeric-aware. The Swift app
/// uses `localizedStandardCompare`; for the ASCII and CJK titles in the
/// library this matches it, with the documented difference that collation is
/// by scalar value rather than the user's locale.
pub(crate) fn natural_cmp(first: &str, second: &str) -> Ordering {
    let first: Vec<char> = first.to_lowercase().chars().collect();
    let second: Vec<char> = second.to_lowercase().chars().collect();
    let (mut left, mut right) = (0, 0);
    while left < first.len() && right < second.len() {
        if first[left].is_ascii_digit() && second[right].is_ascii_digit() {
            let first_start = left;
            while left < first.len() && first[left].is_ascii_digit() {
                left += 1;
            }
            let second_start = right;
            while right < second.len() && second[right].is_ascii_digit() {
                right += 1;
            }
            let first_number: String = first[first_start..left].iter().collect();
            let second_number: String = second[second_start..right].iter().collect();
            let first_trimmed = first_number.trim_start_matches('0');
            let second_trimmed = second_number.trim_start_matches('0');
            let ordering = first_trimmed
                .len()
                .cmp(&second_trimmed.len())
                .then_with(|| first_trimmed.cmp(second_trimmed))
                .then_with(|| first_number.len().cmp(&second_number.len()));
            if ordering != Ordering::Equal {
                return ordering;
            }
            continue;
        }
        match first[left].cmp(&second[right]) {
            Ordering::Equal => {
                left += 1;
                right += 1;
            }
            ordering => return ordering,
        }
    }
    first.len().cmp(&second.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bottle(name: &str) -> BottleVm {
        BottleVm {
            path: format!("/bottles/{name}"),
            name: name.into(),
            available: true,
        }
    }

    fn game(
        id: &str,
        title: &str,
        studio: &str,
        release: &str,
        slot: usize,
        container: &str,
    ) -> Game {
        Game {
            metadata: None,
            character_images: Default::default(),
            id: id.into(),
            identity: id.into(),
            art: Some(std::path::PathBuf::from(format!("/covers/{title}.jpg"))),
            title: title.into(),
            studio: (!studio.is_empty()).then(|| studio.into()),
            description: None,
            characters: Vec::new(),
            metadata_sources: Default::default(),
            release: (!release.is_empty()).then(|| release.into()),
            landscape: false,
            bottle: slot,
            hidden: false,
            rating: None,
            play_time: None,
            daily_play_time: Default::default(),
            last_played: None,
            executable: None,
            folder_path: None,
            game_directory: None,
            is_folder: false,
            needs_executable: false,
            container: container.into(),
            last_session_ended_at: None,
            is_launcher: false,
            tags: Vec::new(),
            multi_executable: false,
            launch_executables: Vec::new(),
            last_executable: None,
        }
    }

    #[test]
    fn aggregation_tags_two_bottles_and_only_de_duplicates_identical_cards() {
        let bottles = [bottle("galgame"), bottle("galgame-win7")];
        let same_art = std::path::PathBuf::from("/covers/Same.jpg");
        let mut first = game("steam:10", "Same", "Key", "2000-01-01", 1, &bottles[0].path);
        first.art = Some(same_art.clone());
        let unique = game("steam:11", "Unique", "", "", 1, &bottles[0].path);
        let mut duplicate = game("steam:10", "Same", "Key", "2000-01-01", 2, &bottles[1].path);
        duplicate.art = Some(same_art.clone());
        let mut separate = game("steam:10", "Same", "Key", "2000-01-01", 2, &bottles[1].path);
        separate.art = Some(std::path::PathBuf::from("/covers/Other.jpg"));

        let mut caches = BTreeMap::new();
        caches.insert(bottles[0].path.clone(), vec![first.clone(), unique.clone()]);
        caches.insert(bottles[1].path.clone(), vec![duplicate, separate.clone()]);

        let merged = merge_bottles(&bottles, &caches);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].title, "Same");
        assert_eq!(merged[0].container, bottles[0].path, "first bottle wins");
        assert_eq!(merged[1].title, "Unique");
        assert_eq!(merged[1].container, bottles[0].path);
        // Different artwork means a genuinely separate install: both rows stay.
        assert_eq!(merged[2].container, bottles[1].path);

        // Rebuilding from the same caches is idempotent (no duplicate cards).
        let refreshed = merge_bottles(&bottles, &caches);
        assert_eq!(refreshed.len(), merged.len());
        assert_eq!(
            refreshed
                .iter()
                .map(|game| (&game.id, &game.container))
                .collect::<Vec<_>>(),
            merged
                .iter()
                .map(|game| (&game.id, &game.container))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_same_steam_game_in_two_bottles_shows_once_even_though_ids_and_paths_differ() {
        // Real ids embed the bottle path (`steam:<path>:<appID>`), and Steam's
        // artwork lives inside each bottle, so only the bottle-relative path
        // identifies the same cover.
        let bottles = [bottle("galgame"), bottle("galgame-win7")];
        let mut first = game(
            &format!("steam:{}/drive_c/Steam:10", bottles[0].path),
            "Same Game",
            "Key",
            "2000-01-01",
            1,
            &bottles[0].path,
        );
        first.art = Some(std::path::PathBuf::from(format!(
            "{}/drive_c/Steam/appcache/librarycache/10/library_600x900.jpg",
            bottles[0].path
        )));
        let mut second = game(
            &format!("steam:{}/drive_c/Steam:10", bottles[1].path),
            "Same Game",
            "Key",
            "2000-01-01",
            2,
            &bottles[1].path,
        );
        second.art = Some(std::path::PathBuf::from(format!(
            "{}/drive_c/Steam/appcache/librarycache/10/library_600x900.jpg",
            bottles[1].path
        )));

        let mut caches = BTreeMap::new();
        caches.insert(bottles[0].path.clone(), vec![first]);
        caches.insert(bottles[1].path.clone(), vec![second]);

        let merged = merge_bottles(&bottles, &caches);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].container, bottles[0].path);
    }

    #[test]
    fn an_entry_belongs_to_the_bottle_that_reported_it() {
        let bottles = [bottle("one"), bottle("two")];
        let mut caches = BTreeMap::new();
        caches.insert(
            bottles[1].path.clone(),
            vec![game("pinned:/a", "A", "", "", 2, &bottles[1].path)],
        );
        let merged = merge_bottles(&bottles, &caches);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].bottle, 2);
        assert_eq!(merged[0].container, bottles[1].path);
    }

    #[test]
    fn search_matches_name_developer_and_year_for_cjk_and_ascii() {
        let spring = game("a", "春之回声", "Sample Studio", "2004-07-23", 1, "/b");
        let air = game("b", "Northwind Signal", "Folded Atlas", "2000-09-08", 1, "/b");

        assert!(matches_query(&spring, " 春之回声 "));
        assert!(matches_query(&spring, "SAMPLE STUDIO"));
        assert!(matches_query(&spring, "2004"));
        assert!(matches_query(&spring, "  200 "));

        assert!(matches_query(&air, "northwind"));
        assert!(matches_query(&air, "folded atlas"));
        assert!(matches_query(&air, "2000"));

        // An empty or whitespace-only query matches everything.
        assert!(matches_query(&air, ""));
        assert!(matches_query(&air, "   "));

        assert!(!matches_query(&air, "sample studio"));
        assert!(!matches_query(&spring, "northwind"));
        assert!(!matches_query(&spring, "1999"));
    }

    #[test]
    fn hidden_entries_leave_the_grid_and_reappear_under_the_filter() {
        let games = [
            game("a", "Visible", "", "", 1, "/b"),
            game("b", "Hidden", "", "", 1, "/b"),
        ];
        let mut hidden = games[1].clone();
        hidden.hidden = true;
        let games = [games[0].clone(), hidden];

        assert_eq!(filter_indices(&games, "", 1, false), vec![0]);
        assert_eq!(filter_indices(&games, "", 1, true), vec![0, 1]);
        // The whole-library scope sees the same hidden state.
        assert_eq!(filter_indices(&games, "", 0, false), vec![0]);
        // A bottle scope still applies the slot filter.
        assert!(filter_indices(&games, "", 2, true).is_empty());
    }

    fn sorted(games: &[Game], sort: Sort, bottles: &[BottleVm]) -> Vec<String> {
        let mut indices: Vec<usize> = (0..games.len()).collect();
        sort_indices(games, &mut indices, sort, bottles, true);
        indices
            .into_iter()
            .map(|index| games[index].title.clone())
            .collect()
    }

    #[test]
    fn launchers_sink_below_games_unless_grouping_is_off() {
        let bottles = [bottle("galgame")];
        let mut steam = game("a", "Steam", "", "", 1, &bottles[0].path);
        steam.is_launcher = true;
        // The most recent session belongs to the launcher: grouping still
        // wins, exactly like `LibraryOrdering.sorted` with `groupsLaunchers`.
        steam.last_session_ended_at = Some(900);
        let game = game("b", "Game", "", "", 1, &bottles[0].path);

        let games = [steam, game];
        assert_eq!(
            sorted(&games, Sort::Recent, &bottles),
            ["Game", "Steam"]
        );

        let mut indices: Vec<usize> = (0..games.len()).collect();
        sort_indices(&games, &mut indices, Sort::Recent, &bottles, false);
        let titles: Vec<_> = indices
            .into_iter()
            .map(|index| games[index].title.clone())
            .collect();
        assert_eq!(titles, ["Steam", "Game"], "timeline keeps the raw order");
    }

    #[test]
    fn recent_orders_by_confirmed_session_then_name() {
        let bottles = [bottle("galgame")];
        let mut alpha = game("a", "Alpha", "", "", 1, &bottles[0].path);
        alpha.last_session_ended_at = Some(100);
        let mut beta = game("b", "Beta", "", "", 1, &bottles[0].path);
        beta.last_session_ended_at = Some(300);
        let delta = game("c", "Delta", "", "", 1, &bottles[0].path);
        let charlie = game("d", "Charlie", "", "", 1, &bottles[0].path);

        let games = [alpha, beta, delta, charlie];
        assert_eq!(
            sorted(&games, Sort::Recent, &bottles),
            ["Beta", "Alpha", "Charlie", "Delta"]
        );
    }

    #[test]
    fn name_is_case_insensitive_and_numeric_aware() {
        let bottles = [bottle("galgame")];
        let games = [
            game("a", "Game 10", "", "", 1, &bottles[0].path),
            game("b", "game 2", "", "", 1, &bottles[0].path),
            game("c", "Game 1", "", "", 1, &bottles[0].path),
        ];
        assert_eq!(
            sorted(&games, Sort::Name, &bottles),
            ["Game 1", "game 2", "Game 10"]
        );
        assert_eq!(natural_cmp("Sample", "sample"), Ordering::Equal);
        assert_eq!(natural_cmp("a", "B"), Ordering::Less);
    }

    #[test]
    fn bottle_sorts_by_bottle_name_then_entry_name() {
        let bottles = [bottle("alpha"), bottle("beta")];
        let games = [
            game("a", "Zeta", "", "", 1, &bottles[0].path),
            game("b", "Alpha", "", "", 2, &bottles[1].path),
            game("c", "Beta", "", "", 1, &bottles[0].path),
        ];
        assert_eq!(
            sorted(&games, Sort::Bottle, &bottles),
            ["Beta", "Zeta", "Alpha"]
        );
    }

    #[test]
    fn clicking_the_current_star_clears_the_rating() {
        assert_eq!(next_rating(None, 3), Some(3));
        assert_eq!(next_rating(Some(2), 3), Some(3));
        assert_eq!(next_rating(Some(3), 3), None);
        // The panel only offers 1..=5.
        assert_eq!(next_rating(Some(5), 1), Some(1));
    }

    #[test]
    fn sort_indices_map_to_the_swift_option_order() {
        // The toolbar stores indices in `LibrarySort` declaration order:
        // recent (0), name (1), bottle (2).
        assert_eq!(Sort::from_index(0), Sort::Recent);
        assert_eq!(Sort::from_index(1), Sort::Name);
        assert_eq!(Sort::from_index(2), Sort::Bottle);
        assert_eq!(Sort::from_index(99), Sort::Recent);
    }

    #[test]
    fn normalize_tag_and_list_enforce_dedup_and_bounds() {
        assert_eq!(normalize_tag("   "), None);
        assert_eq!(normalize_tag("bad\0tag"), None);
        assert_eq!(normalize_tag("  Visual   Novel  "), Some("Visual Novel".into()));
        let long = "a".repeat(50);
        assert_eq!(normalize_tag(&long).unwrap().chars().count(), MAX_TAG_LENGTH);

        let normalized = normalize_tags(["RPG", "rpg", "  Visual   Novel ", "", "Action"]);
        assert_eq!(normalized, vec!["RPG", "Visual Novel", "Action"]);

        let many: Vec<String> = (1..=45).map(|i| format!("Tag{i}")).collect();
        assert_eq!(normalize_tags(many).len(), MAX_TAGS_PER_ENTRY);
    }

    #[test]
    fn tag_filter_single_multi_untagged_and_search_match_tags() {
        let bottles = [bottle("main")];
        let mut g1 = game("g1", "Alpha", "Key", "2020-01-01", 1, &bottles[0].path);
        g1.tags = vec!["RPG".into(), "日系".into()];
        let mut g2 = game("g2", "Beta", "Nitro", "2021-01-01", 1, &bottles[0].path);
        g2.tags = vec!["rpg".into(), "ADV".into()];
        let g3 = game("g3", "Gamma", "", "", 1, &bottles[0].path);
        let games = vec![g1, g2, g3];

        // Search query matches tag names too.
        assert!(matches_query(&games[0], "日系"));
        assert!(!matches_query(&games[2], "日系"));

        // Collect tag counts merges case-insensitively and sorts by count desc then name.
        let counts = collect_tag_counts(&games, true);
        assert_eq!(counts[0], ("RPG".to_owned(), 2));
        assert_eq!(untagged_count(&games, true), 1);

        let mut filter = TagFilter::All;
        filter.toggle_single("RPG");
        assert_eq!(filter_indices_with_tags(&games, "", &filter, 0, true), vec![0, 1]);

        filter.toggle_multi("ADV");
        assert_eq!(filter_indices_with_tags(&games, "", &filter, 0, true), vec![1]);

        filter.toggle_single("RPG"); // clicking active single filter returns to All?
        // First toggle_single sets single "RPG", second clears to All.
        assert_eq!(filter, TagFilter::Tags(vec!["RPG".into()]));
        filter.toggle_single("RPG");
        assert_eq!(filter, TagFilter::All);

        assert_eq!(
            filter_indices_with_tags(&games, "", &TagFilter::Untagged, 0, true),
            vec![2]
        );
    }

    #[test]
    fn studio_normalization_counts_and_clustering_order() {
        let bottles = [bottle("alpha"), bottle("beta")];
        let mut g0 = game("g0", "Clannad", "  Key  ", "2004-04-28", 1, &bottles[0].path);
        g0.tags = vec!["催泪".into()];
        let g1 = game("g1", "Air", "key", "2000-09-08", 1, &bottles[0].path);
        let mut g2 = game("g2", "Saya", "Nitroplus", "2003-12-26", 2, &bottles[1].path);
        g2.tags = vec!["催泪".into()];
        let g3 = game("g3", "NoStudio", "", "", 1, &bottles[0].path);
        let mut hidden = game("g4", "HiddenKey", "Key", "", 1, &bottles[0].path);
        hidden.hidden = true;
        let games = vec![g0, g1, g2, g3, hidden];

        assert_eq!(normalize_studio("  Type   Moon \n"), Some("Type Moon".into()));
        assert_eq!(normalize_studio("   "), None);

        let counts_all = collect_studio_counts(&games, 0, false);
        assert_eq!(counts_all, vec![("Key".into(), 2), ("Nitroplus".into(), 1)]);
        assert_eq!(unknown_studio_count(&games, 0, false), 1);

        let counts_hidden = collect_studio_counts(&games, 1, true);
        assert_eq!(counts_hidden, vec![("Key".into(), 3)]);

        // Clustering keeps relative order inside clusters and puts unknown last.
        let clusters = group_indices_by_studio(&games, &[3, 2, 0, 1]);
        assert_eq!(clusters.len(), 3);
        assert_eq!(clusters[0].label, "Key");
        assert_eq!(clusters[0].indices, vec![0, 1]);
        assert_eq!(clusters[1].label, "Nitroplus");
        assert_eq!(clusters[1].indices, vec![2]);
        assert!(clusters[2].is_unknown);
        assert_eq!(clusters[2].label, UNKNOWN_STUDIO_LABEL);
        assert_eq!(clusters[2].indices, vec![3]);

        // Filtering composes tag_filter + studio_filter + query cleanly.
        let tag_filter = TagFilter::Tags(vec!["催泪".into()]);
        let mut studio_filter = StudioFilter::All;
        studio_filter.toggle_studio("KEY");
        assert_eq!(
            filter_indices_with_filters(&games, "", &tag_filter, &studio_filter, 0, false),
            vec![0]
        );
        assert_eq!(
            filter_indices_with_filters(&games, "", &TagFilter::All, &StudioFilter::Unknown, 0, false),
            vec![3]
        );
    }

    #[test]
    fn pull_reveal_ignores_mid_page_momentum_and_opens_on_second_pull_at_top() {
        let mut pull = PullRevealState::default();
        let t0 = Instant::now();

        // First gesture: scrolling up from mid-page to reach the top does NOT open.
        let r1 = pull.on_wheel(35.0, -120.0, PullWheelPhase::Started, t0, false);
        assert!(!r1.opened);
        let r2 = pull.on_wheel(50.0, 0.0, PullWheelPhase::Moved, t0 + Duration::from_millis(16), false);
        assert!(!r2.opened);
        assert!(!pull.is_open());
        let r3 = pull.on_wheel(0.0, 0.0, PullWheelPhase::Ended, t0 + Duration::from_millis(32), false);
        assert!(!r3.opened);
        // Trackpad momentum packets right after Ended stay blocked.
        let r4 = pull.on_wheel(60.0, 0.0, PullWheelPhase::Moved, t0 + Duration::from_millis(48), false);
        assert!(!r4.opened);
        assert!(!pull.is_open());

        // Second gesture starting while already at top arms and pulls the drawer open.
        let t1 = t0 + Duration::from_millis(120);
        pull.on_wheel(0.0, 0.0, PullWheelPhase::Started, t1, false);
        let partial = pull.on_wheel(20.0, 0.0, PullWheelPhase::Moved, t1 + Duration::from_millis(16), false);
        assert!(partial.consume && !partial.opened);
        assert!((pull.pull_fraction() - 0.5).abs() < 0.01);

        let opened = pull.on_wheel(24.0, 0.0, PullWheelPhase::Moved, t1 + Duration::from_millis(32), false);
        assert!(opened.opened && opened.consume);
        assert!(pull.is_open());

        // Scrolling down closes when no filter is active, but stays open while filtering.
        let keep = pull.on_wheel(-45.0, 0.0, PullWheelPhase::Moved, t1 + Duration::from_millis(90), true);
        assert!(!keep.closed);
        assert!(pull.is_open());

        let close = pull.on_wheel(-45.0, 0.0, PullWheelPhase::Moved, t1 + Duration::from_millis(120), false);
        assert!(close.closed && !close.consume);
        assert!(!pull.is_open());
    }

    #[test]
    fn pull_reveal_mouse_wheel_pause_arms_and_idle_resets() {
        let mut pull = PullRevealState::default();
        let t0 = Instant::now();

        // Discrete wheel reaching top blocks immediate overscroll packets.
        pull.on_wheel(25.0, -40.0, PullWheelPhase::Moved, t0, false);
        pull.on_wheel(50.0, 0.0, PullWheelPhase::Moved, t0 + Duration::from_millis(50), false);
        assert!(!pull.is_open());

        // After pausing >= PULL_GESTURE_PAUSE_MS at top, discrete wheel arms on second pull.
        let t1 = t0 + Duration::from_millis(50 + PULL_GESTURE_PAUSE_MS + 40);
        let notch1 = pull.on_wheel(16.0, 0.0, PullWheelPhase::Moved, t1, false);
        assert!(notch1.needs_idle_reset && !notch1.opened);
        assert!(pull.pull_fraction() > 0.0);

        // Idle reset collapses half-pulled state cleanly; otherwise consecutive notches open it.
        assert!(pull.reset_idle_pull());
        assert_eq!(pull.pull_fraction(), 0.0);

        let t2 = t1 + Duration::from_millis(400);
        pull.on_wheel(22.0, 0.0, PullWheelPhase::Moved, t2, false);
        let notch2 = pull.on_wheel(22.0, 0.0, PullWheelPhase::Moved, t2 + Duration::from_millis(40), false);
        assert!(notch2.opened);
        assert!(pull.is_open());
    }
}
