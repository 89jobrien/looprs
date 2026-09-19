# Migrating to looprs 0.6

Version 0.6 adds runtime options for bounded parallel tool execution and MCP tool discovery. These options expanded the public `RuntimeSettings` struct, so the workspace uses a minor release rather than publishing the change as another 0.5 patch.

## RuntimeSettings construction

Code that constructed `RuntimeSettings` with a struct literal must use the stable constructor and builders.

Before:

```rust
let settings = RuntimeSettings {
    defaults,
    max_tokens_override,
    fs_mode,
};
```

After:

```rust
let settings = RuntimeSettings::new(defaults, max_tokens_override, fs_mode)
    .with_max_parallel(4)
    .with_mcp_server_url("http://127.0.0.1:3000");
```

Omit either builder to retain its default. `RuntimeSettings` is now non-exhaustive, preventing future option additions from breaking downstream construction again. Existing reads of public fields remain supported.

## Release compatibility gates

Release validation now requires health baseline and history artifacts for the exact workspace version. It also runs semantic-version checks for the published library crates. Refresh health artifacts after changing the release version:

```bash
taskit health check --with-coverage --update
jq -c . .health-baseline.json >> .health-history.jsonl
scripts/verify-release-health.sh 0.6.0
```

The workspace and fuzz lockfiles retain `quinn-proto 0.11.17`, matching the security entry in the changelog.
