<h1 align="center">kubectl-pod-debug</h1>

<p align="center">
  <strong>在宿主机上调试 Kubernetes Pod——无需额外容器。</strong><br/>
  SSH 到节点，nsenter 进入 Pod namespace，使用宿主机全部原生工具。
</p>

<p align="center">
  <a href="README.md">English</a>
</p>

---

## 快速开始

```bash
# 一键安装
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-pod-debug/main/install.sh | bash

# 交互式进入 Pod 的 namespace
kubectl pod debug my-pod -n my-ns

# 一键网络诊断
kubectl pod debug my-pod --diag --targets example.com:443

# 抓网络包
kubectl pod debug my-pod -- tcpdump -i eth0 -c 100

# 调试 Java（或任意语言）——进程列表会显示宿主机 PID
kubectl pod debug my-pod -v
# 输出: HOST_PID: 12345 CMD: java -jar app.jar
# 然后: ssh root@<node> "jstack 12345"
```

## 工作原理

```
kubectl pod debug <pod>
       │
       ├─ 1. 查询 K8s API → 获取 Pod 信息（节点、容器ID）
       ├─ 2. SSH 到宿主机节点
       ├─ 3. 检测运行时 → 获取容器 PID
       ├─ 4. 扫描 /proc/<pid>/ns/pid → 映射容器所有进程
       ├─ 5. nsenter -n -p -u -i（使用宿主机 /bin/bash，不进入 mount ns）
       └─ 6. 调试命令在 Pod 的 network/PID namespace 中执行
```

## 为什么选择 kubectl-pod-debug？

| | kubectl debug | kubectl-pod-debug |
|---|---|---|
| **原理** | 创建临时容器 | SSH → 宿主机 nsenter |
| **可用工具** | 受限于调试镜像 | 宿主机全部原生工具 |
| **额外资源** | 需创建临时容器/Pod | 零开销 |
| **前置条件** | EphemeralContainer 特性 | SSH 访问节点 |
| **进程视图** | 仅容器内部进程 | 包含宿主机 PID 映射 |
| **网络诊断** | 手动 | 自动化连通性矩阵 + DNS 链路分析 |

## 通用调试方案

`-v` 会列出容器所有进程及其宿主机 PID：

```
Container PID: 5782
=== Container Processes (host PID -> cmd) ===
  HOST_PID: 5782  CMD: /bin/prometheus ...
  HOST_PID: 5819  CMD: /bin/prometheus-config-reloader ...
```

拿到宿主机 PID 后，SSH 到节点直接用工具：

| 语言 | 命令 |
|------|------|
| Java | `ssh root@<node> "jstack 5782"` |
| Go | `ssh root@<node> "dlv attach 5782"` |
| .NET | `ssh root@<node> "dotnet-dump collect -p 5782"` |
| Python/C/Rust | `ssh root@<node> "gdb -p 5782"` |
| 任意 | `ssh root@<node> "strace -p 5782"` |

## 网络诊断

`--diag` 在 Pod 的网络 namespace 内自动执行连通性和 DNS 分析：

```bash
# 自动发现目标（环境变量、活跃连接、K8s 端点）
kubectl pod debug my-pod --diag

# 添加自定义目标
kubectl pod debug my-pod --diag --targets api.example.com:443,10.0.0.1:8080

# 自定义 DNS 测试名
kubectl pod debug my-pod --diag -- db.example.com redis.internal.svc.cluster.local
```

输出示例：

```
=== Network Diagnostics for pod 'my-pod/my-ns' on node 'k8s-node1' ===

--- Connectivity Matrix ---
TARGET                         PROTO  RESULT   LATENCY   ERROR
10.96.0.1:443                  TCP    ✅ OK    2.3ms
10.98.192.17:9090              TCP    ✅ OK    0.8ms
api.example.com:443            TCP    ❌ FAIL  3002ms    timed out

--- DNS Configuration ---
nameservers: 10.96.0.10
search: monitoring.svc.cluster.local svc.cluster.local cluster.local
ndots: 5

--- DNS Resolution for: kubernetes.default.svc.cluster.local (ndots=5) ---
  ✅  kubernetes.default.svc.cluster.local.  →  10.96.0.1  (0.8ms)
  Total: 1 query, 0.8ms, ✅ 10.96.0.1

--- DNS Resolution for: api.example.com (ndots=5) ---
  ❌  api.example.com.monitoring.svc.cluster.local.  →  NXDOMAIN  (0.6ms)
  ❌  api.example.com.svc.cluster.local.             →  NXDOMAIN  (0.7ms)
  ❌  api.example.com.cluster.local.                 →  NXDOMAIN  (0.5ms)
  ✅  api.example.com.                               →  93.184.216.34  (3.2ms)
  Total: 4 queries, 5.0ms, ✅ 93.184.216.34
```

DNS 分析会逐级展示 `ndots` 搜索域行为——这是排查"Pod 解析外部域名慢"的关键信息。

## 安装

### 一键安装

```bash
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-pod-debug/main/install.sh | bash
```

### 预编译二进制

| 平台 | 架构 | 文件 |
|------|------|------|
| Linux | AMD64 | `kubectl-pod-debug-linux-amd64` |
| Linux | ARM64 | `kubectl-pod-debug-linux-arm64` |
| macOS | Intel | `kubectl-pod-debug-darwin-amd64` |
| macOS | Apple Silicon | `kubectl-pod-debug-darwin-arm64` |

[最新 Release](https://github.com/97460200/kubectl-pod-debug/releases/latest)

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
# 只进入网络 namespace
kubectl pod debug my-pod --ns-type network

# 抓包
kubectl pod debug my-pod -- tcpdump -i eth0 -w /tmp/capture.pcap

# 检查 DNS
kubectl pod debug my-pod --ns-type network -- nslookup kubernetes.default

# 查看 iptables 规则
kubectl pod debug my-pod --ns-type network -- iptables -L -n -v
```

### 进程调试

```bash
# 查看容器进程
kubectl pod debug my-pod --ns-type pid -- ps --ppid 1 -o pid,comm

# 跟踪系统调用
kubectl pod debug my-pod --ns-type pid -- strace -p 1

# 文件描述符
kubectl pod debug my-pod --ns-type pid -- ls -la /proc/1/fd
```

### 全 namespace 调试

```bash
# 进入除 mount 外的所有 namespace（默认——使用宿主机 /bin/bash）
kubectl pod debug my-pod -n production

# 进入所有 namespace 包括 mount（容器 rootfs）
kubectl pod debug my-pod -n production --enter-mount

# Dry-run——预览
kubectl pod debug my-pod --dry-run
```

### 网络诊断

```bash
# 全自动发现
kubectl pod debug my-pod --diag

# 加上自定义目标
kubectl pod debug my-pod --diag --targets external-api.com:443,redis-cluster:6379

# 自定义 DNS 测试名
kubectl pod debug my-pod --diag -- mysql.internal.svc.cluster.local proxy.squid.internal
```

## CLI 参数

| 参数 | 缩写 | 默认值 | 说明 |
|------|------|--------|------|
| `<POD_NAME>` | | 必填 | 目标 Pod 名称 |
| `--namespace` | `-n` | `default` | Kubernetes 命名空间 |
| `--container` | `-c` | 第一个 | 目标容器名称 |
| `--ssh-user` | | `root` | SSH 用户 |
| `--ssh-key` | `-i` | `~/.ssh/id_rsa` | SSH 私钥路径 |
| `--ssh-port` | | `22` | SSH 端口 |
| `--ns-type` | | `all` | `network` `pid` `mount` `uts` `ipc` `all` |
| `--enter-mount` | | false | 同时进入 mount namespace |
| `--diag` | | false | 运行自动化网络诊断 |
| `--targets` | | | `--diag` 模式下追加的自定义目标，逗号分隔 |
| `--runtime` | | `auto` | `auto` `containerd` `docker` |
| `--kubeconfig` | | 自动 | kubeconfig 文件路径 |
| `--context` | | 当前 | Kubernetes context |
| `--dry-run` | | false | 仅预览 |
| `--verbose` | `-v` | false | 显示进程列表和日志 |

## 前置条件

- 可 SSH 密钥登录所有节点
- 节点上已安装 `nsenter`（大多数 Linux 发行版自带）
- 节点上已安装 `crictl` 或 `docker`
- 节点上已安装 `dig` 或 `nslookup`（`--diag` DNS 分析需要）

## 技术栈

- **Rust** — 安全、高性能、单二进制
- **kube + k8s-openapi** — K8s API 客户端
- **russh** — 纯 Rust 异步 SSH
- **clap** — 命令行参数解析
- **tokio** — 异步运行时

## 许可证

[MIT](LICENSE)
