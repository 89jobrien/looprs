# Plugins

Looprs supports plugin manifests in `.looprs/plugins/` and
`$HOME/.looprs/plugins/`. Repository manifests override user manifests with the
same `(kind, name)` key.

Supported kinds:

- `tool`
- `runtime`
- `orchestration`

Orchestration manifests route prompts to agent roles. Tool and runtime manifests
can describe managed daemon processes for integrations that consume the
supervision port.

Minimal orchestration plugin:

```yaml
name: route-health
kind: orchestration
enabled: true
required: false
mode: one_shot
triggers:
  - "taskit health"
route_to_agent: taskit
```

This routing example is `one_shot`: selection is evaluated in-process for each
prompt and does not claim a resident plugin process.

Managed daemon example:

```yaml
name: local-tool-service
kind: tool
enabled: true
required: true
mode: daemon
entry:
  command: local-tool-service
  args: ["serve", "--stdio"]
  probe:
    command: local-tool-service
    args: ["health"]
```

Daemon lifecycle:

- Looprs resolves `entry.command` without a shell and passes `args` verbatim.
- Enabled daemons launch when manifests load. Status reports the live PID.
- Health checks first verify process liveness, then run the optional probe.
- Restart replaces the process and stops after three attempts.
- Shutdown terminates the child and retains a `stopped` status.
- Missing or blank daemon commands and malformed manifests fail loading.
- Disabled daemon manifests may omit `entry`; they remain disabled and are not launched.

Routing notes:

- `#agent` explicit selection still has priority over plugin routing.
- `required: true` is kind-scoped: if a required orchestration plugin matches
  but cannot route correctly, delegation fails for that turn.
