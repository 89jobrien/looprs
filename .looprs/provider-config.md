# Provider Configuration

Looprs supports configuration via `.looprs/provider.json` for advanced users who want to persist settings or manage multiple provider configurations.

## Configuration File Location

```
.looprs/provider.json
```

## Configuration Priority

Settings are loaded in this order (first match wins):

1. **Environment Variables** (highest priority)
   - `PROVIDER=` env var
   - `ANTHROPIC_API_KEY=` env var
   - `OPENAI_API_KEY=` env var
   - `MODEL=` env var

2. **`.looprs/provider.json`** (config file)
   - `provider` field (explicit provider selection)
   - Provider-specific settings (model, max_tokens, etc.)

3. **Auto-Detection** (if no explicit config)
   - Check for API keys in order: Anthropic → OpenAI
   - Try local Ollama availability
   - Error if none found

## Schema

```json
{
  "provider": "anthropic",
  "defaults": {
    "max_tokens": 8192,
    "timeout_secs": 120
  },
  "anthropic": {
    "model": "claude-3-sonnet-20240229",
    "max_tokens": 8192
  },
  "openai": {
    "model": "gpt-4-turbo",
    "max_tokens": 4096
  },
  "local": {
    "model": "mistral",
    "timeout_secs": 60
  }
}
```

## Configuration Options

### Root-Level Options

- **`provider`** (string, optional) - Force specific provider
  - Values: `"anthropic"`, `"openai"`, `"local"`
  - Overrides environment variable detection

### Provider-Specific Settings

Each provider can have its own configuration block:

- **`anthropic`** - Anthropic/Claude settings
- **`openai`** - OpenAI/GPT settings
- **`local`** - Ollama settings

### Available Settings Per Provider

- **`model`** (string) - Model ID/name
  - Anthropic: `claude-3-opus-20240229`, `claude-3-sonnet-20240229`, `claude-3-haiku-20240307`
  - OpenAI: `gpt-4`, `gpt-4-turbo`, `gpt-4-32k`, `gpt-5`
  - Local: `llama2`, `mistral`, `neural-chat`, `codeup`, etc.

- **`max_tokens`** (integer) - Maximum tokens in response
  - Default: 8192
  - Recommended: 4096-8192 (depends on provider and model)

- **`timeout_secs`** (integer) - API request timeout in seconds
  - Default: 120
  - Local models may need shorter timeouts (60-90)

- **`*`** (any other fields) - Provider-specific settings
  - Will be available to custom extensions in the future

### Default Settings

The `defaults` section applies to all providers unless overridden:

```json
{
  "defaults": {
    "max_tokens": 8192,
    "timeout_secs": 120
  }
}
```

Provider-specific settings override defaults.

## Examples

### Example 1: Use OpenAI GPT-4 Turbo

```json
{
  "provider": "openai",
  "openai": {
    "model": "gpt-4-turbo",
    "max_tokens": 4096
  }
}
```

Set environment variable:
```bash
export OPENAI_API_KEY="sk-..."
looprs
```

### Example 2: Multiple Providers with Different Settings

```json
{
  "defaults": {
    "max_tokens": 8192
  },
  "anthropic": {
    "model": "claude-3-sonnet-20240229"
  },
  "openai": {
    "model": "gpt-4-turbo",
    "max_tokens": 4096
  },
  "local": {
    "model": "mistral",
    "timeout_secs": 60
  }
}
```

Then switch providers via environment:
```bash
# Use Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."
looprs

# Or use OpenAI
export OPENAI_API_KEY="sk-..."
export PROVIDER=openai
looprs

# Or use local Ollama
export PROVIDER=local
looprs
```

### Example 3: Override Config File with Environment

```json
{
  "provider": "anthropic",
  "anthropic": {
    "model": "claude-3-opus-20240229"
  }
}
```

But force a different model:
```bash
export MODEL=claude-3-sonnet-20240229
looprs
```

Result: Uses Anthropic provider with Sonnet model.

## Loading Behavior

- If `.looprs/provider.json` doesn't exist, it's optional (no error)
- Invalid JSON causes a clear error message
- Missing fields use defaults
- Environment variables always take precedence over config file

## Validation

The config file is validated when:
1. Looprs starts and loads configuration
2. The provider is created
3. API keys are checked

Errors will be clear about what's misconfigured.

## Tips

- **Keep it simple**: Start with environment variables only
- **Config file for teams**: Use config file + dotenv for team projects
- **Provider switching**: Use `PROVIDER` env var to override config file
- **Model testing**: Use `MODEL` env var to try different models temporarily
- **Version control**: Don't commit actual API keys, use `.env.example` instead

## Troubleshooting

### Config file not loaded

- Check file location: `.looprs/provider.json`
- Verify JSON syntax with `jq . .looprs/provider.json`
- Check file permissions

### Provider not detected

1. Verify config file exists and has valid JSON
2. Check environment variables are set
3. For local Ollama, ensure `ollama serve` is running on localhost:11434
4. Check error message for specifics

### Settings not applied

- Environment variables override config file
- Make sure field names are correct (see schema above)
- Check for typos in provider names
