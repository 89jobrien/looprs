# Config Ownership and Seed Command — Implementation Plan

## Problem Statement

The application currently writes and overwrites `.looprs/config.json`, which should be user-owned. Specifically:

1. **Full overwrite**: `AppConfig::save()` is called from `save_configs()` in `main.rs` whenever the user runs `:set` or `:unset` in the REPL. This serializes the in-memory `AppConfig` and writes it to `.looprs/config.json`, wiping any user-added or hand-edited keys.
2. **Onboarding flag**: The hook executor calls `AppConfig::set_onboarding_demo_seen(flag)`, which does a read-modify-write on `.looprs/config.json`. This preserves unknown fields but still modifies the user's file.

**Objective**: Treat `.looprs/config.json` (and optionally `.looprs/provider.json`) as user-controlled. The app must not overwrite or generate them during normal operation. Provide a dedicated `seed` command that generates *example* config files into a user-specified directory so users can copy or merge as they wish.

## Technical Approach

### Principles

- **User owns config**: No code path in normal run (REPL, hooks, scriptable mode) should write `config.json` or `provider.json` in a way that overwrites user content.
- **Seed is opt-in**: A new CLI subcommand `looprs seed [DIR]` creates example config files only when the user asks; it never runs automatically.
- **Session vs persisted**: Runtime changes from `:set`/`:unset` can remain session-only, or be persisted to a separate, app-managed file (e.g. `.looprs/session-overrides.json`) that is applied on top of user config. This plan recommends session-only for simplicity; optional override file can be a follow-up.

### Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Where does `:set`/`:unset` persist? | Session-only (no write to config.json) | User file stays untouched. Downside: settings don't persist across runs; user can set them in config.json or env. |
| Onboarding `demo_seen` | Store in a separate state file (e.g. `.looprs/state.json`) | Keeps config.json strictly user-owned; app can read/write state for UI flags. |
| Seed output | Write example files with `.example` suffix by default; optional `--write-if-missing` to create `config.json` only when absent | Avoids overwriting; user copies `config.json.example` → `config.json` or uses flag for first-time setup. |
| Seed directory argument | Optional `[DIR]`; default `./.looprs`; support `~` expansion and absolute paths | Matches USER_PROMPT: "cwd/.looprs or ~/.looprs or whatever dir the user inputs". |

## Step-by-Step Implementation

### Phase 1: Stop Writing User Config

1. **Remove `AppConfig::save()` from normal flow**
   - In `src/bin/looprs/main.rs`, remove or refactor `save_configs()` so it does **not** call `app_config.save()`.
   - Keep `provider_config.save()` only if product decision is to allow app to write provider.json; otherwise remove that too and treat provider.json as user-owned (recommended for consistency).
   - **Result**: `handle_colon_command` still updates in-memory `app_config` and `provider_config`; changes apply for the session only and are not written to disk.

2. **Introduce a state file for onboarding**
   - Add a small state abstraction, e.g. `src/state.rs` or extend `app_config.rs`:
     - Path: `.looprs/state.json` (or `.looprs/onboarding_state.json`).
     - Contents: e.g. `{"onboarding": {"demo_seen": true}}`.
   - Load this in addition to (or instead of) reading `onboarding.demo_seen` from `config.json`. If both exist, state file wins for `demo_seen` so the hook only touches state.
   - In `src/hooks/executor.rs`, replace `AppConfig::set_onboarding_demo_seen(flag)` with writing to the state file (e.g. `State::set_onboarding_demo_seen(flag)`).
   - **Config load**: When loading `AppConfig`, merge or prefer state file for `onboarding.demo_seen` so existing behavior (e.g. "don't show onboarding again") is preserved.

3. **Deprecate or limit `AppConfig::save()`**
   - Either remove `AppConfig::save()` and `set_onboarding_demo_seen` that writes to config, or keep `save()` only for tests and document that production code must not call it for user config. Prefer removal to prevent accidental use.

4. **Provider config**
   - If provider.json is also user-owned: stop calling `provider_config.save()` in `save_configs()`. Session-only provider overrides from `:set`; user edits `~/.looprs/provider.json` or repo `.looprs/provider.json` for persistence.

### Phase 2: Seed Command

5. **CLI subcommand parsing**
   - In `src/bin/looprs/args.rs` (or main), detect a subcommand when the first non-flag argument is `seed`:
     - `looprs seed` → seed into `./.looprs` (create if missing).
     - `looprs seed ~/.looprs` → seed into home `.looprs`.
     - `looprs seed /path/to/dir` → seed into that dir.
   - Expand `~` to `env::home_dir()` (or equivalent). Resolve to absolute path for consistent behavior.
   - Run seed logic then `std::process::exit(0)` so main REPL/scriptable flow is not entered.

6. **Seed implementation**
   - New module: `src/seed.rs` (or `src/bin/looprs/seed.rs`).
   - Input: target directory (Path).
   - Behavior:
     - `fs::create_dir_all(dir)?`.
     - Write `config.json.example` with default `AppConfig` serialized (same structure as today's default: `defaults`, `file_references`, `onboarding`).
     - Write `provider.json.example` with default `ProviderConfig` (empty or minimal).
     - Optional: `--write-if-missing` flag: if `config.json` does not exist in dir, write default as `config.json` (not .example). Same for `provider.json` if desired.
   - Output: print which files were written and where (e.g. "Wrote .looprs/config.json.example, .looprs/provider.json.example").

7. **Example content**
   - Reuse `AppConfig::default()` and `serde_json::to_string_pretty` for config.json.example.
   - Reuse `ProviderConfig::default()` for provider.json.example.
   - No secrets or machine-specific paths; keep examples safe to commit.

### Phase 3: Config Load and State

8. **AppConfig load order**
   - Keep loading from `.looprs/config.json` if present (user file).
   - For `onboarding.demo_seen`, after loading config, overlay value from `.looprs/state.json` if that file exists and contains the key. So: state file overrides config for this flag only.

9. **Documentation**
   - README or .looprs docs: state that `config.json` and `provider.json` are user-controlled; use `looprs seed [DIR]` to generate example files. Document `state.json` as app-managed (onboarding and similar flags).

## Potential Challenges and Solutions

| Challenge | Solution |
|-----------|----------|
| Users expect `:set` to persist | Document that settings are session-only; persist via config.json or env. Optional follow-up: persist to `.looprs/session-overrides.json` and merge at load. |
| Existing deployments have app-written config.json | No migration needed; we just stop writing. Existing file remains. State file is new; if missing, treat as demo_seen=false. |
| Seed overwriting existing files | Default: write only `.example` files. Optional `--write-if-missing` only writes when config.json (or provider.json) is absent. |
| Home dir on Unix vs Windows | Use `dirs::home_dir()` or `env::home_dir()`; expand `~` in seed target before `create_dir_all`. |

## Testing Strategy

- **Unit**
  - `AppConfig::load()`: when config.json missing, returns default; when state.json has `onboarding.demo_seen`, that value wins.
  - State module: set then load `demo_seen`, assert value and that config.json was not modified.
  - Seed: run seed into a temp dir; assert `config.json.example` and `provider.json.example` exist and are valid JSON; assert no `config.json` created unless `--write-if-missing` and file missing.
- **Integration**
  - Run `looprs seed /tmp/looprs-seed-test`; verify files and that `looprs` still starts (no regression from removing save).
  - Run REPL, `:set defaults.temperature 0.5`, quit; run again and assert temperature is not 0.5 (session-only).
  - Onboarding hook sets demo_seen: assert state file updated, config.json unchanged (if present).

## Success Criteria

- No code path in normal operation writes `.looprs/config.json` or overwrites user-provided config.
- Onboarding and similar UI state use a dedicated state file (e.g. `.looprs/state.json`), not config.json.
- `looprs seed [DIR]` creates example config (and optionally provider) files in the given directory; supports `./.looprs`, `~/.looprs`, and arbitrary paths with `~` expansion.
- `:set`/`:unset` affect only the current session; user can set defaults in config.json or env.
- Tests cover load order (config + state), seed output, and no config write from REPL.

## Code Hooks (Reference)

- **Remove/change save**: `src/bin/looprs/main.rs` — `save_configs()`, and all call sites (e.g. `handle_colon_command` for `:set`/`:unset`).
- **Onboarding write**: `src/hooks/executor.rs` — `Action::SetConfig` for `onboarding.demo_seen`; switch to state file writer.
- **Config load**: `src/app_config.rs` — `load()`: add state file overlay for `onboarding.demo_seen`.
- **New**: `src/seed.rs` (or under `bin/looprs`), and main/args entry for `looprs seed [DIR]`.

## Optional Follow-Up

- **Persist :set/:unset**: Introduce `.looprs/session-overrides.json` (or similar), merge at load, write only that file from `save_configs()`. User config.json remains untouched; overrides apply on top.
- **Seed --write-if-missing**: Implement and document for first-time setup.
- **Provider config ownership**: If provider.json is also user-owned, remove `provider_config.save()` and document; seed still generates provider.json.example.
