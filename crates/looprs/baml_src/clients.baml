client<llm> Anthropic {
    provider anthropic
    retry_policy Retry
    options {
        model "claude-sonnet-4-6"
        api_key env.ANTHROPIC_API_KEY
        max_tokens 8096
    }
}

client<llm> OpenAI {
    provider openai
    retry_policy Retry
    options {
        model "gpt-4o"
        api_key env.OPENAI_API_KEY
    }
}

client<llm> Ollama {
    provider openai-generic
    options {
        base_url "http://localhost:11434/v1"
        model "llama3.2"
    }
}

client<llm> DefaultClient {
    provider fallback
    options {
        strategy [Anthropic, OpenAI]
    }
}

retry_policy Retry {
    max_retries 2
    strategy {
        type exponential_backoff
        delay_ms 500
        multiplier 2
        max_delay_ms 10000
    }
}
