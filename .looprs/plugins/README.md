# Plugins

Looprs now supports first-class plugin manifests in `.looprs/plugins/`.

Supported kinds:

- `tool`
- `runtime`
- `orchestration`

Current integration uses orchestration plugins to route prompts to agent roles.

Minimal orchestration plugin:

```yaml
name: route-health
kind: orchestration
enabled: true
required: false
mode: daemon
triggers:
  - "taskit health"
route_to_agent: taskit
```

Notes:

- Repo plugins override user plugins with the same `(kind, name)` key.
- `#agent` explicit selection still has priority over plugin routing.
- `required: true` is kind-scoped: if a required orchestration plugin matches
  but cannot route correctly, delegation fails for that turn.
