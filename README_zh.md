<h1 align="center">kubectl-dbg</h1>

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
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-dbg/main/install.sh | bash

# 交互式进入 Pod 的 namespace
kubectl dbg my-pod -n my-ns

# 一键网络诊断
kubectl dbg my-pod --diag --targets example.com:443

# 智能网络抓包（自动下载到本地）
kubectl dbg my-pod --pcap --pcap-filter "tcp port 80"

# 交互式调试助手
kubectl dbg my-pod --assist

# AI 智能诊断
kubectl dbg my-pod --ai

# 查看 Pod 事件时间线
kubectl dbg my-pod --timeline

# 对比 Pod 与 ReplicaSet 配置
kubectl dbg my-pod --diff

# 调试 Java（或任意语言）——进程列表会显示宿主机 PID
kubectl dbg my-pod -v
# 输出: HOST_PID: 12345 CMD: java -jar app.jar
# 然后: ssh root@<node> "jstack 12345"
```

## 工作原理

```
kubectl-dbg <pod>
       │
       ├─ 1. 查询 K8s API → 获取 Pod 信息（节点、容器ID）
       ├─ 2. SSH 到宿主机节点（优先密钥认证，失败后密码认证）
       ├─ 3. 检测运行时 → 获取容器 PID
       ├─ 4. 扫描 /proc/<pid>/ns/pid → 映射容器所有进程
       ├─ 5. nsenter -n -p -u -i（使用宿主机 /bin/bash，不进入 mount ns）
       └─ 6. 调试命令在 Pod 的 network/PID namespace 中执行
```

## 为什么选择 kubectl-dbg？

| | kubectl debug | kubectl-dbg |
|---|---|---|
| **原理** | 创建临时容器 | SSH → 宿主机 nsenter |
| **可用工具** | 受限于调试镜像 | 宿主机全部原生工具 |
| **额外资源** | 需创建临时容器/Pod | 零开销 |
| **前置条件** | EphemeralContainer 特性 | SSH 访问节点 |
| **进程视图** | 仅容器内部进程 | 包含宿主机 PID 映射 |
| **网络诊断** | 手动 | 自动化连通性矩阵 + DNS 链路分析 |
| **网络抓包** | 手动 | 智能 PCAP 文件下载到本地 |
| **交互助手** | 无 | 引导式故障排查菜单 |

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

## 智能网络抓包

`--pcap` 在 Pod 的网络 namespace 中捕获网络数据包，并自动将 PCAP 文件下载到本地：

```bash
# 捕获 100 个数据包（默认）并保存到 /tmp
kubectl-dbg my-pod --pcap

# 使用自定义 BPF 过滤器
kubectl-dbg my-pod --pcap --pcap-filter "tcp port 8080"

# 捕获更多数据包
kubectl-dbg my-pod --pcap --pcap-count 500

# 保存到指定位置
kubectl-dbg my-pod --pcap --pcap-output ~/captures/my-pod.pcap
```

捕获的 PCAP 文件可以用 Wireshark 打开或用 `tshark` 分析：

```bash
# 使用 tshark 分析
tshark -r /tmp/pod_capture_12345.pcap -z io,stat,1,"COUNT(frame)frame"
```

## 交互式调试助手

`--assist` 启动交互式调试助手，提供引导式故障排查：

```bash
kubectl-dbg my-pod --assist
```

功能特性：
- **自动诊断**：检查 DNS 解析、网络连通性、容器健康状态
- **命令菜单**：快速访问常用调试命令
- **问题检测**：识别常见问题并提供修复建议
- **会话记录**：将诊断结果保存到文件

助手菜单示例：
```
╔══════════════════════════════════════════════════════════════╗
║              kubectl-dbg 交互式调试助手                      ║
╚══════════════════════════════════════════════════════════════╝

Pod: my-pod | 命名空间: default | 节点: k8s-node1

=== 自动诊断结果 ===
✅ DNS 解析正常
✅ Kube API 可访问
⚠️  检测到高网络延迟

选择操作：
1) 运行网络诊断
2) 捕获网络包
3) 查看进程列表
4) 检查容器日志
5) 退出

输入选择 [1-5]:
```

## AI 智能诊断

`--ai` 调用 AI 分析 Pod 问题并提供诊断：

```bash
# 基本用法（需要 Ollama 或 OpenAI API）
kubectl dbg my-pod --ai

# 指定模型和端点
kubectl dbg my-pod --ai --ai-model gpt-4 --ai-endpoint http://localhost:11434/v1

# 通过环境变量配置 OpenAI
export OPENAI_API_KEY=your-key
export OPENAI_BASE_URL=https://api.openai.com/v1
kubectl dbg my-pod --ai
```

输出示例：
```
## 诊断结论
Pod 处于 Running 状态，但容器不断重启

## 可能原因
1. 应用启动脚本失败
2. 健康检查配置不当
3. 资源限制过低

## 修复建议
1. 检查容器日志：`kubectl logs my-pod`
2. 增加资源限制
3. 调整 liveness probe 参数
```

## 时间旅行调试

`--timeline` 展示 Pod 生命周期事件：

```bash
# 查看最近 24 小时的事件
kubectl dbg my-pod --timeline

# 自定义时间范围 (1h, 6h, 12h, 24h, 48h, 168h)
kubectl dbg my-pod --timeline --since 48h
```

输出示例：
```
=== Pod Timeline (my-pod/default ===

2026-05-26 10:30:15  ✅  Pod 已创建
2026-05-26 10:30:16  📍  调度到 node-1
2026-05-26 10:30:25  🚀  容器 main 已启动
2026-05-26 10:30:26  ✅  容器 Ready
2026-05-26 14:22:40  ⚠️   存活探针失败
2026-05-26 14:22:41  🔄  容器已重启

=== 容器重启记录 ===

总重启次数: 3
上次重启: 2026-05-26 14:22:41 (2 小时前)
```

## Pod 配置对比

`--diff` 对比 Pod 实际配置与 ReplicaSet 期望配置：

```bash
kubectl dbg my-pod --diff
```

输出示例：
```
=== 配置对比 ===
命名空间: default
Pod: my-pod-7f8d9c6b5-x2p8q
ReplicaSet: my-pod-7f8d9c6b5

🔴 镜像版本不匹配
   Pod:     nginx:1.19
   RS:      nginx:1.21
   影响:  可能运行旧版本镜像

⚠️ CPU 限制
   Pod:     500m
   RS:      1000m
   影响:  资源限制低于预期，可能导致性能问题

✅ 其他配置一致
```

## 安装

### 一键安装

```bash
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-dbg/main/install.sh | bash
```

### 预编译二进制

| 平台 | 架构 | 文件 |
|------|------|------|
| Linux | AMD64 | `kubectl-dbg-linux-amd64` |
| Linux | ARM64 | `kubectl-dbg-linux-arm64` |
| macOS | Intel | `kubectl-dbg-darwin-amd64` |
| macOS | Apple Silicon | `kubectl-dbg-darwin-arm64` |

[最新 Release](https://github.com/97460200/kubectl-dbg/releases/latest)

### 从源码编译

```bash
git clone https://github.com/97460200/kubectl-dbg.git
cd kubectl-dbg
cargo build --release
sudo cp target/release/kubectl-dbg /usr/local/bin/
```

## 使用示例

### 网络调试

```bash
# 只进入网络 namespace
kubectl-dbg my-pod --ns-type network

# 检查 DNS
kubectl-dbg my-pod --ns-type network -- nslookup kubernetes.default

# 查看 iptables 规则
kubectl-dbg my-pod --ns-type network -- iptables -L -n -v
```

### 进程调试

```bash
# 查看容器进程
kubectl-dbg my-pod --ns-type pid -- ps --ppid 1 -o pid,comm

# 跟踪系统调用
kubectl-dbg my-pod --ns-type pid -- strace -p 1

# 文件描述符
kubectl-dbg my-pod --ns-type pid -- ls -la /proc/1/fd
```

### 全 namespace 调试

```bash
# 进入除 mount 外的所有 namespace（默认——使用宿主机 /bin/bash）
kubectl-dbg my-pod -n production

# 进入所有 namespace 包括 mount（容器 rootfs）
kubectl-dbg my-pod -n production --enter-mount

# Dry-run——预览
kubectl-dbg my-pod --dry-run
```

### 网络诊断

```bash
# 全自动发现
kubectl-dbg my-pod --diag

# 加上自定义目标
kubectl-dbg my-pod --diag --targets external-api.com:443,redis-cluster:6379

# 自定义 DNS 测试名
kubectl-dbg my-pod --diag -- mysql.internal.svc.cluster.local proxy.squid.internal
```

### SSH 密码认证

如果密钥认证失败，kubectl-dbg 会提示输入密码：

```bash
# 密钥认证失败后会提示输入密码
kubectl-dbg my-pod

# 通过参数提供密码
kubectl-dbg my-pod --ssh-password mypassword

# 指定 SSH 端口
kubectl-dbg my-pod --ssh-port 2222
```

## CLI 参数

| 参数 | 缩写 | 默认值 | 说明 |
|------|------|--------|------|
| `<POD_NAME>` | | 必填 | 目标 Pod 名称 |
| `--namespace` | `-n` | `default` | Kubernetes 命名空间 |
| `--container` | `-c` | 第一个 | 目标容器名称 |
| `--ssh-user` | | `root` | SSH 用户 |
| `--ssh-key` | `-i` | `~/.ssh/id_rsa` | SSH 私钥路径 |
| `--ssh-password` | | | SSH 密码（密钥认证失败时会提示） |
| `--ssh-port` | | `22` | SSH 端口 |
| `--ns-type` | | `all` | `network` `pid` `mount` `uts` `ipc` `all` |
| `--enter-mount` | | false | 同时进入 mount namespace |
| `--diag` | | false | 运行自动化网络诊断 |
| `--targets` | | | `--diag` 模式下追加的自定义目标，逗号分隔 |
| `--pcap` | | false | 在 Pod namespace 中捕获网络包 |
| `--pcap-filter` | | | BPF 过滤器 |
| `--pcap-count` | | `100` | 捕获数据包数量 |
| `--pcap-output` | | 自动 | PCAP 文件输出路径 |
| `--assist` | | false | 启动交互式调试助手 |
| `--ai` | | false | 启用 AI 智能诊断 |
| `--ai-model` | | `gpt-4` | AI 模型名称 |
| `--ai-endpoint` | | | AI API 端点 URL |
| `--ai-key` | | | AI API Key |
| `--timeline` | | false | 显示 Pod 事件时间线 |
| `--since` | | `24h` | 时间线时间范围 |
| `--diff` | | false | 对比 Pod 与 ReplicaSet 配置 |
| `--force` | | false | 强制执行高风险操作 |
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
