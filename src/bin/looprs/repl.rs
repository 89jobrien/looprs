use std::sync::{Arc, Mutex};

use rustyline::completion::Completer;
use rustyline::hint::Hinter;
use rustyline::highlight::Highlighter;
use rustyline::validate::Validator;
use rustyline::{Cmd, ConditionalEventHandler, Context, Editor, Event, EventContext, EventHandler};
use rustyline::{KeyCode, KeyEvent, Modifiers};
use rustyline::history::DefaultHistory;

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
) {
    let slash_handler = ReplHandler::new(HandlerKind::SlashKey, state.clone(), sets.clone());
    let skill_handler = ReplHandler::new(HandlerKind::SkillKey, state.clone(), sets.clone());
    let colon_handler = ReplHandler::new(HandlerKind::ColonKey, state.clone(), sets.clone());
    let enter_handler = ReplHandler::new(HandlerKind::EnterKey, state.clone(), sets.clone());
    let esc_handler = ReplHandler::new(HandlerKind::EscKey, state, sets);

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
}

#[derive(Debug, Clone, Copy)]
enum HandlerKind {
    SlashKey,
    SkillKey,
    ColonKey,
    EnterKey,
    EscKey,
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
            HandlerKind::SlashKey => self.handle_mode_key(ctx, ReplMode::Slash, '/'),
            HandlerKind::SkillKey => self.handle_mode_key(ctx, ReplMode::Skill, '$'),
            HandlerKind::ColonKey => self.handle_mode_key(ctx, ReplMode::Colon, ':'),
            HandlerKind::EnterKey => self.handle_enter(ctx),
            HandlerKind::EscKey => self.handle_escape(ctx),
        }
    }
}

fn completion_hint(
    line: &str,
    pos: usize,
    prefix: char,
    items: &[String],
) -> Option<String> {
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
                    score += 10;
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
