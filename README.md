<h1 align="center">kubectl-pod-debug</h1>

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
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-pod-debug/main/install.sh | bash

# Enter a pod's namespaces interactively
kubectl pod debug my-pod -n my-ns

# One-click network diagnostics
kubectl pod debug my-pod --diag --targets example.com:443

# Capture network packets
kubectl pod debug my-pod -- tcpdump -i eth0 -c 100

# Debug Java (or any language) — process list shows host PIDs
kubectl pod debug my-pod -v
# Output: HOST_PID: 12345 CMD: java -jar app.jar
# Then: ssh root@<node> "jstack 12345"
```

## How It Works

```
kubectl pod debug <pod>
       │
       ├─ 1. Query K8s API → pod info (node, containerID)
       ├─ 2. SSH to host node
       ├─ 3. Detect runtime → get container PID
       ├─ 4. Scan /proc/<pid>/ns/pid → map all container processes
       ├─ 5. nsenter -n -p -u -i (host /bin/bash, no mount ns)
       └─ 6. Your debug commands run in pod's network/PID namespaces
```

## Why Not `kubectl debug`?

| | `kubectl debug` | `kubectl-pod-debug` |
|---|---|---|
| **Mechanism** | Creates ephemeral container | SSH → `nsenter` on host |
| **Tools** | Limited to debug image | All host-native tools |
| **Resources** | Extra container/Pod | Zero overhead |
| **Prerequisite** | EphemeralContainer feature gate | SSH access to node |
| **Process view** | Container-only | Host PID mapping included |
| **Network diag** | Manual | Automated matrix + DNS chain |

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

## Installation

### One-line

```bash
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-pod-debug/main/install.sh | bash
```

### Pre-built Binaries

| Platform | Architecture | File |
|----------|-------------|------|
| Linux | AMD64 | `kubectl-pod-debug-linux-amd64` |
| Linux | ARM64 | `kubectl-pod-debug-linux-arm64` |
| macOS | Intel | `kubectl-pod-debug-darwin-amd64` |
| macOS | Apple Silicon | `kubectl-pod-debug-darwin-arm64` |

[Latest Release](https://github.com/97460200/kubectl-pod-debug/releases/latest)

### From Source

```bash
git clone https://github.com/97460200/kubectl-pod-debug.git
cd kubectl-pod-debug
cargo build --release
sudo cp target/release/kubectl-pod-debug /usr/local/bin/
```

## Usage

### Network Debugging

```bash
# Enter network namespace only
kubectl pod debug my-pod --ns-type network

# Capture packets
kubectl pod debug my-pod -- tcpdump -i eth0 -w /tmp/capture.pcap

# Check DNS
kubectl pod debug my-pod --ns-type network -- nslookup kubernetes.default

# View iptables rules
kubectl pod debug my-pod --ns-type network -- iptables -L -n -v
```

### Process Debugging

```bash
# View container processes
kubectl pod debug my-pod --ns-type pid -- ps --ppid 1 -o pid,comm

# Trace system calls
kubectl pod debug my-pod --ns-type pid -- strace -p 1

# File descriptors
kubectl pod debug my-pod --ns-type pid -- ls -la /proc/1/fd
```

### Full Namespace Debugging

```bash
# Enter all except mount (default — access host /bin/bash)
kubectl pod debug my-pod -n production

# Enter all including mount (container rootfs)
kubectl pod debug my-pod -n production --enter-mount

# Dry run — preview
kubectl pod debug my-pod --dry-run
```

### Network Diagnostics

```bash
# Full auto-discovery
kubectl pod debug my-pod --diag

# With extra targets
kubectl pod debug my-pod --diag --targets external-api.com:443,redis-cluster:6379

# Custom DNS names
kubectl pod debug my-pod --diag -- mysql.internal.svc.cluster.local proxy.squid.internal
```

## CLI Reference

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `<POD_NAME>` | | required | Target pod name |
| `--namespace` | `-n` | `default` | Kubernetes namespace |
| `--container` | `-c` | first | Target container |
| `--ssh-user` | | `root` | SSH user |
| `--ssh-key` | `-i` | `~/.ssh/id_rsa` | SSH private key |
| `--ssh-port` | | `22` | SSH port |
| `--ns-type` | | `all` | `network` `pid` `mount` `uts` `ipc` `all` |
| `--enter-mount` | | false | Include mount namespace |
| `--diag` | | false | Run automated network diagnostics |
| `--targets` | | | Comma-separated extra targets for `--diag` |
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
