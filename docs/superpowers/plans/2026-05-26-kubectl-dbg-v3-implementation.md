# kubectl-dbg v3.0.0 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 实现 kubectl-dbg v3.0.0 新功能：AI 智能诊断、时间旅行调试、Pod 配置对比

**架构：** 
- 新增 `ai/` 模块处理 OpenAI 兼容 API 调用
- 新增 `timeline/` 模块收集和展示 Pod 事件
- 新增 `diff/` 模块对比 Pod 和 ReplicaSet 配置
- 更新 CLI 参数和主程序逻辑

**技术栈：** Rust, reqwest, tokio, serde, kube

---

## 文件清单

### 新建文件
- `src/ai/mod.rs` - AI 模块入口
- `src/ai/client.rs` - OpenAI 兼容 API 客户端
- `src/ai/analyzer.rs` - 诊断数据分析
- `src/timeline/mod.rs` - 时间旅行模块入口
- `src/timeline/events.rs` - 事件收集
- `src/timeline/formatter.rs` - 时间线格式化
- `src/diff/mod.rs` - 配置对比模块入口
- `src/diff/collector.rs` - 配置收集
- `src/diff/comparator.rs` - 配置对比

### 修改文件
- `Cargo.toml` - 添加 reqwest 依赖
- `src/cli.rs` - 添加新参数
- `src/main.rs` - 集成新功能
- `install.sh` - 更新工具名称
- `.github/workflows/release.yml` - 更新 artifact 名称
- `README.md` - 更新文档
- `README_zh.md` - 更新中文文档
- `src/error.rs` - 添加新的错误类型

---

## 任务列表

### 任务 1：更新 Cargo.toml 添加依赖

**文件：**
- 修改：`Cargo.toml`

- [ ] **步骤 1：添加 reqwest 依赖**

编辑 `Cargo.toml` 在 `[dependencies]` 部分添加：

```toml
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

---

### 任务 2：更新 src/error.rs 添加错误类型

**文件：**
- 修改：`src/error.rs`

- [ ] **步骤 1：添加 AI 和 Timeline 相关错误**

在 `PodDebugError` 枚举中添加：

```rust
#[derive(Debug, thiserror::Error)]
pub enum PodDebugError {
    #[error("AI API error: {reason}")]
    AiApiError { reason: String },
    
    #[error("AI timeout: {reason}")]
    AiTimeout { reason: String },
    
    #[error("Timeline error: {reason}")]
    TimelineError { reason: String },
    
    #[error("Config diff error: {reason}")]
    DiffError { reason: String },
    
    // ... existing errors
}
```

---

### 任务 3：创建 AI 模块

**文件：**
- 创建：`src/ai/mod.rs`
- 创建：`src/ai/client.rs`
- 创建：`src/ai/analyzer.rs`

- [ ] **步骤 1：创建 src/ai/mod.rs**

```rust
pub mod client;
pub mod analyzer;

pub use client::AiClient;
pub use analyzer::AiAnalyzer;
```

- [ ] **步骤 2：创建 src/ai/client.rs**

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

#[derive(Serialize)]
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
    
    pub async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String, crate::error::PodDebugError> {
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
            .header("Content-Type", "application/json");
        
        if let Some(ref key) = self.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
        }
        
        let response = req_builder
            .json(&request)
            .send()
            .await
            .map_err(|e| crate::error::PodDebugError::AiApiError {
                reason: format!("Request failed: {}", e),
            })?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::PodDebugError::AiApiError {
                reason: format!("API error {}: {}", status, body),
            });
        }
        
        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| crate::error::PodDebugError::AiApiError {
                reason: format!("Failed to parse response: {}", e),
            })?;
        
        Ok(chat_response.choices.first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }
}
```

- [ ] **步骤 3：创建 src/ai/analyzer.rs**

```rust
use crate::error::PodDebugError;
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
    
    pub async fn diagnose(&self, context: &str) -> Result<String, PodDebugError> {
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
```

---

### 任务 4：创建 Timeline 模块

**文件：**
- 创建：`src/timeline/mod.rs`
- 创建：`src/timeline/events.rs`
- 创建：`src/timeline/formatter.rs`

- [ ] **步骤 1：创建 src/timeline/mod.rs**

```rust
pub mod events;
pub mod formatter;

pub use events::TimelineCollector;
pub use formatter::TimelineFormatter;
```

- [ ] **步骤 2：创建 src/timeline/events.rs**

```rust
use kube::Client;
use k8s_openapi::api::core::v1::Pod;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ContainerRestart {
    pub count: u32,
    pub last_restart: Option<DateTime<Utc>>,
    pub restart_history: Vec<RestartEntry>,
}

#[derive(Debug, Clone)]
pub struct RestartEntry {
    pub timestamp: DateTime<Utc>,
    pub exit_code: i32,
    pub reason: Option<String>,
}

pub struct TimelineCollector {
    kube_client: Client,
}

impl TimelineCollector {
    pub fn new(kube_client: Client) -> Self {
        Self { kube_client }
    }
    
    pub async fn collect_events(&self, pod_name: &str, namespace: &str, since: Duration) -> Result<Vec<TimelineEvent>, crate::error::PodDebugError> {
        let api: kube::Api<Pod> = kube::Api::namespaced(self.kube_client.clone(), namespace);
        
        let pod = api.get(pod_name).await
            .map_err(|e| crate::error::PodDebugError::TimelineError {
                reason: format!("Failed to get pod: {}", e),
            })?;
        
        let mut events = Vec::new();
        
        // Add creation timestamp
        if let Some(ts) = pod.metadata.creation_timestamp {
            events.push(TimelineEvent {
                timestamp: ts.0,
                event_type: "Created".to_string(),
                message: "Pod created".to_string(),
                reason: None,
            });
        }
        
        // Collect container restart events from status
        if let Some(status) = &pod.status {
            if let Some(restart_count) = status.container_statuses.as_ref().and_then(|cs| cs.first()).map(|s| s.restart_count) {
                if restart_count > 0 {
                    events.push(TimelineEvent {
                        timestamp: Utc::now(),
                        event_type: "Restarted".to_string(),
                        message: format!("Container restarted {} times", restart_count),
                        reason: None,
                    });
                }
            }
        }
        
        Ok(events)
    }
    
    pub fn collect_restart_info(&self, pod: &Pod) -> ContainerRestart {
        let mut restart_history = Vec::new();
        let mut total_restarts = 0u32;
        let mut last_restart = None::<DateTime<Utc>>;
        
        if let Some(status) = &pod.status {
            if let Some(container_statuses) = &status.container_statuses {
                for cs in container_statuses {
                    total_restarts += cs.restart_count;
                    if let Some(last_ts) = &cs.last_state.terminated {
                        if let Some(ts) = &last_ts.finished_at {
                            last_restart = Some(ts.0);
                        }
                    }
                }
            }
        }
        
        ContainerRestart {
            count: total_restarts,
            last_restart,
            restart_history,
        }
    }
}
```

- [ ] **步骤 3：创建 src/timeline/formatter.rs**

```rust
use super::events::{TimelineEvent, ContainerRestart};
use chrono::Local;

pub struct TimelineFormatter;

impl TimelineFormatter {
    pub fn format_events(pod_name: &str, namespace: &str, events: &[TimelineEvent]) -> String {
        let mut output = format!("=== Pod Timeline ({}/{})\n\n", pod_name, namespace);
        
        for event in events {
            let ts = event.timestamp.format("%Y-%m-%d %H:%M:%S");
            let icon = match event.event_type.as_str() {
                "Created" => "✅",
                "Scheduled" => "📍",
                "Pulling" => "🐳",
                "Started" => "🚀",
                "Ready" => "✅",
                "Warning" | "Failed" => "⚠️",
                "Restarted" => "🔄",
                _ => "📌",
            };
            output.push_str(&format!("{}  {}  {}\n", ts, icon, event.message));
        }
        
        output
    }
    
    pub fn format_restarts(restarts: &ContainerRestart) -> String {
        let mut output = String::from("\n=== Container Restarts ===\n\n");
        output.push_str(&format!("Total restarts: {}\n", restarts.count));
        
        if let Some(last) = restarts.last_restart {
            let ago = Local::now().signed_duration_since(last);
            output.push_str(&format!("Last restart: {} ({} ago)\n", 
                last.format("%Y-%m-%d %H:%M:%S"), 
                format_duration(ago)));
        }
        
        output
    }
}

fn format_duration(d: chrono::SignedDuration) -> String {
    let total_secs = d.num_seconds();
    if total_secs < 60 {
        format!("{} seconds", total_secs)
    } else if total_secs < 3600 {
        format!("{} minutes", total_secs / 60)
    } else if total_secs < 86400 {
        format!("{} hours", total_secs / 3600)
    } else {
        format!("{} days", total_secs / 86400)
    }
}
```

---

### 任务 5：创建 Diff 模块

**文件：**
- 创建：`src/diff/mod.rs`
- 创建：`src/diff/collector.rs`
- 创建：`src/diff/comparator.rs`

- [ ] **步骤 1：创建 src/diff/mod.rs**

```rust
pub mod collector;
pub mod comparator;

pub use collector::ConfigCollector;
pub use comparator::ConfigComparator;
```

- [ ] **步骤 2：创建 src/diff/collector.rs**

```rust
use kube::Client;
use k8s_openapi::api::apps::v1::ReplicaSet;
use k8s_openapi::api::core::v1::Pod;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PodConfig {
    pub image: Option<String>,
    pub resources_limits_cpu: Option<String>,
    pub resources_limits_memory: Option<String>,
    pub resources_requests_cpu: Option<String>,
    pub resources_requests_memory: Option<String>,
    pub env_vars: Vec<(String, String)>,
    pub volumes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RsConfig {
    pub name: String,
    pub replicas: i32,
    pub image: Option<String>,
    pub resources_limits_cpu: Option<String>,
    pub resources_limits_memory: Option<String>,
    pub resources_requests_cpu: Option<String>,
    pub resources_requests_memory: Option<String>,
    pub env_vars: Vec<(String, String)>,
    pub volumes: Vec<String>,
}

pub struct ConfigCollector {
    kube_client: Client,
}

impl ConfigCollector {
    pub fn new(kube_client: Client) -> Self {
        Self { kube_client }
    }
    
    pub async fn collect_pod_config(&self, pod: &Pod) -> PodConfig {
        let mut config = PodConfig::default();
        
        if let Some(spec) = &pod.spec {
            if let Some(containers) = spec.containers.first() {
                config.image = containers.image.clone();
                
                if let Some(resources) = &containers.resources {
                    if let Some(limits) = &resources.limits {
                        config.resources_limits_cpu = limits.get("cpu").map(|v| v.0.clone());
                        config.resources_limits_memory = limits.get("memory").map(|v| v.0.clone());
                    }
                    if let Some(requests) = &resources.requests {
                        config.resources_requests_cpu = requests.get("cpu").map(|v| v.0.clone());
                        config.resources_requests_memory = requests.get("memory").map(|v| v.0.clone());
                    }
                }
                
                if let Some(env) = &containers.env {
                    config.env_vars = env.iter()
                        .filter_map(|e| e.value.clone().map(|v| (e.name.clone(), v)))
                        .collect();
                }
            }
        }
        
        config
    }
    
    pub async fn find_replicaset(&self, pod: &Pod) -> Result<Option<(String, ReplicaSet)>, kube::Error> {
        let namespace = pod.metadata.namespace.as_ref().ok_or(kube::Error::RequestValidation)?;
        
        // Find owner reference
        if let Some(owner_refs) = &pod.metadata.owner_references {
            for owner in owner_refs {
                if owner.kind == "ReplicaSet" {
                    let api: kube::Api<ReplicaSet> = kube::Api::namespaced(self.kube_client.clone(), namespace);
                    if let Ok(rs) = api.get(&owner.name).await {
                        return Ok(Some((owner.name.clone(), rs)));
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    pub fn collect_rs_config(&self, rs: &ReplicaSet) -> RsConfig {
        let mut config = RsConfig {
            name: rs.metadata.name.clone().unwrap_or_default(),
            replicas: rs.spec.as_ref().and_then(|s| s.replicas).unwrap_or(0),
            ..Default::default()
        };
        
        if let Some(spec) = &rs.spec {
            if let Some(template) = &spec.template {
                if let Some(containers) = template.spec.as_ref().and_then(|s| s.containers.first()) {
                    config.image = containers.image.clone();
                    
                    if let Some(resources) = &containers.resources {
                        if let Some(limits) = &resources.limits {
                            config.resources_limits_cpu = limits.get("cpu").map(|v| v.0.clone());
                            config.resources_limits_memory = limits.get("memory").map(|v| v.0.clone());
                        }
                        if let Some(requests) = &resources.requests {
                            config.resources_requests_cpu = requests.get("cpu").map(|v| v.0.clone());
                            config.resources_requests_memory = requests.get("memory").map(|v| v.0.clone());
                        }
                    }
                    
                    if let Some(env) = &containers.env {
                        config.env_vars = env.iter()
                            .filter_map(|e| e.value.clone().map(|v| (e.name.clone(), v)))
                            .collect();
                    }
                }
            }
        }
        
        config
    }
}
```

- [ ] **步骤 3：创建 src/diff/comparator.rs**

```rust
use super::collector::{PodConfig, RsConfig};

#[derive(Debug)]
pub enum DiffLevel {
    Critical,  // 🔴
    Warning,   // ⚠️
    Match,     // ✅
}

#[derive(Debug)]
pub struct ConfigDiff {
    pub field: String,
    pub pod_value: String,
    pub rs_value: String,
    pub level: DiffLevel,
    pub impact: String,
}

pub struct ConfigComparator;

impl ConfigComparator {
    pub fn compare(pod_config: &PodConfig, rs_config: &RsConfig) -> Vec<ConfigDiff> {
        let mut diffs = Vec::new();
        
        // Compare image
        if let (Some(pod_img), Some(rs_img)) = (&pod_config.image, &rs_config.image) {
            if pod_img != rs_img {
                diffs.push(ConfigDiff {
                    field: "Image".to_string(),
                    pod_value: pod_img.clone(),
                    rs_value: rs_img.clone(),
                    level: DiffLevel::Critical,
                    impact: "可能运行旧版本镜像".to_string(),
                });
            }
        }
        
        // Compare CPU limits
        if let (Some(pod_cpu), Some(rs_cpu)) = (&pod_config.resources_limits_cpu, &rs_config.resources_limits_cpu) {
            if pod_cpu != rs_cpu {
                diffs.push(ConfigDiff {
                    field: "CPU Limit".to_string(),
                    pod_value: pod_cpu.clone(),
                    rs_value: rs_cpu.clone(),
                    level: DiffLevel::Warning,
                    impact: "资源限制与期望不符，可能导致性能问题".to_string(),
                });
            }
        }
        
        // Compare memory limits
        if let (Some(pod_mem), Some(rs_mem)) = (&pod_config.resources_limits_memory, &rs_config.resources_limits_memory) {
            if pod_mem != rs_mem {
                diffs.push(ConfigDiff {
                    field: "Memory Limit".to_string(),
                    pod_value: pod_mem.clone(),
                    rs_value: rs_mem.clone(),
                    level: DiffLevel::Warning,
                    impact: "内存限制与期望不符，可能导致 OOM".to_string(),
                });
            }
        }
        
        diffs
    }
    
    pub fn format_diffs(diffs: &[ConfigDiff], pod_name: &str, namespace: &str, rs_name: &str) -> String {
        let mut output = format!("=== Configuration Diff ===\n");
        output.push_str(&format!("Namespace: {}\n", namespace));
        output.push_str(&format!("Pod: {}\n", pod_name));
        output.push_str(&format!("ReplicaSet: {}\n\n", rs_name));
        
        let (critical_count, warning_count) = diffs.iter().fold((0, 0), |(c, w), d| {
            match d.level {
                DiffLevel::Critical => (c + 1, w),
                DiffLevel::Warning => (c, w + 1),
                DiffLevel::Match => (c, w),
            }
        });
        
        if critical_count > 0 || warning_count > 0 {
            for diff in diffs {
                let icon = match diff.level {
                    DiffLevel::Critical => "🔴",
                    DiffLevel::Warning => "⚠️",
                    DiffLevel::Match => "✅",
                };
                output.push_str(&format!("{} {} Mismatch\n", icon, diff.field));
                output.push_str(&format!("   Pod:     {}\n", diff.pod_value));
                output.push_str(&format!("   RS:      {}\n", diff.rs_value));
                output.push_str(&format!("   Impact:  {}\n\n", diff.impact));
            }
        } else {
            output.push_str("✅ All settings match\n");
        }
        
        output
    }
}
```

---

### 任务 6：更新 CLI 参数

**文件：**
- 修改：`src/cli.rs`

- [ ] **步骤 1：添加新的 CLI 参数**

在 `Cli` 结构体中添加：

```rust
/// Enable AI-powered diagnosis (requires --ai-endpoint and --ai-key or OPENAI_* env vars)
#[arg(long)]
pub ai: bool,

/// AI model name (default: gpt-4)
#[arg(long, default_value = "gpt-4")]
pub ai_model: String,

/// AI API endpoint URL (e.g., http://localhost:11434/v1 for Ollama)
#[arg(long)]
pub ai_endpoint: Option<String>,

/// AI API key (or use OPENAI_API_KEY env var)
#[arg(long)]
pub ai_key: Option<String>,

/// Enable timeline view of pod events
#[arg(long)]
pub timeline: bool,

/// Time range for timeline (e.g., 1h, 6h, 24h, 48h, 168h)
#[arg(long, default_value = "24h")]
pub since: String,

/// Enable config diff between pod and ReplicaSet
#[arg(long)]
pub diff: bool,

/// Force execution of potentially risky operations
#[arg(long)]
pub force: bool,
```

---

### 任务 7：更新主程序集成新功能

**文件：**
- 修改：`src/main.rs`

- [ ] **步骤 1：添加新模块**

在文件开头添加：

```rust
mod ai;
mod timeline;
mod diff;
```

- [ ] **步骤 2：集成 AI 诊断功能**

在 `main()` 函数中添加：

```rust
// AI 诊断模式
if cli.ai {
    let ai_endpoint = cli.ai_endpoint.clone()
        .or_else(|| std::env::var("OPENAI_BASE_URL").ok())
        .unwrap_or_else(|| "http://localhost:11434/v1".to_string());
    
    let ai_key = cli.ai_key.clone()
        .or_else(|| std::env::var("OPENAI_API_KEY").ok());
    
    let analyzer = ai::AiAnalyzer::new(ai_endpoint, ai_key, cli.ai_model.clone());
    
    // 收集诊断数据
    let diagnostic_data = format!("Pod: {}\nNamespace: {}\nNode: {}\nContainer: {}\nImage: {}",
        cli.pod_name, cli.namespace, node_name, container_name, container_image);
    
    match analyzer.diagnose(&diagnostic_data).await {
        Ok(result) => {
            println!("\n{}", result);
        }
        Err(e) => {
            eprintln!("AI diagnosis failed: {}", e);
            std::process::exit(1);
        }
    }
    return Ok(());
}
```

- [ ] **步骤 3：集成 Timeline 功能**

添加：

```rust
// 时间旅行模式
if cli.timeline {
    let collector = timeline::TimelineCollector::new(k8s_client.clone());
    
    let since_duration = parse_duration(&cli.since);
    
    let events = collector.collect_events(&cli.pod_name, &cli.namespace, since_duration).await?;
    let output = timeline::TimelineFormatter::format_events(&cli.pod_name, &cli.namespace, &events);
    println!("{}", output);
    
    let restarts = collector.collect_restart_info(&pod);
    let restart_output = timeline::TimelineFormatter::format_restarts(&restarts);
    println!("{}", restart_output);
    
    return Ok(());
}
```

- [ ] **步骤 4：集成 Diff 功能**

添加：

```rust
// 配置对比模式
if cli.diff {
    let collector = diff::ConfigCollector::new(k8s_client.clone());
    
    let pod_config = collector.collect_pod_config(&pod).await;
    
    if let Ok(Some((rs_name, rs)))) = collector.find_replicaset(&pod).await {
        let rs_config = collector.collect_rs_config(&rs);
        let diffs = diff::ConfigComparator::compare(&pod_config, &rs_config);
        let output = diff::ConfigComparator::format_diffs(&diffs, &cli.pod_name, &cli.namespace, &rs_name);
        println!("{}", output);
    } else {
        println!("No ReplicaSet found for this pod, cannot compare config");
    }
    
    return Ok(());
}
```

- [ ] **步骤 5：添加辅助函数**

添加：

```rust
fn parse_duration(s: &str) -> chrono::Duration {
    let s = s.trim_end_matches(|c: char| !c.is_ascii_digit());
    match s {
        "1h" => chrono::Duration::hours(1),
        "6h" => chrono::Duration::hours(6),
        "12h" => chrono::Duration::hours(12),
        "24h" => chrono::Duration::hours(24),
        "48h" => chrono::Duration::hours(48),
        "168h" => chrono::Duration::hours(168),
        _ => chrono::Duration::hours(24),
    }
}
```

---

### 任务 8：更新安装脚本

**文件：**
- 修改：`install.sh`

- [ ] **步骤 1：更新工具名称**

将所有 `kubectl-pod-debug` 替换为 `kubectl-dbg`

---

### 任务 9：更新 GitHub Actions

**文件：**
- 修改：`.github/workflows/release.yml`

- [ ] **步骤 1：更新 artifact 名称**

将所有 `kubectl-pod-debug-` 替换为 `kubectl-dbg-`

---

### 任务 10：更新文档

**文件：**
- 修改：`README.md`
- 修改：`README_zh.md`

- [ ] **步骤 1：添加新功能文档**

在 README.md 中添加：

```markdown
## AI 智能诊断

`--ai` 调用 AI 分析 Pod 问题：

```bash
kubectl dbg my-pod --ai

# 指定模型和端点
kubectl dbg my-pod --ai --ai-model gpt-4 --ai-endpoint http://localhost:11434/v1
```

## 时间旅行调试

`--timeline` 展示 Pod 事件历史：

```bash
kubectl dbg my-pod --timeline --since 24h
```

## Pod 配置对比

`--diff` 对比 Pod 与 ReplicaSet 配置：

```bash
kubectl dbg my-pod --diff
```
```

---

### 任务 11：提交代码并测试

- [ ] **步骤 1：编译测试**

```bash
cargo build --release
```

- [ ] **步骤 2：提交代码**

```bash
git add .
git commit -m "feat: add AI diagnosis, timeline and diff features"
git push origin trae/solo-agent-0TR5k3
```

- [ ] **步骤 3：合并到 main 并打 tag**

```bash
git checkout main
git merge trae/solo-agent-0TR5k3
git tag -a v3.0.0 -m "Release v3.0.0 with AI diagnosis, timeline and diff features"
git push origin main
git push origin v3.0.0
```

---

## 执行选项

计划已完成并保存到 `docs/superpowers/plans/2026-05-26-kubectl-dbg-v3-implementation.md`。

**请选择执行方式：**

1. **子代理驱动（推荐）** - 每个任务调度一个新的子代理，任务间进行审查，快速迭代

2. **内联执行** - 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点
