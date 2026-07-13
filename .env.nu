$env.ANTHROPIC_API_KEY = (op item get co34pzkizfie5ax2joyfz4754u
    --vault cli
    --account my.1password.com
    --fields label=ANTHROPIC_API_KEY
    --reveal
    | str trim)

# Suppress BAML runtime request logs. Set to "info" to see traces.
$env.BAML_LOG = "error"
