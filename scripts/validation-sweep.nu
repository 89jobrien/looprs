#!/usr/bin/env nu
# Proof-of-concept validation sweep using looprs in scriptable mode.
# Runs one prompt per target file, treats exit code 0 as success (looprs
# writes plain text to stdout on success; --json only wraps failures).
#
# Usage: ./scripts/validation-sweep.nu file1.rs file2.rs ...

def main [...files: string] {
    let key = (op read "op://cli/OpenAI/credential")
    for file in $files {
        let prompt = $"Review @($file) for correctness issues only. List findings as bullet points, or say 'clean' if none."
        print $"=== ($file) ==="
        let result = (
            with-env {PROVIDER: "openai", OPENAI_API_KEY: $key} {
                do { ^./target/debug/looprs -p $prompt } | complete
            }
        )
        if $result.exit_code == 0 {
            print $result.stdout
        } else {
            print $"FAILED \(exit ($result.exit_code)\): ($result.stderr)"
        }
    }
}
