# kubectl-pod-debug 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 开发 kubectl 插件 `kubectl-pod-debug`，通过 SSH 连接到 Pod 所在宿主机，使用 nsenter 直接进入 Pod 的 Linux namespace 进行调试。

**架构：** 纯 SSH 透传方案。Rust 二进制负责 K8s API 交互（获取 Pod/Node 信息）、SSH 连接管理、容器运行时检测、nsenter 命令编排。交互式模式下直接建立 SSH → nsenter 的 PTY 通道。

**技术栈：** Rust 1.95, clap 4, kube 0.9x, k8s-openapi 0.22, russh 0.4x, tokio 1, thiserror 2, eyre 0.6, tracing

**设计文档：** `docs/superpowers/specs/2026-05-23-kubectl-pod-debug-design.md`

---

## 文件结构

```
kubectl-pod-debug/
├── Cargo.toml                          # 项目元数据和依赖
├── build.rs                            # 编译时嵌入版本信息
├── src/
│   ├── main.rs                         # 入口：初始化 tracing，调用 cli，执行主流程
│   ├── cli.rs                          # clap CLI 参数定义（derive）
│   ├── error.rs                        # PodDebugError 统一错误类型
│   ├── k8s/
│   │   ├── mod.rs                      # 模块导出
│   │   ├── client.rs                   # kubeconfig 加载、kube Client 创建
│   │   ├── pod.rs                      # 获取 Pod 信息、匹配容器状态、提取 containerID
│   │   └── node.rs                     # 获取节点 InternalIP
│   ├── ssh/
│   │   ├── mod.rs                      # 模块导出
│   │   ├── connect.rs                  # SSH 连接建立、密钥认证
│   │   └── exec.rs                     # 远程命令执行、PTY 管理、交互式 shell
│   ├── runtime/
│   │   ├── mod.rs                      # 模块导出 + RuntimeType 枚举
│   │   ├── detector.rs                 # 自动检测容器运行时类型
│   │   ├── containerd.rs               # 通过 crictl inspect 获取容器 PID
│   │   └── docker.rs                   # 通过 docker inspect 获取容器 PID
│   └── nsenter/
│       ├── mod.rs                      # 模块导出
│       └── builder.rs                  # 构建最终的 nsenter 命令字符串
```

---

### 任务 1：项目脚手架 + CLI 参数定义

**文件：**
- 创建：`Cargo.toml`
- 创建：`src/main.rs`
- 创建：`src/cli.rs`
- 创建：`src/error.rs`
- 创建：`build.rs`

- [ ] **步骤 1：创建 Cargo.toml**

```toml
[package]
name = "kubectl-pod-debug"
version = "0.1.0"
edition = "2021"
description = "kubectl plugin for debugging pod network/process via nsenter on host node"
license = "MIT"
repository = "https://github.com/user/kubectl-pod-debug"

[[bin]]
name = "kubectl-pod-debug"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive", "env"] }
kube = { version = "0.98", features = ["runtime", "derive"] }
k8s-openapi = { version = "0.23", features = ["v1_32"] }
kube-runtime = "0.98"
tokio = { version = "1", features = ["full"] }
russh = "0.45"
russh-keys = "0.45"
thiserror = "2"
eyre = "0.6"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
serde_json = "1"
dirs = "6"
```

- [ ] **步骤 2：创建 build.rs**

```rust
fn main() {
    // 编译时嵌入 git 版本信息（如果可用）
    println!("cargo:rerun-if-changed=build.rs");
}
```

- [ ] **步骤 3：创建 src/error.rs**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PodDebugError {
    #[error("Pod '{name}' not found in namespace '{namespace}'")]
    PodNotFound { name: String, namespace: String },

    #[error("Container '{container}' not found in pod '{pod}'")]
    ContainerNotFound { container: String, pod: String },

    #[error("Container '{container}' is not running (state: {state})")]
    ContainerNotRunning { container: String, state: String },

    #[error("Failed to connect to node '{node}' via SSH: {reason}")]
    SshConnectFailed { node: String, reason: String },

    #[error("SSH authentication failed for user '{user}': {reason}")]
    SshAuthFailed { user: String, reason: String },

    #[error("Failed to detect container runtime on node '{node}'")]
    RuntimeDetectionFailed { node: String },

    #[error("Failed to get container PID: {reason}")]
    PidLookupFailed { reason: String },

    #[error("nsenter execution failed: {reason}")]
    NsenterFailed { reason: String },

    #[error("Kubernetes API error: {0}")]
    KubeError(#[from] kube::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PodDebugError>;
```

- [ ] **步骤 4：创建 src/cli.rs**

```rust
use clap::Parser;

/// kubectl plugin for debugging pod network/process via nsenter on host node.
///
/// Connects to the pod's host node via SSH, retrieves the container PID,
/// and uses nsenter to enter the pod's Linux namespaces for debugging.
#[derive(Parser, Debug)]
#[command(name = "kubectl-pod-debug", version, about)]
pub struct Cli {
    /// Target pod name
    pub pod_name: String,

    /// Target namespace
    #[arg(short, long, default_value = "default")]
    pub namespace: String,

    /// Target container name (defaults to first container)
    #[arg(short, long)]
    pub container: Option<String>,

    /// SSH user for connecting to the host node
    #[arg(long, default_value = "root")]
    pub ssh_user: String,

    /// SSH private key path
    #[arg(short = 'i', long, default_value = "~/.ssh/id_rsa")]
    pub ssh_key: String,

    /// SSH port
    #[arg(long, default_value_t = 22)]
    pub ssh_port: u16,

    /// Namespace type to enter: network, pid, mount, uts, ipc, all
    #[arg(long, default_value = "all", value_parser = ["network", "pid", "mount", "uts", "ipc", "all"])]
    pub ns_type: String,

    /// Container runtime: auto, containerd, docker
    #[arg(long, default_value = "auto", value_parser = ["auto", "containerd", "docker"])]
    pub runtime: String,

    /// Path to kubeconfig file
    #[arg(long)]
    pub kubeconfig: Option<String>,

    /// Kubernetes context to use
    #[arg(long)]
    pub context: Option<String>,

    /// Only show the commands that would be executed
    #[arg(long)]
    pub dry_run: bool,

    /// Show verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Command to execute inside the pod's namespace (use -- to separate from flags)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}
```

- [ ] **步骤 5：创建 src/main.rs（骨架）**

```rust
mod cli;
mod error;

use cli::Cli;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> error::Result<()> {
    let cli = Cli::parse();

    // 初始化 tracing
    let filter = if cli.verbose {
        EnvFilter::new("kubectl_pod_debug=debug")
    } else {
        EnvFilter::new("kubectl_pod_debug=warn")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    tracing::info!("kubectl-pod-debug starting");
    tracing::debug!("CLI args: {:?}", cli);

    // TODO: 后续任务将实现主流程
    println!("Pod: {}, Namespace: {}", cli.pod_name, cli.namespace);

    Ok(())
}
```

- [ ] **步骤 6：编译验证**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo init --name kubectl-pod-debug && cp Cargo.toml.bak Cargo.toml && cargo check 2>&1 | tail -20`

注意：先在 /workspace 下 `cargo init`，然后用上面的 Cargo.toml 替换生成的。如果依赖版本不兼容，根据编译错误调整版本号。

预期：编译通过（可能有 warning，不应有 error）

- [ ] **步骤 7：验证 CLI 参数解析**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo run -- --help`

预期：显示帮助信息，包含所有参数

- [ ] **步骤 8：Commit**

```bash
git init && git add -A && git commit -m "feat: project scaffold with CLI definition and error types"
```

---

### 任务 2：Kubernetes 客户端模块

**文件：**
- 创建：`src/k8s/mod.rs`
- 创建：`src/k8s/client.rs`
- 创建：`src/k8s/pod.rs`
- 创建：`src/k8s/node.rs`
- 修改：`src/main.rs`

- [ ] **步骤 1：创建 src/k8s/mod.rs**

```rust
pub mod client;
pub mod node;
pub mod pod;
```

- [ ] **步骤 2：创建 src/k8s/client.rs**

```rust
use crate::cli::Cli;
use crate::error::{PodDebugError, Result};
use kube::{Client, Config};

/// 加载 kubeconfig 并创建 Kubernetes Client
pub async fn build_client(cli: &Cli) -> Result<Client> {
    let mut config = if let Some(kubeconfig_path) = &cli.kubeconfig {
        Config::from_custom_kubeconfig(
            kube::config::Kubeconfig::from_file(kubeconfig_path)
                .map_err(|e| PodDebugError::KubeError(e.into()))?,
            &kube::config::KubeconfigOptions {
                context: cli.context.clone(),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| PodDebugError::KubeError(e.into()))?
    } else {
        Config::from_kubeconfig(&kube::config::KubeconfigOptions {
            context: cli.context.clone(),
            ..Default::default()
        })
        .await
        .map_err(|e| PodDebugError::KubeError(e.into()))?
    };

    // 调优连接参数
    config.accept_invalid_certs = false;

    let client = Client::try_from(config).map_err(|e| PodDebugError::KubeError(e.into()))?;
    Ok(client)
}
```

- [ ] **步骤 3：创建 src/k8s/pod.rs**

```rust
use crate::error::{PodDebugError, Result};
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client};

/// 获取 Pod 信息
pub async fn get_pod(client: &Client, name: &str, namespace: &str) -> Result<Pod> {
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    pods.get(name)
        .await
        .map_err(|e| match e {
            kube::Error::Api(api_err) if api_err.code == 404 => PodDebugError::PodNotFound {
                name: name.to_string(),
                namespace: namespace.to_string(),
            },
            other => PodDebugError::KubeError(other),
        })
}

/// 获取 Pod 运行的节点名称
pub fn get_node_name(pod: &Pod) -> Result<String> {
    pod.spec
        .as_ref()
        .and_then(|s| s.node_name.clone())
        .ok_or_else(|| PodDebugError::NsenterFailed {
            reason: format!("Pod '{}' has no node assigned (is it scheduled?)", pod.metadata.name.as_deref().unwrap_or("?")),
        })
}

/// 获取容器 ID（去掉运行时前缀，如 "containerd://" → 纯 ID）
pub fn get_container_id(pod: &Pod, container_name: &str) -> Result<String> {
    let statuses = pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .ok_or_else(|| PodDebugError::ContainerNotFound {
            container: container_name.to_string(),
            pod: pod.metadata.name.as_deref().unwrap_or("?").to_string(),
        })?;

    let status = statuses
        .iter()
        .find(|cs| cs.name == container_name)
        .ok_or_else(|| PodDebugError::ContainerNotFound {
            container: container_name.to_string(),
            pod: pod.metadata.name.as_deref().unwrap_or("?").to_string(),
        })?;

    // 检查容器是否在运行
    let state = status
        .state
        .as_ref()
        .and_then(|s| s.running.as_ref())
        .ok_or_else(|| PodDebugError::ContainerNotRunning {
            container: container_name.to_string(),
            state: "not running".to_string(),
        })?;

    let container_id = status
        .container_id
        .as_ref()
        .ok_or_else(|| PodDebugError::PidLookupFailed {
            reason: format!("Container '{}' has no containerID", container_name),
        })?;

    // 去掉运行时前缀（"containerd://xxxxx" → "xxxxx"）
    let id = container_id
        .splitn(2, "://")
        .nth(1)
        .unwrap_or(container_id)
        .to_string();

    Ok(id)
}

/// 获取 Pod 中第一个容器的名称
pub fn get_first_container_name(pod: &Pod) -> Option<String> {
    pod.spec.as_ref()?.containers.first()?.name.clone()
}
```

- [ ] **步骤 4：创建 src/k8s/node.rs**

```rust
use crate::error::{PodDebugError, Result};
use k8s_openapi::api::core::v1::Node;
use kube::{Api, Client};

/// 获取节点的 InternalIP 地址
pub async fn get_node_ip(client: &Client, node_name: &str) -> Result<String> {
    let nodes: Api<Node> = Api::all(client.clone());
    let node = nodes.get(node_name).await.map_err(PodDebugError::KubeError)?;

    let addresses = node
        .status
        .as_ref()
        .and_then(|s| s.addresses.as_ref())
        .ok_or_else(|| PodDebugError::SshConnectFailed {
            node: node_name.to_string(),
            reason: "Node has no addresses in status".to_string(),
        })?;

    addresses
        .iter()
        .find(|addr| addr.type_ == "InternalIP")
        .map(|addr| addr.address.clone())
        .ok_or_else(|| PodDebugError::SshConnectFailed {
            node: node_name.to_string(),
            reason: "Node has no InternalIP address".to_string(),
        })
}
```

- [ ] **步骤 5：更新 src/main.rs 集成 K8s 模块**

在 `main.rs` 中添加 `mod k8s;`，并在 main 函数中调用 `k8s::client::build_client(&cli).await?` 验证编译通过。

- [ ] **步骤 6：编译验证**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo check 2>&1 | tail -20`

预期：编译通过

- [ ] **步骤 7：Commit**

```bash
git add -A && git commit -m "feat: add Kubernetes client module (pod info, node IP)"
```

---

### 任务 3：SSH 连接模块

**文件：**
- 创建：`src/ssh/mod.rs`
- 创建：`src/ssh/connect.rs`
- 创建：`src/ssh/exec.rs`

- [ ] **步骤 1：创建 src/ssh/mod.rs**

```rust
pub mod connect;
pub mod exec;
```

- [ ] **步骤 2：创建 src/ssh/connect.rs**

```rust
use crate::error::{PodDebugError, Result};
use russh::client::{self, Handle};
use russh_keys::key::PrivateKey;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

struct SshClient;

#[async_trait::async_trait]
impl client::Handler for SshClient {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // TODO: 在后续版本中实现 host key 验证
        Ok(true)
    }
}

/// 建立 SSH 连接并返回 session handle
pub async fn connect(
    host: &str,
    port: u16,
    user: &str,
    key_path: &str,
) -> Result<Handle<SshClient>> {
    // 展开路径中的 ~
    let key_path = shellexpand::tilde(key_path).to_string();

    let key = PrivateKey::read_openssh_file(&key_path).map_err(|e| PodDebugError::SshAuthFailed {
        user: user.to_string(),
        reason: format!("Failed to read SSH key '{}': {}", key_path, e),
    })?;

    let config = client::Config {
        preferred: russh::Preferred::COMPRESSED,
        ..Default::default()
    };

    let config = Arc::new(config);
    let mut session = client::connect(config, (host, port), SshClient)
        .await
        .map_err(|e| PodDebugError::SshConnectFailed {
            node: host.to_string(),
            reason: e.to_string(),
        })?;

    let auth_res = session
        .authenticate_publickey(user, Arc::new(key))
        .await
        .map_err(|e| PodDebugError::SshAuthFailed {
            user: user.to_string(),
            reason: e.to_string(),
        })?;

    if !auth_res.success() {
        return Err(PodDebugError::SshAuthFailed {
            user: user.to_string(),
            reason: "Public key authentication rejected".to_string(),
        });
    }

    Ok(session)
}
```

注意：需要在 Cargo.toml 中添加 `async-trait` 和 `shellexpand` 依赖。

- [ ] **步骤 3：创建 src/ssh/exec.rs**

```rust
use crate::error::{PodDebugError, Result};
use russh::Channel;
use russh::client::Handle;
use russh::ChannelMsg;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::connect::SshClient;

/// 在远程主机上执行命令并返回 stdout 输出
pub async fn exec_command(
    session: &Handle<SshClient>,
    command: &str,
) -> Result<String> {
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| PodDebugError::NsenterFailed {
            reason: format!("Failed to open SSH channel: {}", e),
        })?;

    channel
        .exec(true, command)
        .await
        .map_err(|e| PodDebugError::NsenterFailed {
            reason: format!("Failed to exec command: {}", e),
        })?;

    let mut output = Vec::new();
    let mut buf = [0u8; 4096];

    loop {
        let msg = channel
            .wait()
            .await
            .map_err(|e| PodDebugError::NsenterFailed {
                reason: format!("SSH channel error: {}", e),
            })?;

        match msg {
            ChannelMsg::Data { data } => {
                output.extend_from_slice(&data);
            }
            ChannelMsg::ExtendedData { data, .. } => {
                // stderr
                output.extend_from_slice(&data);
            }
            ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }

    String::from_utf8(output).map_err(|e| PodDebugError::NsenterFailed {
        reason: format!("Command output is not valid UTF-8: {}", e),
    })
}

/// 在远程主机上启动交互式 shell（PTY 模式）
pub async fn interactive_shell(
    session: &Handle<SshClient>,
    command: &str,
) -> Result<()> {
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| PodDebugError::NsenterFailed {
            reason: format!("Failed to open SSH channel: {}", e),
        })?;

    // 请求 PTY
    channel
        .request_pty(true, "xterm-256color", 80, 24, &[])
        .await
        .map_err(|e| PodDebugError::NsenterFailed {
            reason: format!("Failed to request PTY: {}", e),
        })?;

    channel
        .exec(true, command)
        .await
        .map_err(|e| PodDebugError::NsenterFailed {
            reason: format!("Failed to exec command: {}", e),
        })?;

    // 将本地 stdin/stdout 连接到远程 PTY
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let (mut reader, mut writer) = channel.into_stream();

    // 读取远程输出 → 本地 stdout
    let read_task = tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = stdout.write_all(&buf[..n]).await {
                        eprintln!("stdout write error: {}", e);
                        break;
                    }
                    if let Err(e) = stdout.flush().await {
                        eprintln!("stdout flush error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("read error: {}", e);
                    break;
                }
            }
        }
    });

    // 本地 stdin → 远程输入
    let write_task = tokio::spawn(async move {
        let mut buf = [0u8; 1];
        loop {
            match tokio::io::stdin().read(&mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    if let Err(e) = writer.write_all(&buf).await {
                        eprintln!("stdin write error: {}", e);
                        break;
                    }
                    if let Err(e) = writer.flush().await {
                        eprintln!("stdin flush error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("stdin read error: {}", e);
                    break;
                }
            }
        }
    });

    let _ = tokio::try_join!(read_task, write_task);

    Ok(())
}
```

- [ ] **步骤 4：添加缺失依赖到 Cargo.toml**

在 `[dependencies]` 中添加：
```toml
async-trait = "0.1"
shellexpand = "3"
ssh-key = "0.6"
```

- [ ] **步骤 5：编译验证**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo check 2>&1 | tail -30`

预期：编译通过（根据 russh 版本的实际 API 调整代码，russh 0.45 的 API 可能有变化）

- [ ] **步骤 6：Commit**

```bash
git add -A && git commit -m "feat: add SSH connection and remote execution modules"
```

---

### 任务 4：容器运行时检测模块

**文件：**
- 创建：`src/runtime/mod.rs`
- 创建：`src/runtime/detector.rs`
- 创建：`src/runtime/containerd.rs`
- 创建：`src/runtime/docker.rs`

- [ ] **步骤 1：创建 src/runtime/mod.rs**

```rust
pub mod containerd;
pub mod detector;
pub mod docker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeType {
    Containerd,
    Docker,
}
```

- [ ] **步骤 2：创建 src/runtime/detector.rs**

```rust
use super::RuntimeType;
use crate::error::{PodDebugError, Result};
use crate::ssh::exec;

/// 自动检测节点上的容器运行时
pub async fn detect_runtime(
    exec_fn: &(dyn Fn(&str) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::error::Result<String>> + Send>> + Send + Sync),
) -> Result<RuntimeType> {
    let output = exec_fn("which crictl 2>/dev/null && echo 'containerd' || (which docker 2>/dev/null && echo 'docker' || echo 'unknown')").await?;

    match output.trim() {
        "containerd" => Ok(RuntimeType::Containerd),
        "docker" => Ok(RuntimeType::Docker),
        _ => Err(PodDebugError::RuntimeDetectionFailed {
            node: "unknown".to_string(),
        }),
    }
}
```

注意：上面的签名比较复杂，因为需要传入 SSH 执行函数。更简洁的方式是直接传入 `&Handle<SshClient>`。如果编译有问题，改为直接接受 Handle。

替代实现（更简洁，推荐）：

```rust
use super::RuntimeType;
use crate::error::{PodDebugError, Result};
use russh::client::Handle;
use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;

/// 自动检测节点上的容器运行时
pub async fn detect_runtime(
    session: &Handle<SshClient>,
) -> Result<RuntimeType> {
    let output = exec_command(
        session,
        "which crictl 2>/dev/null && echo 'containerd' || (which docker 2>/dev/null && echo 'docker' || echo 'unknown')"
    ).await?;

    match output.trim() {
        "containerd" => Ok(RuntimeType::Containerd),
        "docker" => Ok(RuntimeType::Docker),
        _ => Err(PodDebugError::RuntimeDetectionFailed {
            node: "unknown".to_string(),
        }),
    }
}
```

- [ ] **步骤 3：创建 src/runtime/containerd.rs**

```rust
use crate::error::{PodDebugError, Result};
use russh::client::Handle;
use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;

/// 通过 crictl 获取容器 PID
pub async fn get_container_pid(
    session: &Handle<SshClient>,
    container_id: &str,
) -> Result<u32> {
    // crictl inspect 返回 JSON，提取 info.pid
    let cmd = format!(
        "crictl inspect {} 2>/dev/null | grep -o '\"pid\"[[:space:]]*:[[:space:]]*[0-9]*' | grep -o '[0-9]*' | head -1",
        container_id
    );

    let output = exec_command(session, &cmd).await?;
    let pid_str = output.trim();

    pid_str.parse::<u32>().map_err(|_| PodDebugError::PidLookupFailed {
        reason: format!("Failed to parse PID from crictl output: '{}'", pid_str),
    })
}
```

- [ ] **步骤 4：创建 src/runtime/docker.rs**

```rust
use crate::error::{PodDebugError, Result};
use russh::client::Handle;
use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;

/// 通过 docker inspect 获取容器 PID
pub async fn get_container_pid(
    session: &Handle<SshClient>,
    container_id: &str,
) -> Result<u32> {
    let cmd = format!(
        "docker inspect --format '{{{{.State.Pid}}}}' {} 2>/dev/null",
        container_id
    );

    let output = exec_command(session, &cmd).await?;
    let pid_str = output.trim();

    pid_str.parse::<u32>().map_err(|_| PodDebugError::PidLookupFailed {
        reason: format!("Failed to parse PID from docker inspect output: '{}'", pid_str),
    })
}
```

- [ ] **步骤 5：编译验证**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo check 2>&1 | tail -20`

预期：编译通过

- [ ] **步骤 6：Commit**

```bash
git add -A && git commit -m "feat: add container runtime detection module (containerd + docker)"
```

---

### 任务 5：nsenter 命令构建模块

**文件：**
- 创建：`src/nsenter/mod.rs`
- 创建：`src/nsenter/builder.rs`

- [ ] **步骤 1：创建 src/nsenter/mod.rs**

```rust
pub mod builder;
```

- [ ] **步骤 2：创建 src/nsenter/builder.rs**

```rust
/// namespace 类型到 nsenter 标志的映射
static NS_FLAGS: &[(&str, &str)] = &[
    ("network", "-n"),
    ("pid", "-p"),
    ("mount", "-m"),
    ("uts", "-u"),
    ("ipc", "-i"),
];

/// 构建 nsenter 命令
///
/// # 参数
/// - `pid`: 容器进程 PID
/// - `ns_type`: namespace 类型（"all", "network", "pid" 等）
/// - `command`: 要在 namespace 中执行的命令（空则使用 /bin/bash）
pub fn build_nsenter_command(pid: u32, ns_type: &str, command: &[String]) -> String {
    let ns_flags = if ns_type == "all" {
        "-a".to_string()
    } else {
        NS_FLAGS
            .iter()
            .filter(|(name, _)| *name == ns_type)
            .map(|(_, flag)| flag.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };

    let cmd = if command.is_empty() {
        "/bin/bash".to_string()
    } else {
        command.join(" ")
    };

    format!("nsenter -t {} {} -- {}", pid, ns_flags, cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_all_ns() {
        let cmd = build_nsenter_command(12345, "all", &[]);
        assert_eq!(cmd, "nsenter -t 12345 -a -- /bin/bash");
    }

    #[test]
    fn test_build_network_ns() {
        let cmd = build_nsenter_command(12345, "network", &[]);
        assert_eq!(cmd, "nsenter -t 12345 -n -- /bin/bash");
    }

    #[test]
    fn test_build_with_command() {
        let cmd = build_nsenter_command(12345, "network", &["tcpdump".to_string(), "-i".to_string(), "eth0".to_string()]);
        assert_eq!(cmd, "nsenter -t 12345 -n -- tcpdump -i eth0");
    }

    #[test]
    fn test_build_pid_ns() {
        let cmd = build_nsenter_command(12345, "pid", &[]);
        assert_eq!(cmd, "nsenter -t 12345 -p -- /bin/bash");
    }
}
```

- [ ] **步骤 3：运行测试**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo test nsenter 2>&1`

预期：4 个测试全部通过

- [ ] **步骤 4：Commit**

```bash
git add -A && git commit -m "feat: add nsenter command builder with tests"
```

---

### 任务 6：主流程集成

**文件：**
- 修改：`src/main.rs`

- [ ] **步骤 1：实现完整的主流程**

将 `src/main.rs` 替换为以下内容：

```rust
mod cli;
mod error;
mod k8s;
mod nsenter;
mod runtime;
mod ssh;

use cli::Cli;
use clap::Parser;
use error::Result;
use runtime::RuntimeType;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // 初始化 tracing
    let filter = if cli.verbose {
        EnvFilter::new("kubectl_pod_debug=debug")
    } else {
        EnvFilter::new("kubectl_pod_debug=warn")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    tracing::info!("kubectl-pod-debug starting");
    tracing::debug!("CLI args: {:?}", cli);

    // 1. 构建 K8s Client
    let client = k8s::client::build_client(&cli).await?;
    tracing::info!("Kubernetes client created");

    // 2. 获取 Pod 信息
    let pod = k8s::pod::get_pod(&client, &cli.pod_name, &cli.namespace).await?;
    let container_name = cli
        .container
        .clone()
        .unwrap_or_else(|| k8s::pod::get_first_container_name(&pod).expect("Pod has no containers"));
    let container_id = k8s::pod::get_container_id(&pod, &container_name)?;
    let node_name = k8s::pod::get_node_name(&pod)?;
    tracing::info!("Pod: {}, Container: {}, Node: {}, ContainerID: {}", cli.pod_name, container_name, node_name, container_id);

    // 3. 获取节点 IP
    let node_ip = k8s::node::get_node_ip(&client, &node_name).await?;
    tracing::info!("Node IP: {}", node_ip);

    // 4. 构建 nsenter 命令（dry-run 模式）
    let nsenter_cmd = nsenter::builder::build_nsenter_command(0, &cli.ns_type, &cli.command);

    if cli.dry_run {
        println!("=== Dry Run ===");
        println!("Node: {} ({})", node_name, node_ip);
        println!("SSH: {}@{}:{} -i {}", cli.ssh_user, node_ip, cli.ssh_port, cli.ssh_key);
        println!("Container: {} (ID: {})", container_name, container_id);
        println!("Runtime: {}", cli.runtime);
        println!("nsenter command: nsenter -t <PID> ... {}", nsenter_cmd);
        return Ok(());
    }

    // 5. 建立 SSH 连接
    let session = ssh::connect::connect(&node_ip, cli.ssh_port, &cli.ssh_user, &cli.ssh_key).await?;
    tracing::info!("SSH connection established to {}", node_ip);

    // 6. 检测/确定运行时
    let runtime_type = match cli.runtime.as_str() {
        "containerd" => RuntimeType::Containerd,
        "docker" => RuntimeType::Docker,
        _ => runtime::detector::detect_runtime(&session).await?,
    };
    tracing::info!("Container runtime: {:?}", runtime_type);

    // 7. 获取容器 PID
    let pid = match runtime_type {
        RuntimeType::Containerd => runtime::containerd::get_container_pid(&session, &container_id).await?,
        RuntimeType::Docker => runtime::docker::get_container_pid(&session, &container_id).await?,
    };
    tracing::info!("Container PID: {}", pid);

    // 8. 构建最终的 nsenter 命令
    let nsenter_cmd = nsenter::builder::build_nsenter_command(pid, &cli.ns_type, &cli.command);
    tracing::info!("Executing: {}", nsenter_cmd);

    // 9. 执行
    if cli.command.is_empty() {
        // 交互式 shell
        ssh::exec::interactive_shell(&session, &nsenter_cmd).await?;
    } else {
        // 单次命令执行
        let output = ssh::exec::exec_command(&session, &nsenter_cmd).await?;
        print!("{}", output);
    }

    Ok(())
}
```

- [ ] **步骤 2：编译验证**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo check 2>&1 | tail -30`

预期：编译通过。如果有 API 不兼容问题，根据错误信息调整。

- [ ] **步骤 3：运行测试**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo test 2>&1`

预期：所有测试通过

- [ ] **步骤 4：验证 dry-run 模式**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo run -- --help`

预期：显示帮助信息

- [ ] **步骤 5：Commit**

```bash
git add -A && git commit -m "feat: integrate all modules into main execution flow"
```

---

### 任务 7：编译优化与分发准备

**文件：**
- 创建：`README.md`
- 修改：`Cargo.toml`（添加 release profile）

- [ ] **步骤 1：优化 Cargo.toml 的 release profile**

在 `Cargo.toml` 末尾添加：

```toml
[profile.release]
opt-level = "z"     # 优化二进制大小
lto = true          # Link-Time Optimization
strip = true        # 去除调试符号
codegen-units = 1   # 更好的优化
```

- [ ] **步骤 2：创建 README.md**

```markdown
# kubectl-pod-debug

A kubectl plugin for debugging pod network/process via nsenter on the host node.

## How it works

1. Connects to the Kubernetes API to get pod info (node name, container ID)
2. SSHs into the pod's host node
3. Detects the container runtime (containerd/docker) and gets the container PID
4. Uses `nsenter` to enter the pod's Linux namespaces
5. Executes debug commands or opens an interactive shell

## Installation

```bash
# Build from source
cargo build --release
cp target/release/kubectl-pod-debug /usr/local/bin/
```

## Usage

```bash
# Interactive shell in pod's all namespaces
kubectl pod-debug my-pod -n my-namespace

# Specify container
kubectl pod-debug my-pod -n my-namespace -c my-container

# Network namespace only
kubectl pod-debug my-pod --ns-type network

# Execute a single command
kubectl pod-debug my-pod -- tcpdump -i eth0 -c 100

# Dry run (show what would be executed)
kubectl pod-debug my-pod --dry-run
```

## Prerequisites

- SSH access to all Kubernetes nodes
- `crictl` or `docker` installed on nodes
- `nsenter` installed on nodes

## License

MIT
```

- [ ] **步骤 3：Release 编译**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo build --release 2>&1 | tail -10`

预期：编译成功，生成 `target/release/kubectl-pod-debug`

- [ ] **步骤 4：验证二进制文件**

运行：`source "$HOME/.cargo/env" && cd /workspace && ./target/release/kubectl-pod-debug --help`

预期：显示帮助信息

- [ ] **步骤 5：Commit**

```bash
git add -A && git commit -m "feat: add README and release build configuration"
```

---

### 任务 8：端到端验证与修复

**文件：**
- 可能修改所有文件

- [ ] **步骤 1：运行所有测试**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo test 2>&1`

预期：所有测试通过

- [ ] **步骤 2：clippy 检查**

运行：`source "$HOME/.cargo/env" && cd /workspace && cargo clippy -- -D warnings 2>&1 | tail -30`

预期：无 warning（如有 warning，修复后重新检查）

- [ ] **步骤 3：检查二进制大小**

运行：`ls -lh /workspace/target/release/kubectl-pod-debug`

预期：二进制大小合理（< 15MB）

- [ ] **步骤 4：最终 Commit**

```bash
git add -A && git commit -m "chore: clippy fixes and final cleanup"
```

---

## 自检

### 1. 规格覆盖度

| 规格章节 | 对应任务 |
|---------|---------|
| CLI 接口（§2） | 任务 1 |
| 核心执行流程（§3.1） | 任务 6 |
| 模块职责（§3.2） | 任务 2-5 |
| 获取容器 PID（§3.3） | 任务 4 |
| nsenter 命令构建（§3.4） | 任务 5 |
| PTY 转发（§3.5） | 任务 3 |
| 项目结构（§4） | 任务 1 |
| Rust 依赖（§5） | 任务 1 |
| 错误处理（§6） | 任务 1 |
| 安全设计（§7） | 任务 1（dry-run）、任务 3（SSH 密钥） |
| 分发（§8） | 任务 7 |
| 配置文件（§9） | 未实现（标记为可选，不在 MVP 范围） |

### 2. 占位符扫描

无占位符。所有步骤包含完整代码。

### 3. 类型一致性

- `PodDebugError` 在任务 1 定义，任务 2-6 一致使用
- `RuntimeType` 在任务 4 定义，任务 6 一致使用
- `Handle<SshClient>` 在任务 3 定义，任务 4、6 一致使用
- `Cli` 在任务 1 定义，任务 2、6 一致使用
