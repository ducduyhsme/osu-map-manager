//! Beginner-friendly collection filters and osu! search syntax parser.
//!
//! Instead of free-form `field<operator>value` rows, the UI offers one
//! control per idea: text boxes for words (artist, title, mapper, ...),
//! min/max sliders for numbers (stars, AR, ...), text boxes for the song
//! length, and a dropdown for the game mode.
//!
//! In addition, users can type directly in the "osu! search text" box
//! using full osu! wiki beatmap search syntax (e.g. `artist="LiSA" stars>=5.5 length<=2:30 mode=osu status=ranked`).

use serde::{Deserialize, Serialize};

use crate::local::LocalBeatmap;

/// Full slider bounds for every numeric filter, shared by the UI and matching.
pub const STARS_RANGE: (f32, f32) = (0.0, 12.0);
pub const AR_RANGE: (f32, f32) = (0.0, 11.0);
pub const CS_RANGE: (f32, f32) = (0.0, 10.0);
pub const OD_RANGE: (f32, f32) = (0.0, 11.0);
pub const HP_RANGE: (f32, f32) = (0.0, 10.0);
pub const BPM_RANGE: (f32, f32) = (0.0, 350.0);

/// Closed numeric range picked with min/max sliders. Disabled means "any".
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RangeFilter {
    pub enabled: bool,
    pub min: f32,
    pub max: f32,
}

impl RangeFilter {
    pub const fn new(min: f32, max: f32) -> Self {
        Self {
            enabled: false,
            min,
            max,
        }
    }

    pub fn matches(&self, value: Option<f32>) -> bool {
        if !self.enabled {
            return true;
        }
        value.is_some_and(|value| value >= self.min && value <= self.max)
    }

    /// osu!web-style tokens (`stars>=6 stars<=8`). Only emitted when enabled
    /// and narrowed: a full-range filter constrains nothing worth showing.
    fn tokens(&self, key: &str, full: (f32, f32)) -> Vec<String> {
        if !self.enabled {
            return Vec::new();
        }
        if self.min <= full.0 && self.max >= full.1 {
            return Vec::new();
        }
        let mut tokens = Vec::new();
        if self.min > full.0 {
            tokens.push(format!("{key}>={}", trim_number(self.min)));
        }
        if self.max < full.1 {
            tokens.push(format!("{key}<={}", trim_number(self.max)));
        }
        tokens
    }
}

fn trim_number(value: f32) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Game mode picked from a dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ModeFilter {
    /// osu!standard (also the default, matching previous behaviour).
    #[default]
    Osu,
    Any,
    Taiko,
    Catch,
    Mania,
}

impl ModeFilter {
    pub const ALL: [Self; 5] = [Self::Osu, Self::Any, Self::Taiko, Self::Catch, Self::Mania];

    pub fn label(self) -> &'static str {
        match self {
            Self::Osu => "osu! (standard)",
            Self::Any => "Any mode",
            Self::Taiko => "taiko",
            Self::Catch => "catch",
            Self::Mania => "mania",
        }
    }

    pub fn token(self) -> Option<&'static str> {
        match self {
            Self::Osu => Some("osu"),
            Self::Any => None,
            Self::Taiko => Some("taiko"),
            Self::Catch => Some("catch"),
            Self::Mania => Some("mania"),
        }
    }

    pub fn matches(self, mode: Option<u8>) -> bool {
        match self {
            Self::Any => true,
            // A missing Mode field means osu!std in the .osu format.
            Self::Osu => mode.unwrap_or(0) == 0,
            Self::Taiko => mode == Some(1),
            Self::Catch => mode == Some(2),
            Self::Mania => mode == Some(3),
        }
    }
}

/// Comparison operators supported by osu! wiki search syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    Equal,              // = or == or :
    NotEqual,           // !=
    GreaterThan,        // >
    GreaterThanOrEqual, // >=
    LessThan,           // <
    LessThanOrEqual,    // <=
}

impl ComparisonOp {
    pub fn compare_f32(self, actual: f32, target: f32) -> bool {
        match self {
            Self::Equal => (actual - target).abs() < 0.05,
            Self::NotEqual => (actual - target).abs() >= 0.05,
            Self::GreaterThan => actual > target,
            Self::GreaterThanOrEqual => actual >= target - 0.001,
            Self::LessThan => actual < target,
            Self::LessThanOrEqual => actual <= target + 0.001,
        }
    }

    pub fn compare_str(self, haystack: &str, needle_lower: &str) -> bool {
        let contains = needle_lower.is_empty() || haystack.to_lowercase().contains(needle_lower);
        match self {
            Self::Equal => contains,
            Self::NotEqual => !contains,
            _ => contains,
        }
    }
}

/// Numeric fields available for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericField {
    Stars,
    Ar,
    Cs,
    Od,
    Hp,
    Bpm,
    Length,
    Circles,
    Sliders,
    Keys,
}

/// Text fields available for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextField {
    Artist,
    Title,
    Creator,
    Difficulty,
    Source,
    Tag,
}

/// Ranked status targets supported by osu! wiki search syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTarget {
    Ranked,
    Approved,
    Qualified,
    Loved,
    Pending,
    Graveyard,
    Specific(u8),
}

impl StatusTarget {
    pub fn matches(&self, actual: u8) -> bool {
        match self {
            Self::Ranked => actual == 4 || actual == 5,
            Self::Approved => actual == 5,
            Self::Qualified => actual == 6,
            Self::Loved => actual == 7,
            Self::Pending => actual == 2,
            Self::Graveyard => actual == 2,
            Self::Specific(s) => actual == *s,
        }
    }
}

/// Single parsed search filter clause.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryClause {
    Numeric {
        field: NumericField,
        op: ComparisonOp,
        value: f32,
    },
    Text {
        field: TextField,
        op: ComparisonOp,
        value: String,
    },
    Mode {
        op: ComparisonOp,
        target_mode: u8,
    },
    Status {
        op: ComparisonOp,
        target: StatusTarget,
    },
    Unkeyed {
        negated: bool,
        text: String,
        numeric_id: Option<i64>,
    },
}

impl QueryClause {
    pub fn matches(&self, map: &LocalBeatmap) -> bool {
        match self {
            Self::Numeric { field, op, value } => {
                let actual = match field {
                    NumericField::Stars => map.stars,
                    NumericField::Ar => map.ar,
                    NumericField::Cs => map.cs,
                    NumericField::Od => map.od,
                    NumericField::Hp => map.hp,
                    NumericField::Bpm => map.bpm,
                    NumericField::Length => map.length_seconds,
                    NumericField::Circles => Some(map.circles as f32),
                    NumericField::Sliders => Some(map.sliders as f32),
                    NumericField::Keys => {
                        if map.mode == Some(3) {
                            map.cs
                        } else {
                            None
                        }
                    }
                };
                let Some(actual) = actual else {
                    return false;
                };
                op.compare_f32(actual, *value)
            }
            Self::Text { field, op, value } => {
                let haystack = match field {
                    TextField::Artist => &map.artist,
                    TextField::Title => &map.title,
                    TextField::Creator => &map.creator,
                    TextField::Difficulty => &map.version,
                    TextField::Source => &map.source,
                    TextField::Tag => &map.tags,
                };
                op.compare_str(haystack, value)
            }
            Self::Mode { op, target_mode } => {
                let actual = map.mode.unwrap_or(0);
                match op {
                    ComparisonOp::Equal => actual == *target_mode,
                    ComparisonOp::NotEqual => actual != *target_mode,
                    _ => actual == *target_mode,
                }
            }
            Self::Status { op, target } => {
                let Some(actual) = map.ranked_status else {
                    return false;
                };
                let matches = target.matches(actual);
                match op {
                    ComparisonOp::Equal => matches,
                    ComparisonOp::NotEqual => !matches,
                    _ => matches,
                }
            }
            Self::Unkeyed {
                negated,
                text,
                numeric_id,
            } => {
                let text_match = contains_lowered(&map.artist, text)
                    || contains_lowered(&map.title, text)
                    || contains_lowered(&map.creator, text)
                    || contains_lowered(&map.version, text)
                    || contains_lowered(&map.source, text)
                    || contains_lowered(&map.tags, text);

                let id_match = numeric_id.is_some_and(|id| {
                    map.beatmap_id == Some(id) || map.beatmapset_id == Some(id)
                });

                let positive_match = text_match || id_match;
                if *negated {
                    !positive_match
                } else {
                    positive_match
                }
            }
        }
    }
}

/// Fully parsed osu! search query with all its clauses.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedOsuQuery {
    pub raw: String,
    pub clauses: Vec<QueryClause>,
}

impl ParsedOsuQuery {
    pub fn is_empty(&self) -> bool {
        self.clauses.is_empty()
    }

    pub fn matches(&self, map: &LocalBeatmap) -> bool {
        for clause in &self.clauses {
            if !clause.matches(map) {
                return false;
            }
        }
        true
    }
}

/// Tokenizes a query string, keeping quoted parts intact.
pub fn tokenize_query(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut current = String::new();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\\' if in_quotes => {
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' {
                        current.push(next);
                        chars.next();
                        continue;
                    }
                }
                current.push('\\');
            }
            '"' => {
                in_quotes = !in_quotes;
            }
            c if c.is_whitespace() && !in_quotes => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    tokens.push(trimmed.to_owned());
                }
                current.clear();
            }
            c => {
                current.push(c);
            }
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        tokens.push(trimmed.to_owned());
    }
    tokens
}

fn split_key_op_val(token: &str) -> Option<(&str, ComparisonOp, &str)> {
    let ops = [
        (">=", ComparisonOp::GreaterThanOrEqual),
        ("<=", ComparisonOp::LessThanOrEqual),
        ("!=", ComparisonOp::NotEqual),
        ("==", ComparisonOp::Equal),
        (">", ComparisonOp::GreaterThan),
        ("<", ComparisonOp::LessThan),
        ("=", ComparisonOp::Equal),
        (":", ComparisonOp::Equal),
    ];
    for (op_str, op) in ops {
        if let Some(pos) = token.find(op_str) {
            let key = &token[..pos];
            if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                let val = &token[pos + op_str.len()..];
                return Some((key, op, val));
            }
        }
    }
    None
}

/// Parses a number or mm:ss time string into seconds.
pub fn parse_time_or_number(s: &str) -> Option<f32> {
    let s = s.trim().trim_matches('"');
    if s.contains(':') {
        let mut parts = s.split(':');
        let minutes = parts.next()?.parse::<f32>().ok()?;
        let seconds = parts.next()?.parse::<f32>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(minutes * 60.0 + seconds)
    } else {
        s.parse::<f32>().ok().filter(|v| v.is_finite())
    }
}

/// Parses an entire search string into a `ParsedOsuQuery`.
pub fn parse_osu_query(input: &str) -> ParsedOsuQuery {
    let tokens = tokenize_query(input);
    let mut clauses = Vec::new();
    for token in tokens {
        if let Some((key, op, val)) = split_key_op_val(&token) {
            let key_lower = key.to_ascii_lowercase();
            let val_clean = val.trim_matches('"');
            match key_lower.as_str() {
                "stars" | "sr" | "star" => {
                    if let Some(num) = parse_time_or_number(val_clean) {
                        clauses.push(QueryClause::Numeric {
                            field: NumericField::Stars,
                            op,
                            value: num,
                        });
                    }
                }
                "ar" => {
                    if let Some(num) = parse_time_or_number(val_clean) {
                        clauses.push(QueryClause::Numeric {
                            field: NumericField::Ar,
                            op,
                            value: num,
                        });
                    }
                }
                "cs" => {
                    if let Some(num) = parse_time_or_number(val_clean) {
                        clauses.push(QueryClause::Numeric {
                            field: NumericField::Cs,
                            op,
                            value: num,
                        });
                    }
                }
                "od" => {
                    if let Some(num) = parse_time_or_number(val_clean) {
                        clauses.push(QueryClause::Numeric {
                            field: NumericField::Od,
                            op,
                            value: num,
                        });
                    }
                }
                "hp" | "dr" => {
                    if let Some(num) = parse_time_or_number(val_clean) {
                        clauses.push(QueryClause::Numeric {
                            field: NumericField::Hp,
                            op,
                            value: num,
                        });
                    }
                }
                "bpm" => {
                    if let Some(num) = parse_time_or_number(val_clean) {
                        clauses.push(QueryClause::Numeric {
                            field: NumericField::Bpm,
                            op,
                            value: num,
                        });
                    }
                }
                "length" | "drain" | "len" | "time" => {
                    if let Some(num) = parse_time_or_number(val_clean) {
                        clauses.push(QueryClause::Numeric {
                            field: NumericField::Length,
                            op,
                            value: num,
                        });
                    }
                }
                "circles" | "circle" => {
                    if let Some(num) = parse_time_or_number(val_clean) {
                        clauses.push(QueryClause::Numeric {
                            field: NumericField::Circles,
                            op,
                            value: num,
                        });
                    }
                }
                "sliders" | "slider" => {
                    if let Some(num) = parse_time_or_number(val_clean) {
                        clauses.push(QueryClause::Numeric {
                            field: NumericField::Sliders,
                            op,
                            value: num,
                        });
                    }
                }
                "keys" | "key" => {
                    if let Some(num) = parse_time_or_number(val_clean) {
                        clauses.push(QueryClause::Numeric {
                            field: NumericField::Keys,
                            op,
                            value: num,
                        });
                    }
                }
                "artist" => {
                    clauses.push(QueryClause::Text {
                        field: TextField::Artist,
                        op,
                        value: val_clean.to_lowercase(),
                    });
                }
                "title" => {
                    clauses.push(QueryClause::Text {
                        field: TextField::Title,
                        op,
                        value: val_clean.to_lowercase(),
                    });
                }
                "creator" | "mapper" | "author" => {
                    clauses.push(QueryClause::Text {
                        field: TextField::Creator,
                        op,
                        value: val_clean.to_lowercase(),
                    });
                }
                "diff" | "difficulty" | "version" => {
                    clauses.push(QueryClause::Text {
                        field: TextField::Difficulty,
                        op,
                        value: val_clean.to_lowercase(),
                    });
                }
                "source" => {
                    clauses.push(QueryClause::Text {
                        field: TextField::Source,
                        op,
                        value: val_clean.to_lowercase(),
                    });
                }
                "tag" | "tags" => {
                    clauses.push(QueryClause::Text {
                        field: TextField::Tag,
                        op,
                        value: val_clean.to_lowercase(),
                    });
                }
                "mode" | "m" => {
                    let target_mode = match val_clean.to_lowercase().as_str() {
                        "osu" | "std" | "standard" | "o" | "0" => Some(0),
                        "taiko" | "t" | "1" => Some(1),
                        "catch" | "ctb" | "fruits" | "c" | "2" => Some(2),
                        "mania" | "m" | "3" => Some(3),
                        _ => None,
                    };
                    if let Some(m) = target_mode {
                        clauses.push(QueryClause::Mode {
                            op,
                            target_mode: m,
                        });
                    }
                }
                "status" => {
                    let target = match val_clean.to_lowercase().as_str() {
                        "ranked" | "r" => Some(StatusTarget::Ranked),
                        "approved" | "a" => Some(StatusTarget::Approved),
                        "qualified" | "q" => Some(StatusTarget::Qualified),
                        "loved" | "l" => Some(StatusTarget::Loved),
                        "pending" | "w" | "wip" => Some(StatusTarget::Pending),
                        "graveyard" | "g" => Some(StatusTarget::Graveyard),
                        s => s.parse::<u8>().ok().map(StatusTarget::Specific),
                    };
                    if let Some(target) = target {
                        clauses.push(QueryClause::Status { op, target });
                    }
                }
                _ => {
                    let (negated, text) = if token.starts_with('-') && token.len() > 1 {
                        (true, &token[1..])
                    } else {
                        (false, token.as_str())
                    };
                    let text = text.trim_matches('"');
                    let numeric_id = text.parse::<i64>().ok();
                    clauses.push(QueryClause::Unkeyed {
                        negated,
                        text: text.to_lowercase(),
                        numeric_id,
                    });
                }
            }
        } else {
            let (negated, text) = if token.starts_with('-') && token.len() > 1 {
                (true, &token[1..])
            } else {
                (false, token.as_str())
            };
            let text = text.trim_matches('"');
            let numeric_id = text.parse::<i64>().ok();
            clauses.push(QueryClause::Unkeyed {
                negated,
                text: text.to_lowercase(),
                numeric_id,
            });
        }
    }
    ParsedOsuQuery {
        raw: input.to_owned(),
        clauses,
    }
}

/// All collection filters. Text is matched case-insensitively when non-empty;
/// the song length bounds are typed in seconds and ignored when blank or
/// invalid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatmapFilters {
    pub artist: String,
    pub title: String,
    pub mapper: String,
    pub difficulty: String,
    pub tag: String,
    pub length_min: String,
    pub length_max: String,
    pub stars: RangeFilter,
    pub ar: RangeFilter,
    pub cs: RangeFilter,
    pub od: RangeFilter,
    pub hp: RangeFilter,
    pub bpm: RangeFilter,
    pub mode: ModeFilter,
    #[serde(default)]
    pub query_text: String,
    #[serde(skip)]
    pub parsed_query: Option<ParsedOsuQuery>,
}

impl Default for BeatmapFilters {
    fn default() -> Self {
        Self {
            artist: String::new(),
            title: String::new(),
            mapper: String::new(),
            difficulty: String::new(),
            tag: String::new(),
            length_min: String::new(),
            length_max: String::new(),
            stars: RangeFilter::default(),
            ar: RangeFilter::default(),
            cs: RangeFilter::default(),
            od: RangeFilter::default(),
            hp: RangeFilter::default(),
            bpm: RangeFilter::default(),
            mode: ModeFilter::default(),
            query_text: String::new(),
            parsed_query: None,
        }
    }
}

impl BeatmapFilters {
    pub fn with_full_ranges() -> Self {
        let mut filters = Self {
            stars: RangeFilter::new(STARS_RANGE.0, STARS_RANGE.1),
            ar: RangeFilter::new(AR_RANGE.0, AR_RANGE.1),
            cs: RangeFilter::new(CS_RANGE.0, CS_RANGE.1),
            od: RangeFilter::new(OD_RANGE.0, OD_RANGE.1),
            hp: RangeFilter::new(HP_RANGE.0, HP_RANGE.1),
            bpm: RangeFilter::new(BPM_RANGE.0, BPM_RANGE.1),
            ..Self::default()
        };
        // Stars, AR and CS are on by default at full range.
        filters.stars.enabled = true;
        filters.ar.enabled = true;
        filters.cs.enabled = true;
        filters.query_text = filters.to_osu_search();
        filters
    }

    pub fn matches_local(&self, map: &LocalBeatmap) -> bool {
        self.matches_local_lowered(map, &self.lowered_text())
    }

    /// Lowercases the text needles once so a full-library refresh pays 5
    /// allocations total instead of 5 per map.
    pub fn lowered_text(&self) -> LoweredTextQueries {
        LoweredTextQueries {
            artist: lowered_needle(&self.artist),
            title: lowered_needle(&self.title),
            mapper: lowered_needle(&self.mapper),
            difficulty: lowered_needle(&self.difficulty),
            tag: lowered_needle(&self.tag),
        }
    }

    pub fn matches_local_lowered(&self, map: &LocalBeatmap, text: &LoweredTextQueries) -> bool {
        if let Some(parsed) = &self.parsed_query {
            return parsed.matches(map);
        }
        contains_lowered(&map.artist, &text.artist)
            && contains_lowered(&map.title, &text.title)
            && contains_lowered(&map.creator, &text.mapper)
            && contains_lowered(&map.version, &text.difficulty)
            && contains_lowered(&map.tags, &text.tag)
            && within_length(map.length_seconds, &self.length_min, &self.length_max)
            && self.stars.matches(map.stars)
            && self.ar.matches(map.ar)
            && self.cs.matches(map.cs)
            && self.od.matches(map.od)
            && self.hp.matches(map.hp)
            && self.bpm.matches(map.bpm)
            && self.mode.matches(map.mode)
    }

    /// Number of active filters, for the sidebar badge.
    pub fn active_count(&self) -> usize {
        if let Some(parsed) = &self.parsed_query {
            return if parsed.is_empty() { 0 } else { parsed.clauses.len() };
        }
        let mut count = 0;
        for text in [
            &self.artist,
            &self.title,
            &self.mapper,
            &self.difficulty,
            &self.tag,
        ] {
            if !text.trim().is_empty() {
                count += 1;
            }
        }
        for range in [
            &self.stars,
            &self.ar,
            &self.cs,
            &self.od,
            &self.hp,
            &self.bpm,
        ] {
            if range.enabled {
                count += 1;
            }
        }
        if parse_bound(&self.length_min).is_some() || parse_bound(&self.length_max).is_some() {
            count += 1;
        }
        if self.mode != ModeFilter::Any {
            count += 1;
        }
        count
    }

    pub fn clear_all(&mut self) {
        let fresh = Self::with_full_ranges();
        self.artist.clear();
        self.title.clear();
        self.mapper.clear();
        self.difficulty.clear();
        self.tag.clear();
        self.length_min.clear();
        self.length_max.clear();
        self.stars = fresh.stars;
        self.ar = fresh.ar;
        self.cs = fresh.cs;
        self.od = fresh.od;
        self.hp = fresh.hp;
        self.bpm = fresh.bpm;
        self.mode = fresh.mode;
        self.query_text = self.to_osu_search();
        self.parsed_query = None;
    }

    /// True when the length boxes are empty or parse as numbers.
    pub fn length_valid(&self) -> bool {
        let min_ok = self.length_min.trim().is_empty() || parse_bound(&self.length_min).is_some();
        let max_ok = self.length_max.trim().is_empty() || parse_bound(&self.length_max).is_some();
        min_ok && max_ok
    }

    /// osu!web-style text for the active filters, shown in the UI.
    pub fn to_osu_search(&self) -> String {
        let mut tokens = Vec::new();
        push_text_token(&mut tokens, "artist", &self.artist);
        push_text_token(&mut tokens, "title", &self.title);
        push_text_token(&mut tokens, "creator", &self.mapper);
        push_text_token(&mut tokens, "difficulty", &self.difficulty);
        push_text_token(&mut tokens, "tag", &self.tag);
        if let Some(min) = parse_bound(&self.length_min) {
            tokens.push(format!("length>={}", trim_number(min)));
        }
        if let Some(max) = parse_bound(&self.length_max) {
            tokens.push(format!("length<={}", trim_number(max)));
        }
        tokens.extend(self.stars.tokens("stars", STARS_RANGE));
        tokens.extend(self.ar.tokens("ar", AR_RANGE));
        tokens.extend(self.cs.tokens("cs", CS_RANGE));
        tokens.extend(self.od.tokens("od", OD_RANGE));
        tokens.extend(self.hp.tokens("hp", HP_RANGE));
        tokens.extend(self.bpm.tokens("bpm", BPM_RANGE));
        if let Some(mode) = self.mode.token() {
            tokens.push(format!("mode={mode}"));
        }
        tokens.join(" ")
    }

    /// Ensures query_text is initialized with the current search representation.
    pub fn init_query_text(&mut self) {
        if self.query_text.is_empty() {
            self.query_text = self.to_osu_search();
        }
    }

    /// Synchronizes UI controls (sliders, inputs) from the typed query_text.
    pub fn sync_ui_from_query_text(&mut self) {
        let trimmed = self.query_text.trim();
        if trimmed.is_empty() {
            self.parsed_query = None;
            self.clear_all();
            self.query_text.clear();
            return;
        }

        let parsed = parse_osu_query(trimmed);

        let mut stars_min = STARS_RANGE.0;
        let mut stars_max = STARS_RANGE.1;
        let mut stars_found = false;

        let mut ar_min = AR_RANGE.0;
        let mut ar_max = AR_RANGE.1;
        let mut ar_found = false;

        let mut cs_min = CS_RANGE.0;
        let mut cs_max = CS_RANGE.1;
        let mut cs_found = false;

        let mut od_min = OD_RANGE.0;
        let mut od_max = OD_RANGE.1;
        let mut od_found = false;

        let mut hp_min = HP_RANGE.0;
        let mut hp_max = HP_RANGE.1;
        let mut hp_found = false;

        let mut bpm_min = BPM_RANGE.0;
        let mut bpm_max = BPM_RANGE.1;
        let mut bpm_found = false;

        let mut len_min = None;
        let mut len_max = None;

        let mut mode = None;

        self.artist.clear();
        self.title.clear();
        self.mapper.clear();
        self.difficulty.clear();
        self.tag.clear();

        for clause in &parsed.clauses {
            match clause {
                QueryClause::Numeric { field, op, value } => {
                    let apply_range = |min: &mut f32, max: &mut f32, found: &mut bool| {
                        *found = true;
                        match op {
                            ComparisonOp::GreaterThan | ComparisonOp::GreaterThanOrEqual => {
                                *min = min.max(*value);
                            }
                            ComparisonOp::LessThan | ComparisonOp::LessThanOrEqual => {
                                *max = max.min(*value);
                            }
                            ComparisonOp::Equal => {
                                *min = *value;
                                *max = *value;
                            }
                            ComparisonOp::NotEqual => {}
                        }
                    };
                    match field {
                        NumericField::Stars => apply_range(&mut stars_min, &mut stars_max, &mut stars_found),
                        NumericField::Ar => apply_range(&mut ar_min, &mut ar_max, &mut ar_found),
                        NumericField::Cs => apply_range(&mut cs_min, &mut cs_max, &mut cs_found),
                        NumericField::Od => apply_range(&mut od_min, &mut od_max, &mut od_found),
                        NumericField::Hp => apply_range(&mut hp_min, &mut hp_max, &mut hp_found),
                        NumericField::Bpm => apply_range(&mut bpm_min, &mut bpm_max, &mut bpm_found),
                        NumericField::Length => match op {
                            ComparisonOp::GreaterThan | ComparisonOp::GreaterThanOrEqual => {
                                len_min = Some(len_min.map_or(*value, |m: f32| m.max(*value)));
                            }
                            ComparisonOp::LessThan | ComparisonOp::LessThanOrEqual => {
                                len_max = Some(len_max.map_or(*value, |m: f32| m.min(*value)));
                            }
                            ComparisonOp::Equal => {
                                len_min = Some(*value);
                                len_max = Some(*value);
                            }
                            ComparisonOp::NotEqual => {}
                        },
                        _ => {}
                    }
                }
                QueryClause::Text { field, op, value } => {
                    if *op == ComparisonOp::Equal {
                        match field {
                            TextField::Artist => self.artist = value.clone(),
                            TextField::Title => self.title = value.clone(),
                            TextField::Creator => self.mapper = value.clone(),
                            TextField::Difficulty => self.difficulty = value.clone(),
                            TextField::Tag => self.tag = value.clone(),
                            _ => {}
                        }
                    }
                }
                QueryClause::Mode { op, target_mode } => {
                    if *op == ComparisonOp::Equal {
                        mode = Some(match target_mode {
                            0 => ModeFilter::Osu,
                            1 => ModeFilter::Taiko,
                            2 => ModeFilter::Catch,
                            3 => ModeFilter::Mania,
                            _ => ModeFilter::Any,
                        });
                    }
                }
                _ => {}
            }
        }

        if stars_found {
            self.stars.enabled = true;
            self.stars.min = stars_min.clamp(STARS_RANGE.0, STARS_RANGE.1);
            self.stars.max = stars_max.clamp(STARS_RANGE.0, STARS_RANGE.1);
        } else {
            self.stars.enabled = false;
        }

        if ar_found {
            self.ar.enabled = true;
            self.ar.min = ar_min.clamp(AR_RANGE.0, AR_RANGE.1);
            self.ar.max = ar_max.clamp(AR_RANGE.0, AR_RANGE.1);
        } else {
            self.ar.enabled = false;
        }

        if cs_found {
            self.cs.enabled = true;
            self.cs.min = cs_min.clamp(CS_RANGE.0, CS_RANGE.1);
            self.cs.max = cs_max.clamp(CS_RANGE.0, CS_RANGE.1);
        } else {
            self.cs.enabled = false;
        }

        if od_found {
            self.od.enabled = true;
            self.od.min = od_min.clamp(OD_RANGE.0, OD_RANGE.1);
            self.od.max = od_max.clamp(OD_RANGE.0, OD_RANGE.1);
        } else {
            self.od.enabled = false;
        }

        if hp_found {
            self.hp.enabled = true;
            self.hp.min = hp_min.clamp(HP_RANGE.0, HP_RANGE.1);
            self.hp.max = hp_max.clamp(HP_RANGE.0, HP_RANGE.1);
        } else {
            self.hp.enabled = false;
        }

        if bpm_found {
            self.bpm.enabled = true;
            self.bpm.min = bpm_min.clamp(BPM_RANGE.0, BPM_RANGE.1);
            self.bpm.max = bpm_max.clamp(BPM_RANGE.0, BPM_RANGE.1);
        } else {
            self.bpm.enabled = false;
        }

        self.length_min = len_min.map(trim_number).unwrap_or_default();
        self.length_max = len_max.map(trim_number).unwrap_or_default();

        if let Some(m) = mode {
            self.mode = m;
        } else {
            self.mode = ModeFilter::Any;
        }

        self.parsed_query = Some(parsed);
    }

    /// Rebuilds query_text when UI controls change.
    pub fn update_query_text_from_ui(&mut self) {
        self.query_text = self.to_osu_search();
        self.parsed_query = None;
    }

    /// Snapshot of UI filter state to detect user modifications in UI controls.
    pub fn ui_snapshot(&self) -> (
        (RangeFilter, RangeFilter, RangeFilter, RangeFilter),
        (RangeFilter, RangeFilter, ModeFilter),
        (String, String, String),
        (String, String, String, String),
    ) {
        (
            (self.stars.clone(), self.ar.clone(), self.cs.clone(), self.od.clone()),
            (self.hp.clone(), self.bpm.clone(), self.mode),
            (self.artist.clone(), self.title.clone(), self.mapper.clone()),
            (self.difficulty.clone(), self.tag.clone(), self.length_min.clone(), self.length_max.clone()),
        )
    }
}

/// Pre-lowered text needles for one refresh pass (see `lowered_text`).
#[derive(Debug, Clone, Default)]
pub struct LoweredTextQueries {
    pub artist: String,
    pub title: String,
    pub mapper: String,
    pub difficulty: String,
    pub tag: String,
}

fn lowered_needle(needle: &str) -> String {
    needle.trim().to_lowercase()
}

fn contains_lowered(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(needle_lower)
}

fn parse_bound(text: &str) -> Option<f32> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<f32>().ok().filter(|value| value.is_finite())
}

fn within_length(length_seconds: Option<f32>, min_text: &str, max_text: &str) -> bool {
    let min = parse_bound(min_text);
    let max = parse_bound(max_text);
    if min.is_none() && max.is_none() {
        return true;
    }
    let Some(length) = length_seconds else {
        return false;
    };
    min.is_none_or(|min| length >= min) && max.is_none_or(|max| length <= max)
}

fn push_text_token(tokens: &mut Vec<String>, key: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    tokens.push(format!("{key}={}", quote_if_needed(value)));
}

fn quote_if_needed(value: &str) -> String {
    if value.contains(' ') || value.contains('/') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filters() -> BeatmapFilters {
        BeatmapFilters::with_full_ranges()
    }

    #[test]
    fn builds_readable_osu_query() {
        let mut query = filters();
        query.artist = "Camellia".into();
        query.stars.enabled = true;
        query.stars.min = 5.5;

        assert_eq!(query.to_osu_search(), "artist=Camellia stars>=5.5 mode=osu");
    }

    #[test]
    fn default_filters_match_std_maps_with_known_values() {
        let query = filters();
        assert!(query.stars.enabled);
        assert!(query.ar.enabled);
        assert!(query.cs.enabled);
        assert!(!query.od.enabled);

        let map = LocalBeatmap {
            artist: "Camellia".into(),
            mode: None,
            stars: Some(5.0),
            ar: Some(9.0),
            cs: Some(4.0),
            ..Default::default()
        };
        assert!(query.matches_local(&map));

        // Unknown values never match an enabled filter.
        let unknown_stars = LocalBeatmap {
            mode: None,
            ar: Some(9.0),
            cs: Some(4.0),
            ..Default::default()
        };
        assert!(!query.matches_local(&unknown_stars));

        let taiko = LocalBeatmap {
            mode: Some(1),
            stars: Some(5.0),
            ar: Some(9.0),
            cs: Some(4.0),
            ..Default::default()
        };
        assert!(!query.matches_local(&taiko));
    }

    #[test]
    fn ranges_and_length_behave() {
        let mut query = filters();
        query.mode = ModeFilter::Any;
        query.stars.enabled = true;
        query.stars.min = 5.0;
        query.stars.max = 7.0;
        query.ar.enabled = false;
        query.cs.enabled = false;
        query.length_min = "60".into();
        query.length_max = "not a number".into();

        let inside = LocalBeatmap {
            stars: Some(6.0),
            length_seconds: Some(120.0),
            ..Default::default()
        };
        assert!(query.matches_local(&inside));

        let too_easy = LocalBeatmap {
            stars: Some(4.9),
            length_seconds: Some(120.0),
            ..Default::default()
        };
        assert!(!query.matches_local(&too_easy));

        // Missing values never match an enabled numeric filter.
        let unknown = LocalBeatmap::default();
        assert!(!query.matches_local(&unknown));
    }

    #[test]
    fn test_parse_time_formats() {
        assert_eq!(parse_time_or_number("120"), Some(120.0));
        assert_eq!(parse_time_or_number("2:30"), Some(150.0));
        assert_eq!(parse_time_or_number("1:05.5"), Some(65.5));
        assert_eq!(parse_time_or_number("0:45"), Some(45.0));
        assert_eq!(parse_time_or_number("invalid"), None);
    }

    #[test]
    fn test_tokenize_quotes_and_negation() {
        let tokens = tokenize_query(r#"artist="Camellia vs nanahira" stars>=5.5 -instrumental 12345"#);
        assert_eq!(
            tokens,
            vec![
                "artist=Camellia vs nanahira",
                "stars>=5.5",
                "-instrumental",
                "12345"
            ]
        );
    }

    #[test]
    fn test_wiki_numeric_operators() {
        let parsed = parse_osu_query("stars>=5.0 ar<9.5 cs!=4 bpm>180 length<=2:30 circles>=500 sliders<200");
        let map_pass = LocalBeatmap {
            stars: Some(5.5),
            ar: Some(9.0),
            cs: Some(3.8),
            bpm: Some(190.0),
            length_seconds: Some(140.0),
            circles: 600,
            sliders: 150,
            ..Default::default()
        };
        let map_fail = LocalBeatmap {
            stars: Some(4.5), // < 5.0
            ar: Some(9.0),
            cs: Some(3.8),
            bpm: Some(190.0),
            length_seconds: Some(140.0),
            circles: 600,
            sliders: 150,
            ..Default::default()
        };
        assert!(parsed.matches(&map_pass));
        assert!(!parsed.matches(&map_fail));
    }

    #[test]
    fn test_wiki_text_and_quoted() {
        let parsed = parse_osu_query("artist=\"super artist\" creator!=sotarks diff=insane");
        let map_pass = LocalBeatmap {
            artist: "Super Artist feat. XYZ".into(),
            creator: "NatsumeRin".into(),
            version: "Insane Difficulty".into(),
            ..Default::default()
        };
        let map_fail = LocalBeatmap {
            artist: "Super Artist".into(),
            creator: "Sotarks".into(), // != sotarks fails
            version: "Insane".into(),
            ..Default::default()
        };
        assert!(parsed.matches(&map_pass));
        assert!(!parsed.matches(&map_fail));
    }

    #[test]
    fn test_wiki_mode_and_status() {
        let parsed = parse_osu_query("mode=taiko status=ranked");
        let map_pass = LocalBeatmap {
            mode: Some(1),
            ranked_status: Some(4),
            ..Default::default()
        };
        let map_fail_mode = LocalBeatmap {
            mode: Some(0),
            ranked_status: Some(4),
            ..Default::default()
        };
        let map_fail_status = LocalBeatmap {
            mode: Some(1),
            ranked_status: Some(2), // graveyard/pending
            ..Default::default()
        };
        assert!(parsed.matches(&map_pass));
        assert!(!parsed.matches(&map_fail_mode));
        assert!(!parsed.matches(&map_fail_status));
    }

    #[test]
    fn test_wiki_unkeyed_and_ids() {
        let parsed_id = parse_osu_query("28107");
        let map_by_id = LocalBeatmap {
            beatmap_id: Some(28107),
            ..Default::default()
        };
        let map_by_set = LocalBeatmap {
            beatmapset_id: Some(28107),
            ..Default::default()
        };
        let map_other = LocalBeatmap {
            beatmap_id: Some(99999),
            ..Default::default()
        };
        assert!(parsed_id.matches(&map_by_id));
        assert!(parsed_id.matches(&map_by_set));
        assert!(!parsed_id.matches(&map_other));

        let parsed_neg = parse_osu_query("-tv");
        let map_clean = LocalBeatmap {
            title: "Full Version Song".into(),
            ..Default::default()
        };
        let map_tv = LocalBeatmap {
            title: "Anime Opening (TV Size)".into(),
            ..Default::default()
        };
        assert!(parsed_neg.matches(&map_clean));
        assert!(!parsed_neg.matches(&map_tv));
    }

    #[test]
    fn test_two_way_sync() {
        let mut filters = BeatmapFilters::default();
        filters.query_text = "artist=Camellia stars>=5.5 stars<=7.5 mode=taiko length>=120".into();
        filters.sync_ui_from_query_text();

        assert_eq!(filters.artist, "camellia");
        assert!(filters.stars.enabled);
        assert_eq!(filters.stars.min, 5.5);
        assert_eq!(filters.stars.max, 7.5);
        assert_eq!(filters.mode, ModeFilter::Taiko);
        assert_eq!(filters.length_min, "120");

        filters.update_query_text_from_ui();
        assert!(filters.query_text.contains("artist=camellia"));
        assert!(filters.query_text.contains("stars>=5.5"));
        assert!(filters.query_text.contains("stars<=7.5"));
        assert!(filters.query_text.contains("mode=taiko"));
    }
}
