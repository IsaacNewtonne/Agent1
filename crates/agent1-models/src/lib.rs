use agent1_core::{
    Agent1Error, ChatMessage, ChatRequest, ChatResponse, ChatStreamResponse, ModelConfig,
    ModelInfo, Result,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::process::Command;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStreamResponse> {
        let response = self.chat(request).await?;
        Ok(ChatStreamResponse {
            content: response.content.clone(),
            chunks: vec![response.content],
        })
    }
    async fn list_models(&self, config: &ModelConfig) -> Result<Vec<ModelInfo>>;
}

pub fn provider_for(config: &ModelConfig) -> Result<Box<dyn ModelProvider>> {
    match config.provider.as_str() {
        "mock" => Ok(Box::new(MockProvider)),
        "ollama" => Ok(Box::new(OllamaProvider::new())),
        "opencode" => Ok(Box::new(OpenCodeProvider)),
        "openai_compatible" | "openai-compatible" | "local_openai" => {
            Ok(Box::new(OpenAiCompatibleProvider::new()))
        }
        other => Err(Agent1Error::Config(format!(
            "unsupported model provider `{other}`"
        ))),
    }
}

pub struct OpenCodeProvider;

#[async_trait]
impl ModelProvider for OpenCodeProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let prompt = request
            .messages
            .iter()
            .map(|message| format!("{}: {}", message.role, message.content))
            .collect::<Vec<_>>()
            .join("\n\n");
        let output = Command::new(opencode_command())
            .args(["run", "--model", &request.model.model, "--format", "json", &prompt])
            .output()
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!(
                    "opencode executable unavailable; install opencode or ensure it is on PATH: {err}"
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(Agent1Error::Runtime(format!(
                "opencode run failed: {}",
                if stderr.is_empty() {
                    format!("exit status {}", output.status)
                } else {
                    stderr
                }
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let content = extract_opencode_content(&stdout).unwrap_or_else(|| stdout.trim().to_string());
        if content.is_empty() {
            return Err(Agent1Error::InvalidModelResponse(
                "opencode returned no content".to_string(),
            ));
        }
        Ok(ChatResponse { content })
    }

    async fn list_models(&self, _config: &ModelConfig) -> Result<Vec<ModelInfo>> {
        let output = Command::new(opencode_command())
            .arg("models")
            .output()
            .await
            .map_err(|err| {
                Agent1Error::Runtime(format!(
                    "opencode executable unavailable; install opencode or ensure it is on PATH: {err}"
                ))
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(Agent1Error::Runtime(format!(
                "opencode models failed: {}",
                if stderr.is_empty() {
                    format!("exit status {}", output.status)
                } else {
                    stderr
                }
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|name| ModelInfo {
                provider: "opencode".to_string(),
                name: name.to_string(),
            })
            .collect())
    }
}

fn opencode_command() -> &'static str {
    if cfg!(windows) {
        "opencode.cmd"
    } else {
        "opencode"
    }
}

fn extract_opencode_content(stdout: &str) -> Option<String> {
    let mut parts = Vec::new();
    for line in stdout.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        collect_content_fields(&value, &mut parts);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}

fn collect_content_fields(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for key in ["content", "text", "delta"] {
                if let Some(text) = map.get(key).and_then(Value::as_str) {
                    if !text.is_empty() {
                        parts.push(text.to_string());
                    }
                }
            }
            for nested in map.values() {
                collect_content_fields(nested, parts);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_content_fields(item, parts);
            }
        }
        _ => {}
    }
}

pub struct MockProvider;

#[async_trait]
impl ModelProvider for MockProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        if request.model.model == "repeat_tool" {
            return Ok(ChatResponse {
                content: r#"{"tool_call":{"name":"file_list","input":{"path":"."}}}"#.to_string(),
            });
        }
        if request.model.model == "mcp_disabled" {
            return Ok(ChatResponse {
                content: r#"{"tool_call":{"name":"mcp_call","input":{"server":"disabled","tool":"ping","input":{}}}}"#.to_string(),
            });
        }
        if request
            .messages
            .iter()
            .any(|message| message.content.starts_with("Tool result."))
        {
            return Ok(ChatResponse {
                content: r#"{"final":"mock observed tool result"}"#.to_string(),
            });
        }
        let content = match request.model.model.as_str() {
            "tool_file_list" => r#"{"tool_call":{"name":"file_list","input":{"path":"."}}}"#,
            "malformed_json" => "{not-json",
            other => {
                return Ok(ChatResponse {
                    content: format!(r#"{{"final":"mock final from {other}"}}"#),
                });
            }
        };
        Ok(ChatResponse {
            content: content.to_string(),
        })
    }

    async fn list_models(&self, _config: &ModelConfig) -> Result<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                provider: "mock".to_string(),
                name: "final".to_string(),
            },
            ModelInfo {
                provider: "mock".to_string(),
                name: "tool_file_list".to_string(),
            },
        ])
    }
}

pub struct OllamaProvider {
    client: Client,
}

impl OllamaProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let base_url = request
            .model
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let response = self
            .client
            .post(format!("{base_url}/api/chat"))
            .json(&json!({
                "model": request.model.model,
                "messages": request.messages,
                "stream": false,
                "options": {
                    "temperature": request.model.temperature,
                    "num_ctx": request.model.context_window
                }
            }))
            .send()
            .await
            .map_err(|err| map_request_error("ollama chat request", err))?
            .error_for_status()
            .map_err(|err| Agent1Error::Runtime(format!("ollama chat returned an error: {err}")))?;
        let body: OllamaChatResponse = response.json().await.map_err(|err| {
            Agent1Error::InvalidModelResponse(format!("ollama response was not JSON: {err}"))
        })?;
        Ok(ChatResponse {
            content: body.message.content,
        })
    }

    async fn list_models(&self, config: &ModelConfig) -> Result<Vec<ModelInfo>> {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let response = self
            .client
            .get(format!("{base_url}/api/tags"))
            .send()
            .await
            .map_err(|err| map_request_error("ollama list models request", err))?
            .error_for_status()
            .map_err(|err| {
                Agent1Error::Runtime(format!("ollama list models returned an error: {err}"))
            })?;
        let body: OllamaTagsResponse = response.json().await.map_err(|err| {
            Agent1Error::InvalidModelResponse(format!("ollama tags response was not JSON: {err}"))
        })?;
        Ok(body
            .models
            .into_iter()
            .map(|model| ModelInfo {
                provider: "ollama".to_string(),
                name: model.name,
            })
            .collect())
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStreamResponse> {
        let base_url = request
            .model
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let response = self
            .client
            .post(format!("{base_url}/api/chat"))
            .json(&json!({
                "model": request.model.model,
                "messages": request.messages,
                "stream": true,
                "options": {
                    "temperature": request.model.temperature,
                    "num_ctx": request.model.context_window
                }
            }))
            .send()
            .await
            .map_err(|err| map_request_error("ollama chat stream request", err))?
            .error_for_status()
            .map_err(|err| {
                Agent1Error::Runtime(format!("ollama chat stream returned an error: {err}"))
            })?;
        let mut stream = response.bytes_stream();
        let mut pending = String::new();
        let mut chunks = Vec::new();
        while let Some(item) = stream.next().await {
            let bytes = item
                .map_err(|err| Agent1Error::Runtime(format!("ollama stream read failed: {err}")))?;
            pending.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(index) = pending.find('\n') {
                let line = pending[..index].trim().to_string();
                pending = pending[index + 1..].to_string();
                if line.is_empty() {
                    continue;
                }
                let packet: OllamaStreamPacket = serde_json::from_str(&line).map_err(|err| {
                    Agent1Error::InvalidModelResponse(format!(
                        "ollama stream packet was not JSON: {err}"
                    ))
                })?;
                if packet.done {
                    continue;
                }
                if let Some(message) = packet.message {
                    if !message.content.is_empty() {
                        chunks.push(message.content);
                    }
                }
            }
        }
        if !pending.trim().is_empty() {
            let packet: OllamaStreamPacket =
                serde_json::from_str(pending.trim()).map_err(|err| {
                    Agent1Error::InvalidModelResponse(format!(
                        "ollama trailing stream packet was not JSON: {err}"
                    ))
                })?;
            if !packet.done {
            if let Some(message) = packet.message {
                if !message.content.is_empty() {
                    chunks.push(message.content);
                }
            }
        }
        }
        if chunks.is_empty() {
            return Err(Agent1Error::InvalidModelResponse(
                "ollama stream returned no content chunks".to_string(),
            ));
        }
        let content = chunks.join("");
        Ok(ChatStreamResponse { content, chunks })
    }
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamPacket {
    #[serde(default)]
    message: Option<OllamaMessage>,
    #[serde(default)]
    done: bool,
}

pub struct OpenAiCompatibleProvider {
    client: Client,
}

impl OpenAiCompatibleProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for OpenAiCompatibleProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiDelta {
    content: Option<String>,
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let base_url = request
            .model
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:8000/v1".to_string());
        let response = self
            .client
            .post(format!("{base_url}/chat/completions"))
            .json(&OpenAiChatRequest {
                model: request.model.model,
                messages: request.messages,
                temperature: Some(request.model.temperature as f64),
                max_tokens: request.model.max_tokens.map(|m| m as i64),
                stream: None,
            })
            .send()
            .await
            .map_err(|err| map_request_error("OpenAI-compatible chat request", err))?
            .error_for_status()
            .map_err(|err| {
                Agent1Error::Runtime(format!("OpenAI-compatible chat returned an error: {err}"))
            })?;
        let body: OpenAiChatResponse = response.json().await.map_err(|err| {
            Agent1Error::InvalidModelResponse(format!(
                "OpenAI-compatible response was not JSON: {err}"
            ))
        })?;
        let content = body
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| {
                Agent1Error::InvalidModelResponse("missing choices[0].message.content".to_string())
            })?;
        Ok(ChatResponse { content })
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStreamResponse> {
        let base_url = request
            .model
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:8000/v1".to_string());
        let response = self
            .client
            .post(format!("{base_url}/chat/completions"))
            .json(&OpenAiChatRequest {
                model: request.model.model,
                messages: request.messages,
                temperature: Some(request.model.temperature as f64),
                max_tokens: request.model.max_tokens.map(|m| m as i64),
                stream: Some(true),
            })
            .send()
            .await
            .map_err(|err| map_request_error("OpenAI-compatible chat stream request", err))?
            .error_for_status()
            .map_err(|err| {
                Agent1Error::Runtime(format!("OpenAI-compatible stream returned an error: {err}"))
            })?;
        let mut stream = response.bytes_stream();
        let mut pending = String::new();
        let mut chunks = Vec::new();
        while let Some(item) = stream.next().await {
            let bytes = item
                .map_err(|err| Agent1Error::Runtime(format!("OpenAI stream read failed: {err}")))?;
            pending.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(index) = pending.find('\n') {
                let line = pending[..index].trim().to_string();
                pending = pending[index + 1..].to_string();
                if line.is_empty() || line == "data: [DONE]" {
                    continue;
                }
                if let Some(data) = line.strip_prefix("data: ") {
                    let chunk: OpenAiStreamChunk = match serde_json::from_str(data) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    let Some(choice) = chunk.choices.into_iter().next() else { continue };
                        if choice.finish_reason.is_none() {
                            if let Some(content) = choice.delta.content {
                                if !content.is_empty() {
                                    chunks.push(content);
                                }
                            }
                        }
                }
            }
        }
        let content = chunks.join("");
        Ok(ChatStreamResponse { content, chunks })
    }

    async fn list_models(&self, config: &ModelConfig) -> Result<Vec<ModelInfo>> {
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:8000/v1".to_string());
        let response = self
            .client
            .get(format!("{base_url}/models"))
            .send()
            .await
            .map_err(|err| map_request_error("OpenAI-compatible list models request", err))?
            .error_for_status()
            .map_err(|err| {
                Agent1Error::Runtime(format!(
                    "OpenAI-compatible list models returned an error: {err}"
                ))
            })?;
        let body: OpenAiModelsResponse = response.json().await.map_err(|err| {
            Agent1Error::InvalidModelResponse(format!(
                "OpenAI-compatible models response was not JSON: {err}"
            ))
        })?;
        Ok(body
            .data
            .into_iter()
            .map(|model| ModelInfo {
                provider: "openai_compatible".to_string(),
                name: model.id,
            })
            .collect())
    }
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: Option<f64>,
    max_tokens: Option<i64>,
    stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

fn map_request_error(context: &str, err: reqwest::Error) -> Agent1Error {
    if err.is_timeout() {
        return Agent1Error::Runtime(format!("{context} timed out"));
    }
    if err.is_connect() {
        return Agent1Error::Runtime(format!("{context} endpoint unavailable: {err}"));
    }
    Agent1Error::Runtime(format!("{context} failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    async fn spawn_http_server<F>(handler: F) -> (String, JoinHandle<()>)
    where
        F: Fn(String) -> String + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("server addr");
        let handler = std::sync::Arc::new(handler);
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let mut buffer = vec![0_u8; 16 * 1024];
            let read = stream.read(&mut buffer).await.expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let response = handler(request);
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        });
        (format!("http://{addr}"), task)
    }

    fn http_json_response(status: u16, body: &str) -> String {
        let status_text = match status {
            200 => "OK",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "OK",
        };
        format!(
            "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn test_ollama_config(base_url: Option<String>) -> ModelConfig {
        ModelConfig {
            provider: "ollama".to_string(),
            model: "qwen2.5:7b".to_string(),
            base_url,
            context_window: 8192,
            temperature: 0.2,
            top_p: None,
            max_tokens: None,
        }
    }

    #[tokio::test]
    async fn ollama_list_models_works() {
        let (base_url, server) = spawn_http_server(|request| {
            assert!(request.starts_with("GET /api/tags HTTP/1.1"));
            http_json_response(
                200,
                r#"{"models":[{"name":"qwen2.5:7b"},{"name":"phi4-mini"}]}"#,
            )
        })
        .await;
        let provider = OllamaProvider::new();
        let models = provider
            .list_models(&test_ollama_config(Some(base_url)))
            .await
            .expect("list models should succeed");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].provider, "ollama");
        assert_eq!(models[0].name, "qwen2.5:7b");
        assert_eq!(models[1].name, "phi4-mini");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn ollama_chat_works() {
        let (base_url, server) = spawn_http_server(|request| {
            assert!(request.starts_with("POST /api/chat HTTP/1.1"));
            assert!(request.contains(r#""stream":false"#));
            http_json_response(200, r#"{"message":{"content":"hello from ollama"}}"#)
        })
        .await;
        let provider = OllamaProvider::new();
        let response = provider
            .chat(ChatRequest {
                model: test_ollama_config(Some(base_url)),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "hi".to_string(),
                }],
            })
            .await
            .expect("chat should succeed");
        assert_eq!(response.content, "hello from ollama");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn ollama_chat_stream_works() {
        let (base_url, server) = spawn_http_server(|request| {
            assert!(request.starts_with("POST /api/chat HTTP/1.1"));
            assert!(request.contains(r#""stream":true"#));
            let body = "{\"message\":{\"content\":\"hello \"},\"done\":false}\n{\"message\":{\"content\":\"stream\"},\"done\":false}\n{\"done\":true}\n";
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
        })
        .await;
        let provider = OllamaProvider::new();
        let response = provider
            .chat_stream(ChatRequest {
                model: test_ollama_config(Some(base_url)),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "hi".to_string(),
                }],
            })
            .await
            .expect("stream should succeed");
        assert_eq!(
            response.chunks,
            vec!["hello ".to_string(), "stream".to_string()]
        );
        assert_eq!(response.content, "hello stream");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn endpoint_unavailable_returns_clear_error() {
        let provider = OllamaProvider::new();
        let error = provider
            .list_models(&test_ollama_config(Some("http://127.0.0.1:9".to_string())))
            .await
            .expect_err("endpoint should be unavailable")
            .to_string();
        assert!(error.contains("endpoint unavailable"));
    }

    #[tokio::test]
    async fn timeout_returns_structured_error() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind timeout server");
        let addr = listener.local_addr().expect("timeout addr");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept timeout request");
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer).await;
            tokio::time::sleep(Duration::from_millis(250)).await;
            let _ = stream
                .write_all(http_json_response(200, r#"{"models":[]}"#).as_bytes())
                .await;
        });
        let client = Client::builder()
            .timeout(Duration::from_millis(50))
            .build()
            .expect("client with timeout");
        let provider = OllamaProvider { client };
        let error = provider
            .list_models(&test_ollama_config(Some(format!("http://{addr}"))))
            .await
            .expect_err("request should time out")
            .to_string();
        assert!(error.contains("timed out"));
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn openai_compatible_mock_endpoint_works() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind openai mock");
        let addr = listener.local_addr().expect("openai addr");
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.expect("accept openai request");
                let mut buffer = vec![0_u8; 16 * 1024];
                let read = stream.read(&mut buffer).await.expect("read openai request");
                let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                let body = if request.starts_with("GET /v1/models HTTP/1.1") {
                    r#"{"data":[{"id":"local-1"},{"id":"local-2"}]}"#
                } else {
                    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
                    r#"{"choices":[{"message":{"content":"openai-compatible ok"}}]}"#
                };
                let response = http_json_response(200, body);
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("write openai response");
            }
        });
        let provider = OpenAiCompatibleProvider::new();
        let config = ModelConfig {
            provider: "openai_compatible".to_string(),
            model: "local-1".to_string(),
            base_url: Some(format!("http://{addr}/v1")),
            context_window: 8192,
            temperature: 0.2,
            top_p: None,
            max_tokens: None,
        };
        let models = provider
            .list_models(&config)
            .await
            .expect("openai-compatible list models should succeed");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].provider, "openai_compatible");
        assert_eq!(models[0].name, "local-1");
        let response = provider
            .chat(ChatRequest {
                model: config,
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                }],
            })
            .await
            .expect("openai-compatible chat should succeed");
        assert_eq!(response.content, "openai-compatible ok");
        server.await.expect("server task");
    }
}
