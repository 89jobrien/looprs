use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum ScoreTrigger {
    OnError,
    OnRepeat { tool_name: String, count: usize },
    OnDemand { n: usize },
}

#[derive(Debug)]
pub struct InteractionPair {
    pub prompt: String,
    pub response: String,
    pub session_id: String,
}

#[derive(Deserialize)]
struct RawEvent {
    #[serde(default)]
    event: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    session_id: String,
}

/// Read last `n` prompt→response pairs, ollama-tagged only.
pub fn load_last_n_ollama_pairs(path: &Path, n: usize) -> Result<Vec<InteractionPair>> {
    use std::io::BufRead;
    let file = std::fs::File::open(path)?;
    let events: Vec<RawEvent> = std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .filter(|e: &RawEvent| e.provider == "ollama")
        .collect();

    let mut pairs: Vec<InteractionPair> = Vec::new();
    let mut i = 0;
    while i + 1 < events.len() {
        if events[i].event == "user_message" && events[i + 1].event == "inference" {
            pairs.push(InteractionPair {
                prompt: events[i].content.clone(),
                response: events[i + 1].content.clone(),
                session_id: events[i].session_id.clone(),
            });
            i += 2;
        } else {
            i += 1;
        }
    }
    let skip = pairs.len().saturating_sub(n);
    Ok(pairs.into_iter().skip(skip).collect())
}

/// Call OpenAI to score pairs. Returns empty vec if OPENAI_API_KEY absent.
/// Writes scores to magi db at db_path if provided.
pub async fn run_scorer(
    pairs: &[InteractionPair],
    scorer_model: &str,
    db_path: Option<&str>,
) -> Result<Vec<f32>> {
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(k) => k,
        Err(_) => {
            log::warn!("OPENAI_API_KEY not set — skipping interaction scoring");
            return Ok(vec![]);
        }
    };

    let client = reqwest::Client::new();
    let mut scores = Vec::new();

    for pair in pairs {
        let body = serde_json::json!({
            "model": scorer_model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are an expert code reviewer. Score this coding response \
                                from 0.0 to 1.0. Reply with only a JSON object: \
                                {\"score\": <float>}"
                },
                {
                    "role": "user",
                    "content": format!("Task: {}\n\nResponse: {}", pair.prompt, pair.response)
                }
            ],
            "temperature": 0.0
        });

        let resp = client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .await;

        let score = match resp {
            Ok(r) if r.status().is_success() => {
                let v: serde_json::Value = r.json().await.unwrap_or_default();
                let content = v["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("{}");
                let parsed: serde_json::Value =
                    serde_json::from_str(content).unwrap_or_default();
                parsed["score"].as_f64().unwrap_or(0.5) as f32
            }
            _ => {
                log::warn!("OpenAI scoring call failed — skipping interaction");
                continue;
            }
        };

        scores.push(score);

        if let Some(db) = db_path {
            if let Err(e) = write_score_to_db(db, &pair.session_id, &pair.prompt, &pair.response, score).await {
                log::warn!("failed to write score to magi db: {e}");
            }
        }
    }

    Ok(scores)
}

async fn write_score_to_db(
    db_path: &str,
    session_id: &str,
    task: &str,
    response: &str,
    score: f32,
) -> Result<()> {
    use rusqlite::Connection;
    let db_path = db_path.to_owned();
    let session_id = session_id.to_owned();
    let task = task.to_owned();
    let response = response.to_owned();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let conn = Connection::open(&db_path)?;
        conn.execute(
            "INSERT OR IGNORE INTO interactions \
             (task, response, judge_score, reward, processed, source_session) \
             VALUES (?1, ?2, ?3, ?4, 0, ?5)",
            rusqlite::params![task, response, score as f64, score as f64, session_id],
        )?;
        Ok(())
    })
    .await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    fn write_fixture(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("2026-04-07-sess-abc.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"ts":"2026-04-07T00:00:00Z","session_id":"s1","event":"user_message","content":"fix the bug","provider":"ollama"}}"#).unwrap();
        writeln!(f, r#"{{"ts":"2026-04-07T00:00:01Z","session_id":"s1","event":"inference","content":"here is the fix","provider":"ollama"}}"#).unwrap();
        writeln!(f, r#"{{"ts":"2026-04-07T00:00:02Z","session_id":"s1","event":"inference","content":"cloud response","provider":"openai"}}"#).unwrap();
        path
    }

    #[test]
    fn test_load_pairs_filters_ollama_only() {
        let dir = tempdir().unwrap();
        let path = write_fixture(dir.path());
        let pairs = load_last_n_ollama_pairs(&path, 10).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].prompt, "fix the bug");
        assert_eq!(pairs[0].response, "here is the fix");
    }

    #[test]
    fn test_load_pairs_respects_limit() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sess.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 0..5usize {
            writeln!(f, r#"{{"ts":"2026-04-07T00:00:00Z","session_id":"s1","event":"user_message","content":"q{i}","provider":"ollama"}}"#).unwrap();
            writeln!(f, r#"{{"ts":"2026-04-07T00:00:00Z","session_id":"s1","event":"inference","content":"a{i}","provider":"ollama"}}"#).unwrap();
        }
        let pairs = load_last_n_ollama_pairs(&path, 2).unwrap();
        assert_eq!(pairs.len(), 2);
    }
}
