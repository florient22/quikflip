//! Runs the Claude Code CLI headless (`claude -p`) with a JSON schema, so answers come back as validated JSON.
//! Authentication comes from the environment (CLAUDE_CODE_OAUTH_TOKEN, made with `claude setup-token`).

use std::env;
use std::process::Stdio;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const DEFAULT_PROGRAM: &str = "claude";
const DEFAULT_TIMEOUT_SECONDS: u64 = 900;

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
            .args(["-p", "--output-format", "json", "--tools", "", "--no-session-persistence", "--json-schema"])
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
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().chars().take(600).collect();
            // The CLI also prints JSON errors on stdout; prefer that message when there is one.
            if let Ok(message) = parse_envelope::<Value>(&output.stdout) {
                return Err(ClaudeError::Failed { status: output.status.to_string(), stderr: message.to_string() });
            }
            return Err(ClaudeError::Failed { status: output.status.to_string(), stderr });
        }
        parse_envelope(&output.stdout)
    }
}

/// Reads the CLI's `--output-format json` envelope: `structured_output` when present, otherwise JSON inside `result`.
pub fn parse_envelope<T: DeserializeOwned>(stdout: &[u8]) -> Result<T, ClaudeError> {
    let envelope: Value = serde_json::from_slice(stdout).map_err(|error| ClaudeError::Shape(format!("not JSON: {error}")))?;
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

    #[test]
    fn reads_structured_output_and_falls_back_to_result() {
        let structured = br#"{"type":"result","is_error":false,"result":"ignored","structured_output":{"fruits":["Apple"]}}"#;
        assert_eq!(parse_envelope::<Fruits>(structured).unwrap().fruits, ["Apple"]);
        let fenced = br#"{"type":"result","is_error":false,"result":"```json\n{\"fruits\":[\"Pear\"]}\n```"}"#;
        assert_eq!(parse_envelope::<Fruits>(fenced).unwrap().fruits, ["Pear"]);
        let failed = br#"{"type":"result","is_error":true,"result":"Invalid API key"}"#;
        assert!(matches!(parse_envelope::<Fruits>(failed), Err(ClaudeError::Reported(_))));
    }
}
