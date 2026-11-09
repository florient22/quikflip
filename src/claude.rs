//! Runs the Claude Code CLI headless (`claude -p`) with a JSON schema, so answers come back as validated JSON.
//! Authentication comes from the environment (CLAUDE_CODE_OAUTH_TOKEN, made with `claude setup-token`).
//! The CLI streams JSON lines; besides the final result they carry the subscription's usage-limit status, which
//! tells us when a refused call may be tried again.

use std::env;
use std::process::Stdio;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const DEFAULT_PROGRAM: &str = "claude";
const DEFAULT_TIMEOUT_SECONDS: u64 = 900;
/// The HTTP status the API answers with when the subscription's usage limit is reached.
const TOO_MANY_REQUESTS: u64 = 429;
const LIMIT_REJECTED: &str = "rejected";

#[derive(Debug, thiserror::Error)]
pub enum ClaudeError {
    #[error("could not start the Claude CLI ({program}): {source}; install Claude Code or set QUIKFLIP_CLAUDE_BIN")]
    Spawn { program: String, source: std::io::Error },
    #[error("Claude did not finish within {0} seconds")]
    Timeout(u64),
    #[error("the Claude CLI exited with {status}: {stderr}")]
    Failed { status: String, stderr: String },
    #[error("Claude reported an error: {0}")]
    Reported(String),
    #[error("Claude's answer did not have the expected shape: {0}")]
    Shape(String),
    /// The subscription's usage limit is reached; `resets_at` is the Unix time (seconds) it resets, when known.
    #[error("Claude's usage limit is reached{}", reset_text(*resets_at))]
    UsageLimit { resets_at: Option<i64> },
}

#[derive(Debug, Clone)]
pub struct Claude {
    program: String,
    model: Option<String>,
    timeout: Duration,
}

impl Claude {
    /// QUIKFLIP_CLAUDE_BIN (default "claude"), QUIKFLIP_CLAUDE_MODEL (default: the CLI's own default),
    /// QUIKFLIP_CLAUDE_TIMEOUT_SECONDS (default 900).
    pub fn from_env() -> anyhow::Result<Self> {
        let program = env::var("QUIKFLIP_CLAUDE_BIN").unwrap_or_else(|_| DEFAULT_PROGRAM.to_owned());
        let model = env::var("QUIKFLIP_CLAUDE_MODEL").ok().filter(|model| !model.trim().is_empty());
        let timeout = match env::var("QUIKFLIP_CLAUDE_TIMEOUT_SECONDS") {
            Ok(text) => text.parse().map_err(|_| anyhow::anyhow!("QUIKFLIP_CLAUDE_TIMEOUT_SECONDS must be a whole number, got {text:?}"))?,
            Err(_) => DEFAULT_TIMEOUT_SECONDS,
        };
        Ok(Self { program, model, timeout: Duration::from_secs(timeout) })
    }

    /// Sends `prompt` and returns Claude's answer, validated against `schema` by the CLI and parsed into T.
    pub async fn structured<T: DeserializeOwned>(&self, prompt: &str, schema: &Value) -> Result<T, ClaudeError> {
        let mut command = Command::new(&self.program);
        command
            .args(["-p", "--output-format", "stream-json", "--verbose", "--tools", "", "--no-session-persistence", "--json-schema"])
            .arg(schema.to_string());
        if let Some(model) = &self.model {
            command.args(["--model", model]);
        }
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        let mut child = command.spawn().map_err(|source| ClaudeError::Spawn { program: self.program.clone(), source })?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await.map_err(|source| ClaudeError::Spawn { program: self.program.clone(), source })?;
        }
        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| ClaudeError::Timeout(self.timeout.as_secs()))?
            .map_err(|source| ClaudeError::Spawn { program: self.program.clone(), source })?;
        let stream = read_stream(&output.stdout);
        if let Some(limit) = stream.usage_limit() {
            return Err(limit);
        }
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().chars().take(600).collect();
            // The CLI also reports errors in its result line; prefer that message when there is one.
            if let Some(result) = &stream.result {
                return Err(ClaudeError::Failed { status: output.status.to_string(), stderr: result.to_string().chars().take(600).collect() });
            }
            return Err(ClaudeError::Failed { status: output.status.to_string(), stderr });
        }
        let result = stream.result.ok_or_else(|| ClaudeError::Shape("no result line in the output".into()))?;
        parse_envelope(&result)
    }
}

fn reset_text(resets_at: Option<i64>) -> String {
    resets_at.and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0)).map(|time| format!(" until {}", time.format("%Y-%m-%d %H:%M UTC"))).unwrap_or_default()
}

/// What the CLI's `--output-format stream-json` lines tell us: the final result and the last usage-limit status.
#[derive(Debug, Default)]
struct Stream {
    result: Option<Value>,
    limit_status: Option<String>,
    limit_resets_at: Option<i64>,
}

impl Stream {
    /// A usage-limit error when the call failed because the limit is reached.
    fn usage_limit(&self) -> Option<ClaudeError> {
        let failed = self.result.as_ref().is_none_or(|result| result.get("is_error").and_then(Value::as_bool) == Some(true));
        let refused = self.limit_status.as_deref() == Some(LIMIT_REJECTED)
            || self.result.as_ref().and_then(|result| result.get("api_error_status")).and_then(Value::as_u64) == Some(TOO_MANY_REQUESTS);
        (failed && refused).then_some(ClaudeError::UsageLimit { resets_at: self.limit_resets_at })
    }
}

fn read_stream(stdout: &[u8]) -> Stream {
    let mut stream = Stream::default();
    for line in String::from_utf8_lossy(stdout).lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else { continue };
        match message.get("type").and_then(Value::as_str) {
            Some("rate_limit_event") => {
                let info = message.get("rate_limit_info");
                stream.limit_status = info.and_then(|info| info.get("status")).and_then(Value::as_str).map(str::to_owned);
                stream.limit_resets_at = info.and_then(|info| info.get("resetsAt")).and_then(Value::as_i64);
            }
            Some("result") => stream.result = Some(message),
            _ => {}
        }
    }
    stream
}

/// Reads the CLI's result message: `structured_output` when present, otherwise JSON inside `result`.
pub fn parse_envelope<T: DeserializeOwned>(envelope: &Value) -> Result<T, ClaudeError> {
    if envelope.get("is_error").and_then(Value::as_bool) == Some(true) {
        let message = envelope.get("result").and_then(Value::as_str).or_else(|| envelope.get("subtype").and_then(Value::as_str)).unwrap_or("unknown error");
        return Err(ClaudeError::Reported(message.to_owned()));
    }
    let payload = match envelope.get("structured_output").filter(|value| !value.is_null()) {
        Some(value) => value.clone(),
        None => {
            let text = envelope.get("result").and_then(Value::as_str).ok_or_else(|| ClaudeError::Shape("no result in the answer".into()))?;
            let start = text.find('{').ok_or_else(|| ClaudeError::Shape("no JSON object in the answer".into()))?;
            let end = text.rfind('}').ok_or_else(|| ClaudeError::Shape("no JSON object in the answer".into()))?;
            serde_json::from_str(&text[start..=end]).map_err(|error| ClaudeError::Shape(error.to_string()))?
        }
    };
    serde_json::from_value(payload).map_err(|error| ClaudeError::Shape(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug, PartialEq)]
    struct Fruits {
        fruits: Vec<String>,
    }

    fn result_of(stdout: &[u8]) -> Value {
        read_stream(stdout).result.unwrap()
    }

    #[test]
    fn reads_structured_output_and_falls_back_to_result() {
        let structured = br#"{"type":"result","is_error":false,"result":"ignored","structured_output":{"fruits":["Apple"]}}"#;
        assert_eq!(parse_envelope::<Fruits>(&result_of(structured)).unwrap().fruits, ["Apple"]);
        let fenced = br#"{"type":"result","is_error":false,"result":"```json\n{\"fruits\":[\"Pear\"]}\n```"}"#;
        assert_eq!(parse_envelope::<Fruits>(&result_of(fenced)).unwrap().fruits, ["Pear"]);
        let failed = br#"{"type":"result","is_error":true,"result":"Invalid API key"}"#;
        assert!(matches!(parse_envelope::<Fruits>(&result_of(failed)), Err(ClaudeError::Reported(_))));
    }

    #[test]
    fn a_refused_call_reports_when_the_limit_resets() {
        let refused = b"{\"type\":\"system\",\"subtype\":\"init\"}\n\
            {\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"status\":\"rejected\",\"resetsAt\":1794223800}}\n\
            {\"type\":\"result\",\"is_error\":true,\"result\":\"limit reached\"}\n";
        assert!(matches!(read_stream(refused).usage_limit(), Some(ClaudeError::UsageLimit { resets_at: Some(1_794_223_800) })));
        let status_only = br#"{"type":"result","is_error":true,"api_error_status":429,"result":"limit"}"#;
        assert!(matches!(read_stream(status_only).usage_limit(), Some(ClaudeError::UsageLimit { resets_at: None })));
        let fine = b"{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"status\":\"allowed\",\"resetsAt\":1794223800}}\n\
            {\"type\":\"result\",\"is_error\":false,\"structured_output\":{\"fruits\":[]}}\n";
        assert!(read_stream(fine).usage_limit().is_none());
    }
}
