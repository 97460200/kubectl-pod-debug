<h1 align="center">kubectl-dbg</h1>

<p align="center">
  <strong>Debug Kubernetes pods from the host node — no extra containers needed.</strong><br/>
  SSH into the node, nsenter the pod's namespaces, use all host-native tools.
</p>

<p align="center">
  <a href="README_zh.md">中文</a>
</p>

---

## Quick Start

```bash
# Install
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-dbg/main/install.sh | bash

# Enter a pod's namespaces interactively
kubectl dbg my-pod -n my-ns

# One-click network diagnostics
kubectl dbg my-pod --diag --targets example.com:443

# Smart packet capture (downloads to local)
kubectl dbg my-pod --pcap --pcap-filter "tcp port 80"

# Interactive debugging assistant
kubectl dbg my-pod --assist

# AI-powered diagnosis
kubectl dbg my-pod --ai

# View pod event timeline
kubectl dbg my-pod --timeline

# Compare pod config with ReplicaSet
kubectl dbg my-pod --diff

# Debug Java (or any language) — process list shows host PIDs
kubectl dbg my-pod -v
# Output: HOST_PID: 12345 CMD: java -jar app.jar
# Then: ssh root@<node> "jstack 12345"
```

## How It Works

```
kubectl-dbg <pod>
       │
       ├─ 1. Query K8s API → pod info (node, containerID)
       ├─ 2. SSH to host node (key auth first, password fallback)
       ├─ 3. Detect runtime → get container PID
       ├─ 4. Scan /proc/<pid>/ns/pid → map all container processes
       ├─ 5. nsenter -n -p -u -i (host /bin/bash, no mount ns)
       └─ 6. Your debug commands run in pod's network/PID namespaces
```

## Why Not `kubectl debug`?

| | `kubectl debug` | `kubectl-dbg` |
|---|---|---|
| **Mechanism** | Creates ephemeral container | SSH → `nsenter` on host |
| **Tools** | Limited to debug image | All host-native tools |
| **Resources** | Extra container/Pod | Zero overhead |
| **Prerequisite** | EphemeralContainer feature gate | SSH access to node |
| **Process view** | Container-only | Host PID mapping included |
| **Network diag** | Manual | Automated matrix + DNS chain |
| **Packet capture** | Manual | Smart pcap download to local |
| **Interactive assist** | None | Guided troubleshooting menu |

## Universal Debugging

The `-v` flag prints every container process mapped to its host PID:

```
Container PID: 5782
=== Container Processes (host PID -> cmd) ===
  HOST_PID: 5782  CMD: /bin/prometheus ...
  HOST_PID: 5819  CMD: /bin/prometheus-config-reloader ...
```

Then use any host tool directly via SSH:

| Language | Command |
|----------|---------|
| Java | `ssh root@<node> "jstack 5782"` |
| Go | `ssh root@<node> "dlv attach 5782"` |
| .NET | `ssh root@<node> "dotnet-dump collect -p 5782"` |
| Python/C/Rust | `ssh root@<node> "gdb -p 5782"` |
| Any | `ssh root@<node> "strace -p 5782"` |

## Network Diagnostics

`--diag` runs automated connectivity and DNS analysis from inside the pod's network namespace:

```bash
# Auto-discover targets from env vars, active connections, and K8s endpoints
kubectl pod debug my-pod --diag

# Add custom targets
kubectl pod debug my-pod --diag --targets api.example.com:443,10.0.0.1:8080

# Custom DNS test names
kubectl pod debug my-pod --diag -- db.example.com redis.internal.svc.cluster.local
```

Example output:

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

The DNS analysis shows `ndots` search-domain behavior step-by-step — critical for diagnosing why a pod takes too long to resolve external names.

## Smart Packet Capture

`--pcap` captures network packets in the pod's network namespace and automatically downloads the PCAP file to your local machine:

```bash
# Capture 100 packets (default) and save to /tmp
kubectl-dbg my-pod --pcap

# Custom BPF filter
kubectl-dbg my-pod --pcap --pcap-filter "tcp port 8080"

# Capture more packets
kubectl-dbg my-pod --pcap --pcap-count 500

# Save to specific location
kubectl-dbg my-pod --pcap --pcap-output ~/captures/my-pod.pcap
```

The captured PCAP file can be opened with Wireshark or analyzed with `tshark`:

```bash
# Analyze with tshark
tshark -r /tmp/pod_capture_12345.pcap -z io,stat,1,"COUNT(frame)frame"
```

## Interactive Debugging Assistant

`--assist` launches an interactive debugging assistant with guided troubleshooting:

```bash
kubectl-dbg my-pod --assist
```

Features:
- **Auto-diagnosis**: Checks DNS resolution, network connectivity, container health
- **Command menu**: Quick access to common debugging commands
- **Problem detection**: Identifies common issues and suggests fixes
- **Session logging**: Saves diagnostic results to file

Example assistant menu:
```
╔══════════════════════════════════════════════════════════════╗
║               kubectl-dbg Interactive Assistant              ║
╚══════════════════════════════════════════════════════════════╝

Pod: my-pod | Namespace: default | Node: k8s-node1

=== Auto-Diagnosis Results ===
✅ DNS resolution working
✅ Kube API accessible
⚠️  High network latency detected

Choose an action:
1) Run network diagnostics
2) Capture packets
3) View process list
4) Check container logs
5) Exit

Enter choice [1-5]:
```

## AI-Powered Diagnosis

`--ai` calls AI to analyze Pod issues and provide diagnosis:

```bash
# Basic usage (requires Ollama or OpenAI API)
kubectl dbg my-pod --ai

# Specify model and endpoint
kubectl dbg my-pod --ai --ai-model gpt-4 --ai-endpoint http://localhost:11434/v1

# With API key for OpenAI
export OPENAI_API_KEY=your-key
export OPENAI_BASE_URL=https://api.openai.com/v1
kubectl dbg my-pod --ai
```

Example output:
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

## Timeline Debugging

`--timeline` shows Pod lifecycle events:

```bash
# View events in last 24 hours
kubectl dbg my-pod --timeline

# Custom time range (1h, 6h, 12h, 24h, 48h, 168h)
kubectl dbg my-pod --timeline --since 48h
```

Example output:
```
=== Pod Timeline (my-pod/default ===

2026-05-26 10:30:15  ✅  Pod created
2026-05-26 10:30:16  📍  Scheduled to node-1
2026-05-26 10:30:25  🚀  Container main started
2026-05-26 10:30:26  ✅  Container ready
2026-05-26 14:22:40  ⚠️   Liveness probe failed
2026-05-26 14:22:41  🔄  Container restarted

=== Container Restarts ===

Total restarts: 3
Last restart: 2026-05-26 14:22:41 (2 hours ago)
```

## Configuration Diff

`--diff` compares Pod actual config with ReplicaSet desired config:

```bash
kubectl dbg my-pod --diff
```

Example output:
```
=== Configuration Diff ===
Namespace: default
Pod: my-pod-7f8d9c6b5-x2p8q
ReplicaSet: my-pod-7f8d9c6b5

🔴 Image Mismatch
   Pod:     nginx:1.19
   RS:      nginx:1.21
   Impact:  可能运行旧版本镜像

⚠️ CPU Limit
   Pod:     500m
   RS:      1000m
   Impact:  资源限制低于预期，可能导致性能问题

✅ All other settings match
```

## Installation

### One-line

```bash
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-dbg/main/install.sh | bash
```

### Pre-built Binaries

| Platform | Architecture | File |
|----------|-------------|------|
| Linux | AMD64 | `kubectl-dbg-linux-amd64` |
| Linux | ARM64 | `kubectl-dbg-linux-arm64` |
| macOS | Intel | `kubectl-dbg-darwin-amd64` |
| macOS | Apple Silicon | `kubectl-dbg-darwin-arm64` |

[Latest Release](https://github.com/97460200/kubectl-dbg/releases/latest)

### From Source

```bash
git clone https://github.com/97460200/kubectl-dbg.git
cd kubectl-dbg
cargo build --release
sudo cp target/release/kubectl-dbg /usr/local/bin/
```

## Usage

### Network Debugging

```bash
# Enter network namespace only
kubectl-dbg my-pod --ns-type network

# Check DNS
kubectl-dbg my-pod --ns-type network -- nslookup kubernetes.default

# View iptables rules
kubectl-dbg my-pod --ns-type network -- iptables -L -n -v
```

### Process Debugging

```bash
# View container processes
kubectl-dbg my-pod --ns-type pid -- ps --ppid 1 -o pid,comm

# Trace system calls
kubectl-dbg my-pod --ns-type pid -- strace -p 1

# File descriptors
kubectl-dbg my-pod --ns-type pid -- ls -la /proc/1/fd
```

### Full Namespace Debugging

```bash
# Enter all except mount (default — access host /bin/bash)
kubectl-dbg my-pod -n production

# Enter all including mount (container rootfs)
kubectl-dbg my-pod -n production --enter-mount

# Dry run — preview
kubectl-dbg my-pod --dry-run
```

### Network Diagnostics

```bash
# Full auto-discovery
kubectl-dbg my-pod --diag

# With extra targets
kubectl-dbg my-pod --diag --targets external-api.com:443,redis-cluster:6379

# Custom DNS names
kubectl-dbg my-pod --diag -- mysql.internal.svc.cluster.local proxy.squid.internal
```

### SSH Password Authentication

If key-based authentication fails, kubectl-dbg will prompt for password:

```bash
# Will prompt for password if key auth fails
kubectl-dbg my-pod

# Provide password via argument
kubectl-dbg my-pod --ssh-password mypassword

# Specify SSH port
kubectl-dbg my-pod --ssh-port 2222
```

## CLI Reference

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `<POD_NAME>` | | required | Target pod name |
| `--namespace` | `-n` | `default` | Kubernetes namespace |
| `--container` | `-c` | first | Target container |
| `--ssh-user` | | `root` | SSH user |
| `--ssh-key` | `-i` | `~/.ssh/id_rsa` | SSH private key |
| `--ssh-password` | | | SSH password (prompts if key auth fails) |
| `--ssh-port` | | `22` | SSH port |
| `--ns-type` | | `all` | `network` `pid` `mount` `uts` `ipc` `all` |
| `--enter-mount` | | false | Include mount namespace |
| `--diag` | | false | Run automated network diagnostics |
| `--targets` | | | Comma-separated extra targets for `--diag` |
| `--pcap` | | false | Capture network packets in pod namespace |
| `--pcap-filter` | | | BPF filter for packet capture |
| `--pcap-count` | | `100` | Number of packets to capture |
| `--pcap-output` | | auto | Output path for PCAP file |
| `--assist` | | false | Launch interactive debugging assistant |
| `--ai` | | false | Enable AI-powered diagnosis |
| `--ai-model` | | `gpt-4` | AI model name |
| `--ai-endpoint` | | | AI API endpoint URL |
| `--ai-key` | | | AI API key |
| `--timeline` | | false | Show pod event timeline |
| `--since` | | `24h` | Time range for timeline |
| `--diff` | | false | Compare pod config with ReplicaSet |
| `--force` | | false | Force execution of risky operations |
| `--runtime` | | `auto` | `auto` `containerd` `docker` |
| `--kubeconfig` | | auto | kubeconfig path |
| `--context` | | current | Kubernetes context |
| `--dry-run` | | false | Preview only |
| `--verbose` | `-v` | false | Show process list + logs |

## Prerequisites

- SSH key-based access to all nodes
- `nsenter` on nodes (included in most Linux distros)
- `crictl` or `docker` on nodes
- `dig` or `nslookup` on nodes (for `--diag` DNS analysis)

## Tech Stack

- **Rust** — safe, fast, single binary
- **kube + k8s-openapi** — K8s API client
- **russh** — async SSH in pure Rust
- **clap** — CLI argument parsing
- **tokio** — async runtime

## License

[MIT](LICENSE)
