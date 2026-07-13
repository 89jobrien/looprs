use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering},
};

use looprs::{FsMode, ui};
use rustyline::completion::Completer;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Cmd, ConditionalEventHandler, Context, Editor, Event, EventContext, EventHandler};
use rustyline::{KeyCode, KeyEvent, Modifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplMode {
    Normal,
    Slash,
    Skill,
    Colon,
}

#[derive(Debug, Clone)]
pub struct MatchSets {
    pub commands: Vec<String>,
    pub skills: Vec<String>,
    pub settings: Vec<String>,
}

#[derive(Debug)]
pub struct ReplState {
    mode: ReplMode,
    last_completed: Option<String>,
}

impl ReplState {
    pub fn new() -> Self {
        Self {
            mode: ReplMode::Normal,
            last_completed: None,
        }
    }

    pub fn reset(&mut self) {
        self.mode = ReplMode::Normal;
        self.last_completed = None;
    }
}

pub struct ReplHelper {
    state: Arc<Mutex<ReplState>>,
    sets: Arc<MatchSets>,
}

impl ReplHelper {
    pub fn new(sets: MatchSets) -> Self {
        Self {
            state: Arc::new(Mutex::new(ReplState::new())),
            sets: Arc::new(sets),
        }
    }

    pub fn state(&self) -> Arc<Mutex<ReplState>> {
        self.state.clone()
    }

    pub fn sets(&self) -> Arc<MatchSets> {
        self.sets.clone()
    }

    pub fn reset(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.reset();
        }
    }
}

impl Highlighter for ReplHelper {}
impl Validator for ReplHelper {}

impl Completer for ReplHelper {
    type Candidate = String;

    fn complete(
        &self,
        _line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        Ok((pos, Vec::new()))
    }
}

impl Hinter for ReplHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        let state = self.state.lock().ok()?;
        if pos != line.len() {
            return None;
        }
        match state.mode {
            ReplMode::Slash => completion_hint(line, pos, '/', &self.sets.commands),
            ReplMode::Skill => completion_hint(line, pos, '$', &self.sets.skills),
            ReplMode::Colon => settings_hint(line, pos, &self.sets.settings),
            ReplMode::Normal => None,
        }
    }
}

impl rustyline::Helper for ReplHelper {}

pub fn bind_repl_keys(
    editor: &mut Editor<ReplHelper, DefaultHistory>,
    state: Arc<Mutex<ReplState>>,
    sets: Arc<MatchSets>,
    fs_mode: Arc<AtomicU8>,
) {
    let slash_handler = ReplHandler::new(HandlerKind::Slash, state.clone(), sets.clone());
    let skill_handler = ReplHandler::new(HandlerKind::Skill, state.clone(), sets.clone());
    let colon_handler = ReplHandler::new(HandlerKind::Colon, state.clone(), sets.clone());
    let enter_handler = ReplHandler::new(HandlerKind::Enter, state.clone(), sets.clone());
    let tab_handler = FsModeToggleHandler::new(fs_mode);
    let esc_handler = ReplHandler::new(HandlerKind::Esc, state, sets);

    let _ = editor.bind_sequence(
        Event::from(KeyEvent(KeyCode::Char('/'), Modifiers::NONE)),
        EventHandler::Conditional(Box::new(slash_handler)),
    );
    let _ = editor.bind_sequence(
        Event::from(KeyEvent(KeyCode::Char('$'), Modifiers::NONE)),
        EventHandler::Conditional(Box::new(skill_handler)),
    );
    let _ = editor.bind_sequence(
        Event::from(KeyEvent(KeyCode::Char(':'), Modifiers::NONE)),
        EventHandler::Conditional(Box::new(colon_handler)),
    );
    let _ = editor.bind_sequence(
        Event::from(KeyEvent(KeyCode::Enter, Modifiers::NONE)),
        EventHandler::Conditional(Box::new(enter_handler)),
    );
    let _ = editor.bind_sequence(
        Event::from(KeyEvent(KeyCode::Esc, Modifiers::NONE)),
        EventHandler::Conditional(Box::new(esc_handler)),
    );
    let _ = editor.bind_sequence(
        Event::from(KeyEvent(KeyCode::Tab, Modifiers::NONE)),
        EventHandler::Conditional(Box::new(tab_handler)),
    );
}

struct FsModeToggleHandler {
    fs_mode: Arc<AtomicU8>,
}

impl FsModeToggleHandler {
    fn new(fs_mode: Arc<AtomicU8>) -> Self {
        Self { fs_mode }
    }

    fn handle_tab(&self, ctx: &EventContext) -> Option<Cmd> {
        if !ctx.line().is_empty() || ctx.pos() != 0 {
            return None;
        }

        let current = FsMode::from_u8(self.fs_mode.load(Ordering::Relaxed));
        let next = current.next();
        self.fs_mode.store(next.to_u8(), Ordering::Relaxed);
        ui::info(format!("fs_mode = {}", next.as_str()));
        Some(Cmd::AcceptLine)
    }
}

impl ConditionalEventHandler for FsModeToggleHandler {
    fn handle(
        &self,
        _evt: &Event,
        _n: rustyline::RepeatCount,
        _positive: bool,
        ctx: &EventContext,
    ) -> Option<Cmd> {
        self.handle_tab(ctx)
    }
}

#[derive(Debug, Clone, Copy)]
enum HandlerKind {
    Slash,
    Skill,
    Colon,
    Enter,
    Esc,
}

struct ReplHandler {
    kind: HandlerKind,
    state: Arc<Mutex<ReplState>>,
    sets: Arc<MatchSets>,
}

impl ReplHandler {
    fn new(kind: HandlerKind, state: Arc<Mutex<ReplState>>, sets: Arc<MatchSets>) -> Self {
        Self { kind, state, sets }
    }

    fn handle_mode_key(&self, ctx: &EventContext, mode: ReplMode, ch: char) -> Option<Cmd> {
        if !ctx.line().is_empty() || ctx.pos() != 0 {
            return None;
        }
        let mut state = self.state.lock().ok()?;
        state.mode = mode;
        state.last_completed = None;
        Some(Cmd::SelfInsert(1, ch))
    }

    fn handle_enter(&self, ctx: &EventContext) -> Option<Cmd> {
        let line = ctx.line();
        let pos = ctx.pos();
        if pos != line.len() {
            return None;
        }

        let mut state = self.state.lock().ok()?;
        match state.mode {
            ReplMode::Slash => handle_completion_enter(line, '/', &self.sets.commands, &mut state),
            ReplMode::Skill => handle_completion_enter(line, '$', &self.sets.skills, &mut state),
            _ => None,
        }
    }

    fn handle_escape(&self, ctx: &EventContext) -> Option<Cmd> {
        if ctx.line().is_empty() {
            return None;
        }
        let mut state = self.state.lock().ok()?;
        if state.mode == ReplMode::Normal {
            return None;
        }
        state.reset();
        Some(Cmd::Kill(rustyline::Movement::WholeLine))
    }
}

impl ConditionalEventHandler for ReplHandler {
    fn handle(
        &self,
        _evt: &Event,
        _n: rustyline::RepeatCount,
        _positive: bool,
        ctx: &EventContext,
    ) -> Option<Cmd> {
        match self.kind {
            HandlerKind::Slash => self.handle_mode_key(ctx, ReplMode::Slash, '/'),
            HandlerKind::Skill => self.handle_mode_key(ctx, ReplMode::Skill, '$'),
            HandlerKind::Colon => self.handle_mode_key(ctx, ReplMode::Colon, ':'),
            HandlerKind::Enter => self.handle_enter(ctx),
            HandlerKind::Esc => self.handle_escape(ctx),
        }
    }
}

fn completion_hint(line: &str, pos: usize, prefix: char, items: &[String]) -> Option<String> {
    if pos != line.len() {
        return None;
    }
    if !line.starts_with(prefix) {
        return None;
    }
    let token = line.split_whitespace().next().unwrap_or("");
    if token.len() <= 1 || token.contains(' ') {
        return None;
    }
    let query = token.trim_start_matches(prefix);
    let best = best_match(query, prefix, items)?;
    let completion = best.strip_prefix(token).unwrap_or("");
    if completion.is_empty() {
        None
    } else {
        Some(completion.to_string())
    }
}

fn settings_hint(line: &str, pos: usize, items: &[String]) -> Option<String> {
    if pos != line.len() {
        return None;
    }
    let line = line.trim_start_matches(':');
    let mut parts = line.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    if cmd.is_empty() {
        return None;
    }
    if !matches!(cmd, "set" | "get" | "unset") {
        return None;
    }
    let query = parts.next().unwrap_or("");
    let best = best_match(query, '\0', items)?;
    let completion = best.strip_prefix(query).unwrap_or(best);
    if completion.is_empty() {
        None
    } else {
        Some(completion.to_string())
    }
}

fn handle_completion_enter(
    line: &str,
    prefix: char,
    items: &[String],
    state: &mut ReplState,
) -> Option<Cmd> {
    if !line.starts_with(prefix) {
        return None;
    }
    let token = line.split_whitespace().next().unwrap_or("");
    if token.is_empty() {
        return None;
    }
    let query = token.trim_start_matches(prefix);
    let best = best_match(query, prefix, items)?;
    let matched = best.as_str();

    let has_args = line.split_whitespace().count() > 1;
    if token == matched {
        if has_args {
            state.reset();
            return Some(Cmd::AcceptLine);
        }
        if state.last_completed.as_deref() == Some(matched) {
            state.reset();
            return Some(Cmd::AcceptLine);
        }
        state.last_completed = Some(matched.to_string());
        return Some(Cmd::Insert(1, " ".to_string()));
    }

    let completion = matched.strip_prefix(token).unwrap_or("");
    if completion.is_empty() {
        return None;
    }
    state.last_completed = Some(matched.to_string());
    Some(Cmd::Insert(1, completion.to_string()))
}

fn best_match<'a>(query: &str, prefix: char, items: &'a [String]) -> Option<&'a String> {
    let query = query.trim().to_lowercase();
    if items.is_empty() {
        return None;
    }
    if query.is_empty() {
        return items.first();
    }

    let mut best_score = i32::MIN;
    let mut best_item = None;

    for item in items {
        let candidate = if prefix == '\0' {
            item.as_str()
        } else {
            item.trim_start_matches(prefix)
        };
        if let Some(score) = fuzzy_score(&query, candidate) {
            let score = score - candidate.len() as i32;
            if score > best_score {
                best_score = score;
                best_item = Some(item);
            }
        }
    }

    best_item
}

const CONSECUTIVE_MATCH_BONUS: i32 = 10;

fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    let mut score = 0;
    let mut last_match = None;
    let mut pos = 0;
    let cand = candidate.to_lowercase();
    for ch in query.chars() {
        if let Some(idx) = cand[pos..].find(ch) {
            let abs = pos + idx;
            if let Some(prev) = last_match {
                if abs == prev + 1 {
                    score += CONSECUTIVE_MATCH_BONUS;
                } else {
                    score += 2;
                }
            } else {
                score += 1;
            }
            last_match = Some(abs);
            pos = abs + 1;
        } else {
            return None;
        }
    }
    Some(score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // IDEA(Q2): proptest invariants for the three fuzzy-match functions.

    proptest! {
        #[test]
        fn prop_fuzzy_score_empty_query_always_matches(candidate in "[a-z]{1,20}") {
            // Empty query matches every candidate (vacuous match).
            prop_assert!(fuzzy_score("", &candidate).is_some());
        }

        #[test]
        fn prop_fuzzy_score_prefix_beats_sparse(
            prefix in "[a-z]{2,6}",
            suffix in "[a-z]{1,6}",
        ) {
            // A query that matches as a contiguous prefix should score at
            // least as high as the same query matched sparsely (prefix chars
            // interleaved with extra chars in between).
            let candidate_prefix = format!("{prefix}{suffix}");
            // Build a sparse candidate: insert 'z' between each prefix char.
            let sparse: String = prefix.chars().flat_map(|c| [c, 'z']).collect();
            let sparse_candidate = format!("{sparse}{suffix}");
            if let (Some(s_prefix), Some(s_sparse)) =
                (fuzzy_score(&prefix, &candidate_prefix), fuzzy_score(&prefix, &sparse_candidate))
            {
                prop_assert!(
                    s_prefix >= s_sparse,
                    "prefix score {s_prefix} should be ≥ sparse score {s_sparse}"
                );
            }
        }

        #[test]
        fn prop_best_match_returns_exact_when_present(
            name in "[a-z]{3,10}",
            extra in "[a-z]{3,10}",
        ) {
            prop_assume!(name != extra);
            let items = vec![
                format!("/{name}"),
                format!("/{extra}"),
            ];
            // Querying for an exact item name should return that item.
            let result = best_match(&name, '/', &items);
            prop_assert!(result.is_some());
            prop_assert_eq!(result.unwrap(), &format!("/{name}"));
        }
    }

    // ── fuzzy_score ──────────────────────────────────────────────────────────

    #[test]
    fn fuzzy_score_exact_prefix_scores_positive() {
        let score = fuzzy_score("ref", "refactor");
        assert!(score.is_some());
        assert!(score.unwrap() > 0);
    }

    #[test]
    fn fuzzy_score_no_match_returns_none() {
        assert!(fuzzy_score("xyz", "abc").is_none());
    }

    #[test]
    fn fuzzy_score_consecutive_chars_beat_sparse() {
        let consecutive = fuzzy_score("ab", "abc").unwrap();
        let sparse = fuzzy_score("ab", "axb").unwrap();
        assert!(
            consecutive > sparse,
            "consecutive ({consecutive}) should outscore sparse ({sparse})"
        );
    }

    #[test]
    fn fuzzy_score_empty_query_returns_some() {
        assert!(fuzzy_score("", "anything").is_some());
    }

    // ── best_match ───────────────────────────────────────────────────────────

    #[test]
    fn best_match_returns_closest_prefix() {
        let items = vec![
            "/refactor".to_string(),
            "/test".to_string(),
            "/lint".to_string(),
        ];
        let result = best_match("ref", '/', &items);
        assert_eq!(result.map(String::as_str), Some("/refactor"));
    }

    #[test]
    fn best_match_empty_items_returns_none() {
        assert!(best_match("ref", '/', &[]).is_none());
    }

    #[test]
    fn best_match_empty_query_returns_first() {
        let items = vec!["/a".to_string(), "/b".to_string()];
        assert_eq!(best_match("", '/', &items).map(String::as_str), Some("/a"));
    }

    // ── completion_hint ──────────────────────────────────────────────────────

    #[test]
    fn completion_hint_empty_line_returns_none() {
        let items = vec!["/refactor".to_string()];
        assert!(completion_hint("", 0, '/', &items).is_none());
    }

    #[test]
    fn completion_hint_returns_suffix() {
        let items = vec!["/refactor".to_string()];
        let hint = completion_hint("/ref", 4, '/', &items);
        assert_eq!(hint.as_deref(), Some("actor"));
    }

    #[test]
    fn completion_hint_exact_match_returns_none() {
        let items = vec!["/refactor".to_string()];
        assert!(completion_hint("/refactor", 9, '/', &items).is_none());
    }

    // ── ReplState ────────────────────────────────────────────────────────────

    #[test]
    fn repl_state_starts_in_normal_mode() {
        let state = ReplState::new();
        assert_eq!(state.mode, ReplMode::Normal);
        assert!(state.last_completed.is_none());
    }

    #[test]
    fn repl_state_reset_clears_mode_and_completion() {
        let mut state = ReplState::new();
        state.mode = ReplMode::Slash;
        state.last_completed = Some("/refactor".to_string());
        state.reset();
        assert_eq!(state.mode, ReplMode::Normal);
        assert!(state.last_completed.is_none());
    }

    // ── handle_completion_enter ──────────────────────────────────────────────

    #[test]
    fn handle_completion_enter_inserts_suffix_on_partial_match() {
        let items = vec!["/refactor".to_string()];
        let mut state = ReplState::new();
        let result = handle_completion_enter("/ref", '/', &items, &mut state);
        assert!(result.is_some());
        assert_eq!(state.last_completed.as_deref(), Some("/refactor"));
    }

    #[test]
    fn handle_completion_enter_accepts_line_on_second_exact_match() {
        let items = vec!["/refactor".to_string()];
        let mut state = ReplState::new();
        state.last_completed = Some("/refactor".to_string());
        let result = handle_completion_enter("/refactor", '/', &items, &mut state);
        assert!(matches!(result, Some(rustyline::Cmd::AcceptLine)));
    }

    #[test]
    fn handle_completion_enter_non_prefix_returns_none() {
        let items = vec!["/refactor".to_string()];
        let mut state = ReplState::new();
        assert!(handle_completion_enter("hello", '/', &items, &mut state).is_none());
    }

    // ── property tests ───────────────────────────────────────────────────────

    proptest! {
        /// If fuzzy_score returns Some, every char of query must appear in candidate
        /// in order (subsequence invariant).
        #[test]
        fn prop_fuzzy_score_some_implies_subsequence(
            query in "[a-z]{0,8}",
            candidate in "[a-z]{0,16}",
        ) {
            if fuzzy_score(&query, &candidate).is_some() {
                let mut pos = 0;
                let cand: Vec<char> = candidate.chars().collect();
                for ch in query.chars() {
                    let found = cand[pos..].iter().position(|&c| c == ch);
                    prop_assert!(
                        found.is_some(),
                        "fuzzy_score returned Some but '{ch}' not found after pos {pos} in '{candidate}'"
                    );
                    pos += found.unwrap() + 1;
                }
            }
        }

        /// Empty query always matches any candidate.
        #[test]
        fn prop_fuzzy_score_empty_query_always_some(candidate in "[a-z]{0,16}") {
            prop_assert!(fuzzy_score("", &candidate).is_some());
        }
    }
}
