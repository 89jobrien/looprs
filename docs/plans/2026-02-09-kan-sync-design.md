# Kan Sync Design

Date: 2026-02-09

## Goal

Centralize issues into a single source of truth that both the user and the LLM can read and create/update. Looprs should surface a consistent kan board view derived from that source, and make it configurable per repo.

## Decisions

- **Source of truth**: configurable per repo.
- **Config location**: `.looprs/config.json`.
- **Operations**: read + create + update (no delete/close unless added later).
- **Issue schema**: `id`, `title`, `status`, `priority`, `assignee`, `tags`, `updated_at`, `meta`.
- **Status taxonomy**: `backlog | ready | in_progress | review | done`.
- **Unknown fields**: preserved in `meta`.
- **Kan output**: counts by column (current `KanStatus` shape), with a path to extend to full cards later.

## Architecture

Add a normalized issue model and an adapter layer that selects a backing store based on config.

Components:

1) `Issue` model + normalization helpers.
2) `IssueStore` trait: `list`, `get`, `create`, `update`.
3) `BdIssueStore` adapter.
4) `JsonlIssueStore` adapter.
5) `KanProjector` to convert issues → `KanStatus` (counts by status).
6) Config-driven selection in `.looprs/config.json`.

Keep the sync logic inside looprs so SessionStart context and kan status remain consistent and testable.

## Config

Example `.looprs/config.json`:

```json
{
  "issues": {
    "source": "bd",
    "jsonl_path": ".looprs/issues.jsonl"
  }
}
```

- `source` is `bd` or `jsonl`.
- `jsonl_path` only used when `source` is `jsonl` (default to `.looprs/issues.jsonl`).

## Data Flow

- On SessionStart, looprs selects a store, lists issues, and projects to `KanStatus` for context injection.
- LLM actions create/update issues via the same store, keeping source of truth consistent.
- Unknown fields round-trip via `meta`.

Normalization rules:

- Unknown status → `backlog`, preserve `meta.original_status`.
- Missing priority → `normal`.
- Missing `id` in JSONL → generate deterministic ID and record `meta.generated_id`.
- Missing `assignee`/`tags` → null / empty list.
- `updated_at` set to current UTC on create/update when absent.

## Error Handling

- `bd` missing or not a bd repo: return no issues and emit a warning once per session.
- JSONL missing: treat as empty and create on first write.
- JSON parse errors per line: skip line, warn with line number, continue.
- Reject `jsonl_path` that escapes repo unless explicitly allowed.

JSONL updates:

- Append-only for create.
- For update: rewrite atomically (write temp, then rename).
- Optional optimistic check via `updated_at` or a `revision` field if present.

## Testing (Full TDD)

Unit tests:

- Normalization mapping: status, default priority, meta preservation, generated IDs.
- JSONL: parse valid lines, skip invalid, create/update, atomic rewrite.
- BD: parse `bd list --json`, map to normalized schema, handle missing bd tool.

Integration tests:

- Temp repo with jsonl source: create issue → update issue → `kan status --json` counts match.
- Mocked `bd` binary in PATH (if feasible) for list/create/update flows.

Behavior tests:

- SessionStart context includes kan counts when configured.
- LLM command path hits adapter and updates source.

## Open Questions

- Should `close/reopen` be added to the CRUD surface now or later?
- Should `priority` be a constrained enum (`low|normal|high|urgent`) or kept free-form?
- Do we need a migration tool for existing `bd` issues → jsonl?
