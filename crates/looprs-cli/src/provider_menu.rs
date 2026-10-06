use anyhow::Result;
use std::env;

use looprs::ui;
use looprs::{ProviderConfig, ProviderSettings};

/// List locally installed Ollama models via `ollama list`. Returns an
/// empty vec if `ollama` isn't on PATH or the command fails.
pub(crate) fn list_ollama_models() -> Vec<String> {
    let Ok(output) = std::process::Command::new("ollama").arg("list").output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let Ok(text) = String::from_utf8(output.stdout) else {
        return Vec::new();
    };
    parse_ollama_list_output(&text)
}

/// Pure parser for `ollama list` output: first column of every row after
/// the `NAME  ID  SIZE  MODIFIED` header.
fn parse_ollama_list_output(text: &str) -> Vec<String> {
    text.lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

pub(crate) fn models_gist_url() -> String {
    env::var("LOOPRS_MODELS_GIST_URL").unwrap_or_else(|_| {
        "https://gist.githubusercontent.com/pydanticai/known-models/refs/heads/main/models.json"
            .to_string()
    })
}

/// Interactive `looprs provider` entrypoint: pick a provider, and for
/// `local` also pick an installed Ollama model, then persist the choice
/// to `.looprs/provider.json`.
pub(crate) fn run_provider_menu() -> Result<()> {
    // TODO(feature-idea-4): Offer every provider supported by the runtime and
    // collect any provider-specific model settings before persisting a choice.
    let providers = vec![
        "anthropic".to_string(),
        "openai".to_string(),
        "local (Ollama)".to_string(),
    ];

    let Some(index) = looprs_tui::select("Select a provider", &providers)? else {
        println!("Cancelled.");
        return Ok(());
    };

    let mut config = ProviderConfig::load().unwrap_or_default();

    match index {
        0 => config.provider = Some("anthropic".to_string()),
        1 => config.provider = Some("openai".to_string()),
        2 => {
            let models = list_ollama_models();
            if models.is_empty() {
                ui::error(
                    "No Ollama models found. Install Ollama and run `ollama pull <model>` first.",
                );
                return Ok(());
            }
            let Some(model_index) = looprs_tui::select("Select a local model", &models)? else {
                println!("Cancelled.");
                return Ok(());
            };
            config.provider = Some("local".to_string());
            config.local = Some(ProviderSettings {
                model: Some(models[model_index].clone()),
                ..Default::default()
            });
        }
        _ => unreachable!("select() returned an out-of-range index"),
    }

    config.save()?;
    println!(
        "Saved provider={} to .looprs/provider.json",
        config.provider.as_deref().unwrap_or("?")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_ollama_list_output;

    // Captured from a real `ollama list` invocation.
    const REAL_OLLAMA_LIST_OUTPUT: &str = "NAME                                             ID              SIZE      MODIFIED\nfunctiongemma:latest                             7c19b650567a    300 MB    2 months ago\ngemma-lg:latest                                  e6349aa91a78    24 GB     2 months ago\nhf.co/unsloth/gemma-4-26B-A4B-it-GGUF:UD-Q6_K    e6349aa91a78    24 GB     2 months ago\nnomic-embed-text:latest                          0a109f422b47    274 MB    2 months ago\nllama3.2:latest                                  a80c4f17acd5    2.0 GB    4 months ago\n";

    #[test]
    fn parses_model_names_from_real_output() {
        let models = parse_ollama_list_output(REAL_OLLAMA_LIST_OUTPUT);
        assert_eq!(
            models,
            vec![
                "functiongemma:latest",
                "gemma-lg:latest",
                "hf.co/unsloth/gemma-4-26B-A4B-it-GGUF:UD-Q6_K",
                "nomic-embed-text:latest",
                "llama3.2:latest",
            ]
        );
    }

    #[test]
    fn header_only_output_yields_no_models() {
        let models = parse_ollama_list_output(
            "NAME                                             ID              SIZE      MODIFIED\n",
        );
        assert!(models.is_empty());
    }

    #[test]
    fn empty_output_yields_no_models() {
        assert!(parse_ollama_list_output("").is_empty());
    }

    #[test]
    fn parse_ollama_list_output_skips_header_and_reads_names() {
        let text = "NAME ID SIZE MODIFIED\nllama3.2:latest abc 2G now\n";
        let parsed = parse_ollama_list_output(text);
        assert_eq!(parsed, vec!["llama3.2:latest"]);
    }
}
