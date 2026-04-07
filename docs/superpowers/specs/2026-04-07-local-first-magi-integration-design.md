# looprs: Local-First Coding Agent + magi Integration

**Date:** 2026-04-07
**Status:** Approved
**Scope:** looprs session logging, magi hook bridge, provider routing, new commands, desktop badge

---

## Positioning

looprs is a local-first coding agent. No data leaves the machine by default. No cloud API is
required for inference. The user owns their primary model and improves it over time through magi's
automated RL pipeline.

**Provider intent:**

| Provider | Role |
|----------|------|
| Ollama   | Default inference — the model you own and improve |
| OpenAI   | Task outsourcing (explicit), LLM-as-judge (automatic), eval (triggered) |

magi only ingests and trains on Ollama-tagged interactions. OpenAI interactions are never fed into
the RL pipeline.

---

## Architecture

### Session Logging

looprs writes a structured JSONL session log to `~/.looprs/sessions/<date>-<session-id>.jsonl`.
Each line is one event object. Required fields on every event:

```json
{ "ts": "<iso8601>", "session_id": "<uuid>", "provider": "ollama|openai", "event": "<type>", ... }
```

Event types: `user_message`, `inference`, `tool_use`, `tool_result`, `session_end`.

The `provider` field on each event enables magi to filter — only `ollama` events enter the RL
pipeline.

### magi Hook Bridge

magi ships a looprs hook at `hooks/looprs/magi_ingest.yaml` (within the magi repo).
Install by symlinking: `ln -s ~/dev/magi/hooks/looprs/magi_ingest.yaml ~/.looprs/hooks/`. It fires on `SessionEnd`, reads the
session JSONL, filters to `provider=ollama` events, reconstructs task/response/tool pairs, and
writes them to magi's `db/rewards.db`.

looprs requires no code changes beyond the session log format. The hook is the only coupling point.
Hook failure is silent — looprs never blocks on it. Errors go to `~/.looprs/logs/hooks.log`.

### Provider Routing

`~/.looprs/models.toml` defines curated model tiers:

```toml
[default]
provider = "ollama"
model = "magistral-small-rl-v17"  # tracks magi's current trained version

[tiers]
fast      = { provider = "ollama", model = "qwen2.5-coder:7b" }
capable   = { provider = "ollama", model = "magistral-small-rl-v17" }
outsource = { provider = "openai", model = "gpt-4o" }
judge     = { provider = "openai", model = "gpt-5.4" }

[magi]
modelcard = "/Users/joe/dev/magi/modelcard.yaml"  # path to magi modelcard; overridable
db        = "/Users/joe/dev/magi/db/rewards.db"   # path to magi rewards db
```

The `default` model is the one magi trains. When magi produces a new version, the user updates
`models.toml` (or `/reset-model` handles rollback).

### New Commands

| Command        | Behavior |
|----------------|----------|
| `/model-status` | Reads `modelcard.yaml`, prints current model version, mean reward (last 50), last training run |
| `/fine-tune`    | Flags current session as high-priority in magi db (bumps reward signal for RL) |
| `/reset-model`  | Reverts `models.toml` default to a specified base version |
| `/eval [n=10]`  | Triggers OpenAI judge on last N interactions (default 10), writes scores to magi db |
| `/outsource`    | Re-runs current task against the `outsource` tier model |

### Eval Triggers

OpenAI judge is invoked automatically in two conditions, and manually on demand:

- `on-error`: tool failure → auto-trigger `/eval` on that interaction
- `on-repeat`: same tool called ≥3 times in a session → auto-trigger `/eval`
- `on-demand`: user explicitly runs `/eval`

If OpenAI is unavailable (no key, rate limit), eval is skipped silently. The interaction is scored
with rule+embed signals only.

### Desktop Badge

looprs desktop reads `modelcard.yaml` on startup, polls every 60s. Displays:

- Current model version
- Mean reward over last 50 interactions
- Training status: `idle` | `scoring` | `training`

If `modelcard.yaml` is missing or malformed, badge shows "unknown" — no crash.

---

## Data Flow

### Happy Path (Ollama → magi training)

```
user prompt
  → looprs agent loop (Ollama inference)
  → tool execution
  → events written to session JSONL (provider=ollama)
  → SessionEnd fires magi_ingest hook
  → hook extracts ollama interactions → writes to rewards.db
  → magi score.py: rule + embed + memory signals
  → if reward in [0.3, 0.7]: gpt-5.4 judge called (cached)
  → reward stored → prompt RL update
  → if 500+ high-reward interactions: weight RL (MLX LoRA) → new Ollama model
  → modelcard.yaml updated → desktop badge refreshes
```

### Outsource Path

```
user runs /outsource (or on-error / on-repeat auto-trigger)
  → task routed to openai provider
  → response returned to user
  → event tagged provider=openai → NOT written to magi db
```

### Eval Path

```
on-error / on-repeat / /eval
  → last N interactions extracted from session JSONL
  → gpt-5.4 judge called with task + response
  → scores written to rewards.db
  → feeds normal magi RL pipeline
```

---

## Error Handling

| Failure | Behavior |
|---------|----------|
| magi hook failure | Silent — session unaffected. Error logged to `~/.looprs/logs/hooks.log`. |
| OpenAI unavailable | Eval skipped. Interaction scored with rule+embed only. No crash, no retry. |
| Ollama unavailable | Surface error to user. Fall back to `outsource` tier if configured. No silent fallback. |
| `modelcard.yaml` missing/malformed | Desktop badge shows "unknown". No crash. |
| magi db write conflict | Hook uses SQLite WAL mode. Retry once, then drop with log entry. |

---

## Testing

| Layer | Tests |
|-------|-------|
| looprs session log | Unit: valid JSONL output, correct provider tags, all event types present |
| looprs commands | Integration: `/model-status`, `/fine-tune`, `/reset-model`, `/eval`, `/outsource` against mock modelcard + mock Ollama |
| magi ingest | `test_ingest_from_looprs_session`: fixture JSONL → assert correct db writes |
| Desktop badge | Snapshot test against fixture `modelcard.yaml` |

---

## Out of Scope (This Phase)

- magi UI changes
- Changes to magi's RL pipeline internals
- Cloud fallback for inference (outsourcing is explicit, not automatic fallback)
- Multi-user or shared model training
