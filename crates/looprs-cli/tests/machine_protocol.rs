//! Verifies versioned and legacy machine-readable CLI event output.

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

const V1: &str = "looprs-machine/v1";

#[derive(Clone, Copy)]
enum FakeResponse {
    Success,
    Failure,
    Delayed,
}

fn fake_ollama(response: FakeResponse) -> (String, Receiver<()>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake Ollama");
    listener
        .set_nonblocking(true)
        .expect("set fake server nonblocking");
    let address = format!("http://{}", listener.local_addr().expect("local address"));
    let (request_tx, request_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        for _ in 0..3_000 {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = request_tx.send(());
                    serve_response(&mut stream, response);
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return,
            }
        }
    });
    (address, request_rx, handle)
}

fn capturing_fake_ollama() -> (String, Receiver<String>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake Ollama");
    let address = format!("http://{}", listener.local_addr().expect("local address"));
    let (request_tx, request_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut request = [0_u8; 64 * 1024];
        let bytes_read = stream.read(&mut request).expect("read request");
        request_tx
            .send(String::from_utf8_lossy(&request[..bytes_read]).into_owned())
            .expect("send captured request");
        let body = json!({
            "message": {"role": "assistant", "content": "delegated ok"},
            "prompt_eval_count": 7,
            "eval_count": 3
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });
    (address, request_rx, handle)
}

fn serve_response(stream: &mut TcpStream, response: FakeResponse) {
    let mut request = [0_u8; 16 * 1024];
    let _ = stream.read(&mut request);
    if matches!(response, FakeResponse::Delayed) {
        thread::sleep(Duration::from_secs(3));
    }
    let (status, body) = match response {
        FakeResponse::Failure => ("500 Internal Server Error", json!({"error": "forced"})),
        FakeResponse::Success | FakeResponse::Delayed => (
            "200 OK",
            json!({
                "message": {"role": "assistant", "content": "machine ok"},
                "prompt_eval_count": 7,
                "eval_count": 3
            }),
        ),
    };
    let body = body.to_string();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn base_command(temp_home: &Path, ollama_host: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_looprs"));
    command
        .current_dir(temp_home)
        .env_clear()
        .env("HOME", temp_home)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("PROVIDER", "local")
        .env("MODEL", "test-model")
        .env("OLLAMA_HOST", ollama_host)
        .arg("--prompt")
        .arg("hello")
        .arg("--quiet")
        .arg("--no-hooks");
    command
}

fn command(temp_home: &Path, ollama_host: &str) -> Command {
    let mut command = base_command(temp_home, ollama_host);
    command
        .arg("--machine-protocol")
        .arg(V1)
        .arg("--run-id")
        .arg("integration-run");
    command
}

fn machine_events(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .filter(|value: &Value| value.get("protocol").and_then(Value::as_str) == Some(V1))
        .collect()
}

#[test]
fn v1_success_has_ordered_events_run_id_sequence_usage_and_separate_streams() {
    let home = tempfile::tempdir().expect("temp home");
    let (host, _requests, server) = fake_ollama(FakeResponse::Success);
    let output = command(home.path(), &host).output().expect("run looprs");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events = machine_events(&output);
    assert_eq!(
        events
            .first()
            .and_then(|v| v.pointer("/event/kind"))
            .and_then(Value::as_str),
        Some("run.started")
    );
    assert_eq!(
        events
            .last()
            .and_then(|v| v.pointer("/event/kind"))
            .and_then(Value::as_str),
        Some("run.succeeded")
    );
    assert!(events.iter().all(|v| v["run_id"] == "integration-run"));
    let sequences: Vec<u64> = events.iter().filter_map(|v| v["seq"].as_u64()).collect();
    assert!(sequences.windows(2).all(|pair| pair[1] == pair[0] + 1));
    assert_eq!(
        events.last().expect("success event")["event"]["data"]["usage"]["input_tokens"],
        7
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("machine ok"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("\"protocol\""));
    server.join().expect("fake server joins");
}

#[test]
fn v1_provider_failure_emits_failed_and_exits_nonzero() {
    let home = tempfile::tempdir().expect("temp home");
    let (host, _requests, server) = fake_ollama(FakeResponse::Failure);
    let output = command(home.path(), &host).output().expect("run looprs");
    assert!(!output.status.success());
    let events = machine_events(&output);
    assert_eq!(
        events.last().expect("failure event")["event"]["kind"],
        "run.failed"
    );
    server.join().expect("fake server joins");
}

#[test]
fn v1_expired_deadline_emits_cancelled_without_starting() {
    let home = tempfile::tempdir().expect("temp home");
    let mut command = command(home.path(), "http://127.0.0.1:9");
    command.env("LOOPRS_MACHINE_DEADLINE_MS", "1");
    let output = command.output().expect("run looprs");
    assert!(!output.status.success());
    let events = machine_events(&output);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"]["kind"], "run.cancelled");
    assert_eq!(events[0]["event"]["data"]["reason"], "deadline_exceeded");
}

#[test]
fn v1_existing_cancel_file_emits_cancelled_without_starting() {
    let home = tempfile::tempdir().expect("temp home");
    let cancel = home.path().join("cancel");
    std::fs::write(&cancel, "cancel").expect("write cancel file");
    let mut command = command(home.path(), "http://127.0.0.1:9");
    command.arg("--cancel-file").arg(&cancel);
    let output = command.output().expect("run looprs");
    assert!(!output.status.success());
    let events = machine_events(&output);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"]["kind"], "run.cancelled");
    assert_eq!(events[0]["event"]["data"]["reason"], "cancel_requested");
}

#[test]
fn v1_deadline_cancels_in_flight_provider_request() {
    let home = tempfile::tempdir().expect("temp home");
    let (host, _requests, server) = fake_ollama(FakeResponse::Delayed);
    let mut command = command(home.path(), &host);
    command.arg("--deadline-seconds").arg("1");
    let output = command.output().expect("run looprs");
    assert!(!output.status.success());
    let events = machine_events(&output);
    assert_eq!(
        events.first().expect("started")["event"]["kind"],
        "run.started"
    );
    assert_eq!(
        events.last().expect("cancelled")["event"]["kind"],
        "run.cancelled"
    );
    assert_eq!(
        events.last().expect("cancelled")["event"]["data"]["reason"],
        "deadline_exceeded"
    );
    server.join().expect("fake server joins");
}

#[test]
fn v1_cancel_file_interrupts_in_flight_provider_request() {
    let home = tempfile::tempdir().expect("temp home");
    let cancel = home.path().join("cancel-later");
    let (host, requests, server) = fake_ollama(FakeResponse::Delayed);
    let mut command = command(home.path(), &host);
    command
        .arg("--cancel-file")
        .arg(&cancel)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().expect("spawn looprs");
    requests
        .recv_timeout(Duration::from_secs(5))
        .expect("provider request should start");
    std::fs::write(&cancel, "cancel").expect("write cancel file");
    let output = child.wait_with_output().expect("wait for looprs");
    assert!(!output.status.success());
    let events = machine_events(&output);
    assert_eq!(
        events.last().expect("cancelled")["event"]["kind"],
        "run.cancelled"
    );
    assert_eq!(
        events.last().expect("cancelled")["event"]["data"]["reason"],
        "cancel_requested"
    );
    server.join().expect("fake server joins");
}

#[test]
fn legacy_machine_log_keeps_top_level_kind_and_data() {
    let home = tempfile::tempdir().expect("temp home");
    let (host, _requests, server) = fake_ollama(FakeResponse::Success);
    let mut command = base_command(home.path(), &host);
    command.arg("--machine-log");
    let output = command.output().expect("run looprs");
    let records: Vec<Value> = String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    assert!(records.iter().any(|value| value.get("kind").is_some()));
    assert!(records.iter().all(|value| value.get("protocol").is_none()));
    server.join().expect("fake server joins");
}

#[test]
fn scriptable_cli_delegates_through_runtime_orchestrator() {
    let home = tempfile::tempdir().expect("temp home");
    let looprs_dir = home.path().join(".looprs");
    let agents_dir = looprs_dir.join("agents");
    std::fs::create_dir_all(&agents_dir).expect("create agents directory");
    std::fs::write(
        looprs_dir.join("config.json"),
        r#"{"paths":{"agents":".looprs/agents","skills":".looprs/skills","plugins":".looprs/plugins"}}"#,
    )
    .expect("write config");
    std::fs::write(
        agents_dir.join("reviewer.yaml"),
        r#"name: reviewer
role: Reviewer
system_prompt: Review carefully
tools: [read]
triggers: [hello]
"#,
    )
    .expect("write agent");
    let (host, requests, server) = capturing_fake_ollama();

    let output = command(home.path(), &host).output().expect("run looprs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let request = requests
        .recv_timeout(Duration::from_secs(5))
        .expect("captured provider request");
    let body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .expect("HTTP request body");
    let payload: Value = serde_json::from_str(body).expect("request JSON");
    let user_content = payload["messages"]
        .as_array()
        .and_then(|messages| messages.iter().find(|message| message["role"] == "user"))
        .and_then(|message| message["content"].as_str())
        .expect("user prompt");
    assert!(user_content.contains("[Delegation]"));
    assert!(user_content.contains("Agent: reviewer"));
    server.join().expect("fake server joins");
}
