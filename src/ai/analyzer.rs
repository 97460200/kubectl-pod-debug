use crate::error::{PodDebugError, Result};
use super::client::AiClient;

pub struct AiAnalyzer {
    client: AiClient,
}

impl AiAnalyzer {
    pub fn new(endpoint: String, api_key: Option<String>, model: String) -> Self {
        Self {
            client: AiClient::new(endpoint, api_key, model),
        }
    }
    
    pub async fn diagnose(&self, context: &str) -> Result<String> {
        let system_prompt = r#"你是一个 Kubernetes 调试专家。请分析以下 Pod 的诊断数据，找出可能的问题并提供修复建议。

请按以下格式输出:
## 诊断结论
{conclusion}

## 可能原因
{causes}

## 修复建议
{fixes}

只输出上述格式的内容，不要输出其他内容。"#;

        self.client.chat(system_prompt, context).await
    }
}
