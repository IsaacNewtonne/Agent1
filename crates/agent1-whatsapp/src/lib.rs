use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};

const SIDECAR_BASE: &str = "http://127.0.0.1:17372";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppStatus {
    pub state: String,
    pub phone: Option<String>,
    pub qr: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub to: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub success: bool,
    pub message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyRequest {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub text: String,
}

pub struct WhatsAppService {
    http: Client,
    event_tx: broadcast::Sender<WhatsAppEvent>,
    status: Arc<RwLock<WhatsAppStatus>>,
    pending_commands: mpsc::Sender<IncomingCommand>,
}

#[derive(Debug, Clone)]
pub enum WhatsAppEvent {
    QrReceived { qr_svg: String },
    Connected { phone: String },
    Disconnected,
    MessageReceived { from: String, body: String },
    Error { message: String },
}

#[derive(Debug, Clone)]
pub struct IncomingCommand {
    pub text: String,
}

impl WhatsAppService {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);
        let (pending_commands, _) = mpsc::channel(100);

        Self {
            http: Client::new(),
            event_tx,
            status: Arc::new(RwLock::new(WhatsAppStatus {
                state: "disconnected".to_string(),
                phone: None,
                qr: None,
                error: None,
            })),
            pending_commands,
        }
    }

    pub fn with_commands_channel(pending_commands: mpsc::Sender<IncomingCommand>) -> Self {
        let (event_tx, _) = broadcast::channel(100);

        Self {
            http: Client::new(),
            event_tx,
            status: Arc::new(RwLock::new(WhatsAppStatus {
                state: "disconnected".to_string(),
                phone: None,
                qr: None,
                error: None,
            })),
            pending_commands,
        }
    }

    pub async fn get_status(&self) -> Result<WhatsAppStatus> {
        let resp = self
            .http
            .get(format!("{}/status", SIDECAR_BASE))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let status: WhatsAppStatus = r.json().await?;
                let mut current = self.status.write().await;
                *current = status.clone();
                Ok(status)
            }
            Ok(r) => {
                if r.status().as_u16() == 404 {
                    return Ok(WhatsAppStatus {
                        state: "sidecar_not_found".to_string(),
                        phone: None,
                        qr: None,
                        error: Some("WhatsApp sidecar route /status was not found".to_string()),
                    });
                }
                Ok(WhatsAppStatus {
                    state: "error".to_string(),
                    phone: None,
                    qr: None,
                    error: Some(format!("WhatsApp sidecar returned HTTP {}", r.status())),
                })
            }
            Err(_) => Ok(WhatsAppStatus {
                state: "sidecar_offline".to_string(),
                phone: None,
                qr: None,
                error: Some("WhatsApp sidecar is not reachable on 127.0.0.1:17372".to_string()),
            }),
        }
    }

    pub async fn get_qr_svg(&self) -> Result<Option<String>> {
        let resp = self
            .http
            .get(format!("{}/qrsvg", SIDECAR_BASE))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let svg = r.text().await?;
                Ok(Some(svg))
            }
            _ => Ok(None),
        }
    }

    pub async fn connect(&self) -> Result<()> {
        let resp = self
            .http
            .post(format!("{}/connect", SIDECAR_BASE))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if resp.status().is_success() {
            let mut status = self.status.write().await;
            status.state = "connecting".to_string();
        }

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.http
            .post(format!("{}/disconnect", SIDECAR_BASE))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        let mut status = self.status.write().await;
        *status = WhatsAppStatus {
            state: "disconnected".to_string(),
            phone: None,
            qr: None,
            error: None,
        };

        Ok(())
    }

    pub async fn reset(&self) -> Result<()> {
        self.http
            .post(format!("{}/reset", SIDECAR_BASE))
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await?;

        let mut status = self.status.write().await;
        *status = WhatsAppStatus {
            state: "disconnected".to_string(),
            phone: None,
            qr: None,
            error: None,
        };

        Ok(())
    }

    pub async fn send_message(&self, to: &str, text: &str) -> Result<SendMessageResponse> {
        let resp = self
            .http
            .post(format!("{}/send", SIDECAR_BASE))
            .json(&SendMessageRequest {
                to: to.to_string(),
                text: text.to_string(),
            })
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        let result: SendMessageResponse = resp.json().await?;
        Ok(result)
    }

    pub async fn send_notification(&self, title: &str, body: &str) -> Result<()> {
        let resp = self
            .http
            .post(format!("{}/notify", SIDECAR_BASE))
            .json(&NotifyRequest {
                title: title.to_string(),
                body: body.to_string(),
            })
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !resp.status().is_success() {
            let err: serde_json::Value = resp.json().await?;
            anyhow::bail!("Notify failed: {:?}", err);
        }

        Ok(())
    }

    pub async fn send_approval(&self, message: &str) -> Result<()> {
        let resp = self
            .http
            .post(format!("{}/approve", SIDECAR_BASE))
            .json(&serde_json::json!({ "message": message }))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !resp.status().is_success() {
            let err: serde_json::Value = resp.json().await?;
            anyhow::bail!("Approval send failed: {:?}", err);
        }

        Ok(())
    }

    pub async fn handle_incoming_command(&self, text: &str) -> Result<()> {
        self.pending_commands
            .send(IncomingCommand {
                text: text.to_string(),
            })
            .await?;
        Ok(())
    }

    pub async fn send_command_to_sidecar(&self, text: &str) -> Result<()> {
        let resp = self
            .http
            .post(format!("{}/command", SIDECAR_BASE))
            .json(&CommandRequest {
                text: text.to_string(),
            })
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        if !resp.status().is_success() {
            let err: serde_json::Value = resp.json().await?;
            anyhow::bail!("Command failed: {:?}", err);
        }

        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WhatsAppEvent> {
        self.event_tx.subscribe()
    }

    pub async fn poll_for_updates(&self) -> Result<()> {
        let status = self.get_status().await?;

        match status.state.as_str() {
            "connected" => {
                let current = self.status.read().await;
                if current.state != "connected" {
                    let phone = status.phone.clone().unwrap_or_default();
                    let _ = self.event_tx.send(WhatsAppEvent::Connected { phone });
                }
            }
            "qr_ready" => {
                if let Ok(Some(svg)) = self.get_qr_svg().await {
                    let current = self.status.read().await;
                    if current.qr.is_none() {
                        let _ = self
                            .event_tx
                            .send(WhatsAppEvent::QrReceived { qr_svg: svg });
                    }
                }
            }
            "disconnected" => {
                let mut current = self.status.write().await;
                if current.state == "connected" || current.state == "connecting" {
                    let _ = self.event_tx.send(WhatsAppEvent::Disconnected);
                }
                *current = status;
            }
            _ => {}
        }

        Ok(())
    }
}

impl Default for WhatsAppService {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for WhatsAppService {
    fn clone(&self) -> Self {
        Self {
            http: self.http.clone(),
            event_tx: self.event_tx.clone(),
            status: self.status.clone(),
            pending_commands: self.pending_commands.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_whatsapp_service_creation() {
        let service = super::WhatsAppService::new();
        assert_eq!(service.event_tx.receiver_count(), 0);
    }

    #[tokio::test]
    async fn test_status_when_sidecar_offline() {
        let service = super::WhatsAppService::new();
        let status = service.get_status().await.unwrap();
        assert_eq!(status.state, "sidecar_offline");
    }
}
