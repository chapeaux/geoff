use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

use camino::Utf8Path;

/// Manages a Deno SSR worker subprocess that renders web components
/// to declarative shadow DOM HTML via JSON-RPC over stdin/stdout.
pub struct SsrWorker {
    child: Child,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
    next_id: u64,
    cache: HashMap<String, SsrResult>,
}

/// The result of server-side rendering a web component.
#[derive(Clone, Debug)]
pub struct SsrResult {
    /// The rendered inner HTML (shadow DOM content).
    pub html: String,
    /// Whether the component uses a shadow root.
    pub has_shadow_root: bool,
}

impl SsrWorker {
    /// Spawn the Deno SSR worker at the given script path.
    pub fn spawn(worker_path: &Utf8Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut child = Command::new("deno")
            .arg("run")
            .arg("--allow-read")
            .arg("--allow-net")
            .arg(worker_path.as_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().ok_or("Failed to open stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to open stdout")?;

        Ok(Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            next_id: 1,
            cache: HashMap::new(),
        })
    }

    /// Render a web component via the SSR worker, returning the shadow DOM HTML.
    ///
    /// Results are cached by `(tag_name, attributes, children)` for the lifetime
    /// of this worker instance.
    pub fn render_component(
        &mut self,
        script_path: &str,
        tag_name: &str,
        attributes: &HashMap<String, String>,
        children: &str,
        renderer: Option<&str>,
    ) -> Result<SsrResult, Box<dyn std::error::Error>> {
        // Check cache
        let renderer_str = renderer.unwrap_or("linkedom");
        let cache_key = format!(
            "{}:{}:{}:{}",
            tag_name,
            serde_json::to_string(attributes)?,
            children,
            renderer_str
        );
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        let id = self.next_id;
        self.next_id += 1;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "render_component",
            "params": {
                "scriptPath": format!("file://{}", script_path),
                "tagName": tag_name,
                "attributes": attributes,
                "children": children,
                "renderer": renderer_str,
            },
            "id": id,
        });

        // Send request
        writeln!(self.stdin, "{}", serde_json::to_string(&request)?)?;
        self.stdin.flush()?;

        // Read response
        let mut line = String::new();
        self.reader.read_line(&mut line)?;

        let response: serde_json::Value = serde_json::from_str(&line)?;

        if let Some(error) = response.get("error") {
            return Err(format!("SSR error: {error}").into());
        }

        let result = response.get("result").ok_or("No result in response")?;
        let ssr_result = SsrResult {
            html: result
                .get("html")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            has_shadow_root: result
                .get("hasShadowRoot")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        };

        // Cache the result
        self.cache.insert(cache_key, ssr_result.clone());

        Ok(ssr_result)
    }

    /// Gracefully shut down the worker process.
    pub fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0", "method": "shutdown", "id": 0
        });
        let _ = writeln!(self.stdin, "{}", serde_json::to_string(&request)?);
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for SsrWorker {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
