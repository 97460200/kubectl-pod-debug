use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use crate::error::{PodDebugError, Result};

#[derive(Clone)]
pub struct AiClient {
    client: Client,
    endpoint: String,
    api_key: Option<String>,
    model: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

impl AiClient {
    pub fn new(endpoint: String, api_key: Option<String>, model: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            client,
            endpoint,
            api_key,
            model,
        }
    }
    
    pub async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                },
            ],
            temperature: 0.7,
        };
        
        let mut req_builder = self.client
            .post(format!("{}/chat/completions", self.endpoint))
            .header("Content-Type", "application/json")
            .json(&request);
        
        if let Some(ref key) = self.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
        }
        
        let response = req_builder
            .send()
            .await
            .map_err(|e| PodDebugError::AiApiError {
                reason: format!("Request failed: {}", e),
            })?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(PodDebugError::AiApiError {
                reason: format!("API error {}: {}", status, body),
            });
        }
        
        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| PodDebugError::AiApiError {
                reason: format!("Failed to parse response: {}", e),
            })?;
        
        Ok(chat_response.choices.first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }
}
