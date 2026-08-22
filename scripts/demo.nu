#!/usr/bin/env nu
# Demo run of looprs against the OpenAI provider, using scriptable mode.

def main [prompt: string = "In one sentence, explain what looprs does."] {
    let key = (op read "op://cli/OpenAI/credential")
    with-env {PROVIDER: "openai", OPENAI_API_KEY: $key} {
        ^./target/debug/looprs -p $prompt
    }
}
