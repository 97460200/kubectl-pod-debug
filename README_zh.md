<h1 align="center">
  <img src="https://img.shields.io/badge/kubectl--plugin-pod--debug-blue?style=for-the-badge" alt="kubectl plugin"/>
  <img src="https://img.shields.io/badge/Rust-1.95-orange?style=flat-square&logo=rust" alt="Rust"/>
  <img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="License"/>
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20macOS-lightgrey?style=flat-square" alt="Platform"/>
</h1>

<p align="center">
  <strong>通过 nsenter 在宿主机上调试 Pod 网络和进程的 kubectl 插件</strong><br/>
  无需拉起额外容器 — 直接使用宿主机原生调试工具
</p>

<p align="center">
  <a href="README.md">English</a> | <strong>中文</strong>
</p>

---

## 为什么选择 kubectl-pod-debug？

| | `kubectl debug` | **kubectl-pod-debug** |
|---|---|---|
| **原理** | 创建临时容器 | SSH → 宿主机 `nsenter` |
| **可用工具** | 受限于调试镜像 | 宿主机上所有原生工具 |
| **网络视角** | 共享 Pod 的 network namespace | 直接进入 Pod 的 network namespace |
| **额外资源** | 创建临时容器/Pod | **零开销** |
| **前置条件** | 需要 EphemeralContainer 特性 | 需要 SSH 访问节点 |

## 快速开始

```bash
# 一键安装
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-pod-debug/main/install.sh | bash

# 交互式进入 Pod 的所有 namespace
kubectl pod-debug my-pod -n my-namespace

# 抓包
kubectl pod-debug my-pod -- tcpdump -i eth0 -c 100

# 查看路由表
kubectl pod-debug my-pod --ns-type network -- ip route show
```

## 工作原理

```
kubectl pod-debug <pod>
       │
       ├─ 1. 查询 K8s API → 获取 Pod 信息（节点、容器ID）
       ├─ 2. SSH 到宿主机节点
       ├─ 3. 检测运行时（containerd/docker）→ 获取容器 PID
       ├─ 4. nsenter -t <PID> -n -p -u -i（不进入 mount，使用宿主机 /bin/bash）
       │    （添加 --enter-mount 可一并进入 mount namespace 查看容器文件系统）
       └─ 5. 你的调试命令在 Pod 的 namespace 中执行
```

## 安装

### 一键安装（推荐）

```bash
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-pod-debug/main/install.sh | bash
```

可选参数：

```bash
# 安装指定版本
curl -fsSL ... | bash -s -- --tag v0.1.0

# 自定义安装路径
curl -fsSL ... | bash -s -- --path /opt/bin

# 强制覆盖已有安装
curl -fsSL ... | bash -s -- --force
```

### 下载预编译二进制

| 平台 | 架构 | 文件 |
|------|------|------|
| Linux | x86_64 (AMD64) | `kubectl-pod-debug-linux-amd64` |
| Linux | ARM64 | `kubectl-pod-debug-linux-arm64` |
| macOS | x86_64 (Intel) | `kubectl-pod-debug-darwin-amd64` |
| macOS | ARM64 (Apple Silicon) | `kubectl-pod-debug-darwin-arm64` |

**最新版本**: https://github.com/97460200/kubectl-pod-debug/releases/latest

### 从源码编译

```bash
git clone https://github.com/97460200/kubectl-pod-debug.git
cd kubectl-pod-debug
cargo build --release
sudo cp target/release/kubectl-pod-debug /usr/local/bin/
```

## 使用示例

### 网络调试

```bash
# 交互式进入 Pod 的网络 namespace
kubectl pod-debug my-pod --ns-type network

# 在 eth0 上抓包
kubectl pod-debug my-pod -- tcpdump -i eth0 -w /tmp/capture.pcap

# 检查 DNS 解析
kubectl pod-debug my-pod --ns-type network -- nslookup kubernetes.default.svc.cluster.local

# 查看 iptables 规则
kubectl pod-debug my-pod --ns-type network -- iptables -L -n -v

# 测试连通性
kubectl pod-debug my-pod --ns-type network -- ping -c 3 10.96.0.1
```

### 进程调试

```bash
# 查看容器进程
kubectl pod-debug my-pod --ns-type pid -- ps aux

# 跟踪系统调用
kubectl pod-debug my-pod --ns-type pid -- strace -p 1

# 查看文件描述符
kubectl pod-debug my-pod --ns-type pid -- ls -la /proc/1/fd
```

### 全功能调试

```bash
# 进入除 mount 外的所有 namespace（默认 — 使用宿主机 /bin/bash）
kubectl pod-debug my-pod -n production

# 进入所有 namespace 包括 mount（容器文件系统 — 需要镜像中有 /bin/sh）
kubectl pod-debug my-pod -n production --enter-mount

# 指定多容器 Pod 中的某个容器
kubectl pod-debug my-pod -c sidecar-container

# Dry-run — 查看将要执行的命令
kubectl pod-debug my-pod --dry-run
```

## CLI 参数

| 参数 | 缩写 | 默认值 | 说明 |
|------|------|--------|------|
| `<POD_NAME>` | | （必填） | 目标 Pod 名称 |
| `--namespace` | `-n` | `default` | Kubernetes 命名空间 |
| `--container` | `-c` | 第一个容器 | 目标容器名称 |
| `--ssh-user` | | `root` | 连接节点使用的 SSH 用户 |
| `--ssh-key` | `-i` | `~/.ssh/id_rsa` | SSH 私钥路径 |
| `--ssh-port` | | `22` | SSH 端口 |
| `--ns-type` | | `all` | Namespace 类型：`network`、`pid`、`mount`、`uts`、`ipc`、`all` |
| `--enter-mount` | | `false` | 同时进入容器 mount namespace（容器文件系统） |
| `--runtime` | | `auto` | 容器运行时：`auto`、`containerd`、`docker` |
| `--kubeconfig` | | 自动检测 | kubeconfig 文件路径 |
| `--context` | | 当前 context | Kubernetes context |
| `--dry-run` | | `false` | 仅显示将要执行的命令 |
| `--verbose` | `-v` | `false` | 启用详细输出 |
| `-- <命令>` | | | 在 Pod namespace 中执行的命令（用 `--` 分隔） |

## 前置条件

- SSH 访问所有 Kubernetes 节点
- 节点上已安装 `crictl` 或 `docker`
- 节点上已安装 `nsenter`（大多数 Linux 发行版已包含）

## 技术栈

- **Rust** — 高性能、内存安全、单二进制分发
- **kube + k8s-openapi** — Kubernetes API 客户端
- **russh** — 纯 Rust SSH 客户端
- **clap** — CLI 参数解析
- **tokio** — 异步运行时

## 许可证

[MIT](LICENSE)

---

<p align="center">
  <sub>为 Kubernetes 开发者而造 ❤️</sub>
</p>
