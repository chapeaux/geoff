//! JSON-RPC bridge: manages a Deno subprocess and sends/receives messages
//! over stdin/stdout using newline-delimited JSON.

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

use crate::protocol::{JsonRpcRequest, JsonRpcResponse};

/// Manages a Deno subprocess communicating via JSON-RPC over stdin/stdout.
pub struct DenoBridge {
    child: Child,
    stdin: tokio::process::ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl DenoBridge {
    /// Spawn a Deno subprocess running the given script path.
    pub async fn spawn(
        script_path: &str,
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut child = Command::new("deno")
            .arg("run")
            .arg("--allow-read")
            .arg("--allow-write")
            .arg("--allow-net")
            .arg(script_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| {
                Box::new(std::io::Error::other(format!(
                    "failed to spawn deno for {script_path}: {e}"
                )))
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Box::new(std::io::Error::other("failed to capture deno stdin")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Box::new(std::io::Error::other("failed to capture deno stdout")))?;

        Ok(Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            next_id: 1,
        })
    }

    /// Send a JSON-RPC request and wait for a response.
    pub async fn call(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> std::result::Result<JsonRpcResponse, Box<dyn std::error::Error + Send + Sync>> {
        let id = self.next_id;
        self.next_id += 1;

        let request = JsonRpcRequest::new(method, params, id);
        let mut line = serde_json::to_string(&request)?;
        line.push('\n');

        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;

        let mut response_line = String::new();
        let bytes_read = self.reader.read_line(&mut response_line).await?;
        if bytes_read == 0 {
            return Err(Box::new(std::io::Error::other(
                "deno subprocess closed stdout unexpectedly",
            )));
        }

        let response: JsonRpcResponse = serde_json::from_str(response_line.trim())?;

        if response.id != id {
            return Err(Box::new(std::io::Error::other(format!(
                "JSON-RPC id mismatch: expected {id}, got {}",
                response.id
            ))));
        }

        Ok(response)
    }

    /// Send a shutdown request and wait for the process to exit.
    pub async fn shutdown(
        &mut self,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Try to send shutdown, ignore errors if process already exited
        let _ = self.call(crate::protocol::METHOD_SHUTDOWN, None).await;
        let _ = self.child.wait().await;
        Ok(())
    }
}
