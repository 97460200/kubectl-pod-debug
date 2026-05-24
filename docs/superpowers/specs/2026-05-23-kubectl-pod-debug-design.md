# kubectl-pod-debug 设计规格

> 日期：2026-05-23
> 状态：已批准

## 1. 概述

### 1.1 目标

开发一个 kubectl 插件 `kubectl-pod-debug`，用于在 Kubernetes 环境下调试 Pod 的进程和网络。核心特点是**不拉起新容器**，而是通过 SSH 连接到 Pod 所在的宿主机节点，使用 `nsenter` 直接进入 Pod 的 Linux namespace，利用宿主机上的原生调试工具进行诊断。

### 1.2 与 kubectl debug 的区别

| 维度 | kubectl debug | kubectl-pod-debug |
|------|--------------|-------------------|
| 原理 | 创建 Ephemeral Container 或新 Pod | 直接在宿主机上 nsenter |
| 网络视角 | 共享目标 Pod 的 network namespace | 直接进入 Pod 的 network namespace |
| 工具来源 | 调试容器镜像中的工具 | 宿主机上已安装的工具 |
| 额外资源 | 创建临时容器/Pod | 无额外资源消耗 |
| 前置条件 | 需要 EphemeralContainer 特性 | 需要 SSH 访问节点 |

### 1.3 技术选型

| 维度 | 选择 |
|------|------|
| 语言 | Rust |
| CLI 框架 | clap (derive API) |
| K8s 客户端 | kube + k8s-openapi |
| SSH 库 | russh (纯 Rust, async) |
| 异步运行时 | tokio |
| 节点访问方式 | SSH 直连 |
| 容器运行时 | containerd + Docker（自动检测） |
| 交互模式 | 交互式 Shell + 单次命令执行 |

## 2. CLI 接口

### 2.1 基本用法

```bash
# 交互式进入 Pod 的所有 namespace
kubectl pod-debug <pod-name> -n <namespace>

# 指定容器（多容器 Pod）
kubectl pod-debug <pod-name> -n <namespace> -c <container-name>

# 仅进入网络 namespace
kubectl pod-debug <pod-name> -n <namespace> --ns-type network

# 单次执行命令
kubectl pod-debug <pod-name> -n <namespace> -- tcpdump -i eth0 -c 100

# 指定 SSH 用户和密钥
kubectl pod-debug <pod-name> --ssh-user root --ssh-key ~/.ssh/id_rsa

# 指定 kubeconfig 和 context
kubectl pod-debug <pod-name> --kubeconfig /path/to/config --context prod

# Dry-run 模式
kubectl pod-debug <pod-name> --dry-run
```

### 2.2 参数定义

| 参数 | 缩写 | 类型 | 默认值 | 说明 |
|------|------|------|--------|------|
| `pod-name` | | 位置参数 | 必填 | 目标 Pod 名称 |
| `--container` | `-c` | string | 第一个容器 | 目标容器名 |
| `--namespace` | `-n` | string | `default` | Pod 所在 namespace |
| `--ssh-user` | | string | `root` | SSH 用户名 |
| `--ssh-key` | `-i` | string | `~/.ssh/id_rsa` | SSH 私钥路径 |
| `--ssh-port` | | u16 | `22` | SSH 端口 |
| `--ns-type` | | enum | `all` | 进入的 namespace 类型 |
| `--runtime` | | enum | `auto` | 容器运行时 |
| `--kubeconfig` | | string | 自动检测 | kubeconfig 路径 |
| `--context` | | string | 当前 context | kubeconfig context |
| `--dry-run` | | flag | `false` | 仅显示将要执行的命令 |
| `--verbose` | `-v` | flag | `false` | 显示详细执行过程 |

**`--ns-type` 枚举值**：`network` | `pid` | `mount` | `uts` | `ipc` | `all`

**`--runtime` 枚举值**：`auto` | `containerd` | `docker`

**`--` 之后的内容**作为要在 Pod namespace 中执行的命令。不指定则进入交互式 shell。

## 3. 架构设计

### 3.1 核心执行流程

```
kubectl pod-debug <pod>
    │
    ├─ 1. 解析 CLI 参数
    ├─ 2. 加载 kubeconfig（kube-rs Client）
    ├─ 3. 获取 Pod 信息（nodeName, containerID, containerStatus）
    ├─ 4. 获取节点 IP（node.status.addresses）
    ├─ 5. 建立 SSH 连接（russh，认证 + PTY）
    ├─ 6. 远程获取容器 PID（自动检测运行时 → crictl/docker）
    ├─ 7. 构建 nsenter 命令
    └─ 8. 执行：
         ├─ 8a. 单次命令 → 返回结果
         └─ 8b. 交互式 shell → PTY 转发
```

### 3.2 模块职责

| 模块 | 文件 | 职责 |
|------|------|------|
| CLI | `src/cli.rs` | 参数解析与验证 |
| K8s Client | `src/k8s/client.rs` | kubeconfig 加载、kube Client 创建 |
| Pod | `src/k8s/pod.rs` | 获取 Pod 信息、匹配容器状态 |
| Node | `src/k8s/node.rs` | 获取节点 IP 地址 |
| SSH Connect | `src/ssh/connect.rs` | SSH 连接建立、密钥认证 |
| SSH Exec | `src/ssh/exec.rs` | 远程命令执行、PTY 管理、交互式 shell |
| Runtime Detector | `src/runtime/detector.rs` | 自动检测容器运行时类型 |
| Containerd | `src/runtime/containerd.rs` | 通过 crictl 获取容器 PID |
| Docker | `src/runtime/docker.rs` | 通过 docker inspect 获取容器 PID |
| Nsenter Builder | `src/nsenter/builder.rs` | 构建最终的 nsenter 命令字符串 |
| Error | `src/error.rs` | 统一错误类型定义 |

### 3.3 获取容器 PID 的详细逻辑

**containerd**：
```bash
# 通过 containerID 获取 PID
crictl inspect <container-id> | jq '.info.pid'
```

**Docker**：
```bash
# 通过 containerID 获取 PID
docker inspect --format '{{.State.Pid}}' <container-id>
```

**自动检测逻辑**：
1. SSH 到节点，执行 `which crictl 2>/dev/null && echo "containerd" || (which docker 2>/dev/null && echo "docker" || echo "unknown")`
2. 如果结果为 `unknown`，报错退出
3. 用户也可通过 `--runtime` 强制指定

### 3.4 nsenter 命令构建

```bash
# 全功能调试（默认 --ns-type all）
nsenter -t <PID> -a -- <command>

# 仅网络调试
nsenter -t <PID> -n -- <command>

# 交互式 shell（无 --command）
nsenter -t <PID> -a -- /bin/bash
```

namespace 类型到 nsenter 标志的映射：

| ns-type | nsenter 标志 |
|---------|-------------|
| `network` | `-n` |
| `pid` | `-p` |
| `mount` | `-m` |
| `uts` | `-u` |
| `ipc` | `-i` |
| `all` | `-a` |

### 3.5 交互式 Shell 的 PTY 转发

- 在 SSH 通道上请求 PTY（pseudo-terminal）
- 将本地 stdin/stdout/stderr 直接连接到远程 PTY
- 支持信号转发（Ctrl+C → SIGINT, Ctrl+D → EOF）
- 窗口大小自适应（监听 SIGWINCH 信号，更新远程 PTY 大小）

## 4. 项目结构

```
kubectl-pod-debug/
├── Cargo.toml
├── Cargo.lock
├── build.rs
├── README.md
├── LICENSE
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── error.rs
│   ├── k8s/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── pod.rs
│   │   └── node.rs
│   ├── ssh/
│   │   ├── mod.rs
│   │   ├── connect.rs
│   │   └── exec.rs
│   ├── runtime/
│   │   ├── mod.rs
│   │   ├── detector.rs
│   │   ├── containerd.rs
│   │   └── docker.rs
│   └── nsenter/
│       ├── mod.rs
│       └── builder.rs
└── completions/
    ├── kubectl-pod-debug.bash
    ├── kubectl-pod-debug.fish
    └── _kubectl-pod-debug.ps1
```

## 5. Rust 依赖

| Crate | 版本 | 用途 |
|-------|------|------|
| `clap` | 4.x | CLI 参数解析（derive API + features: derive, env]） |
| `kube` | 0.9x | Kubernetes API 客户端 |
| `k8s-openapi` | 0.22.x | Kubernetes API 类型定义（feature gate: 按版本） |
| `kube-runtime` | 0.9x | Kubernetes 运行时辅助 |
| `russh` | 0.4x | 纯 Rust SSH2 客户端 |
| `tokio` | 1.x | 异步运行时（features: full） |
| `thiserror` | 2.x | 错误类型定义 |
| `eyre` | 0.6.x | 友好的错误报告 |
| `tracing` | 0.1.x | 结构化日志 |
| `tracing-subscriber` | 0.3.x | 日志格式化输出 |
| `serde_json` | 1.x | JSON 解析（crictl/docker inspect 输出） |
| `dirs` | 5.x | 获取用户 home 目录 |

## 6. 错误处理

### 6.1 统一错误类型

```rust
#[derive(Debug, thiserror::Error)]
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

    #[error("SSH error: {0}")]
    SshError(#[from] russh::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

### 6.2 错误展示策略

- 所有错误通过 `eyre` 提供友好的上下文信息
- `--verbose` 模式下显示完整的错误链和 backtrace
- 关键操作失败时提供修复建议（如"请检查 SSH 密钥权限"）

## 7. 安全设计

1. **SSH 密钥管理**：默认使用 `~/.ssh/id_rsa`，支持 SSH Agent 转发，不支持密码认证
2. **最小 K8s 权限**：只需 `get pods` 和 `get nodes` 的读取权限
3. **dry-run 模式**：`--dry-run` 只显示将要执行的命令，不实际执行
4. **审计日志**：`--verbose` 模式下记录完整操作链路

## 8. 分发

1. **主分发**：编译为静态二进制 `kubectl-pod-debug`，放入 `$PATH`
2. **Krew 索引**：后续可提交到 Krew 插件索引
3. **多平台**：通过 GitHub Actions 编译 Linux (x86_64/aarch64) + macOS (x86_64/aarch64)

## 9. 配置文件（可选）

路径：`~/.kube/pod-debug.yaml`

```yaml
ssh:
  user: root
  port: 22
  key: ~/.ssh/id_rsa
defaults:
  ns-type: all
  runtime: auto
```

配置优先级：CLI 参数 > 配置文件 > 默认值

## 10. 未来扩展（不在本次实现范围）

- 支持 Bastion/Jump Host 跳板 SSH
- 支持 CRI-O 运行时
- 抓包结果自动下载到本地
- 网络诊断自动化（连通性矩阵、DNS 解析链路分析）
- Krew 索引提交
