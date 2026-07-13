#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! regex = "1"
//! ```

use regex::Regex;
use std::{env, fs};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(target) = args.first() else {
        println!("Usage: /reset-model <model-name>  e.g. /reset-model magistral-small-2506");
        std::process::exit(1);
    };

    let valid = Regex::new(r"^[a-zA-Z0-9._:/-]+$").unwrap();
    if !valid.is_match(target) {
        println!("Invalid model name: {target:?}");
        std::process::exit(1);
    }

    let home = env::var("HOME").unwrap_or_else(|_| ".".into());
    let cfg_path = format!("{home}/.looprs/models.toml");
    if !std::path::Path::new(&cfg_path).exists() {
        println!("models.toml not found at {cfg_path}");
        std::process::exit(1);
    }

    let content = fs::read_to_string(&cfg_path).expect("read models.toml");

    // Patch only the [default] section's model = "..." line
    let section_re = Regex::new(r"(?m)^(\[[^\]]+\])").unwrap();
    let model_re = Regex::new(r#"(?m)^(model\s*=\s*)"[^"]*""#).unwrap();

    let mut result = String::new();
    let mut last = 0usize;
    let mut in_default = false;

    for mat in section_re.find_iter(&content) {
        let chunk = &content[last..mat.start()];
        if in_default {
            result.push_str(
                &model_re.replace(chunk, format!(r#"${{1}}"{target}""#).as_str()),
            );
        } else {
            result.push_str(chunk);
        }
        in_default = mat.as_str() == "[default]";
        result.push_str(mat.as_str());
        last = mat.end();
    }

    let tail = &content[last..];
    if in_default {
        result.push_str(&model_re.replace(tail, format!(r#"${{1}}"{target}""#).as_str()));
    } else {
        result.push_str(tail);
    }

    fs::write(&cfg_path, result).expect("write models.toml");
    println!("Default model reset to: {target}");
}
