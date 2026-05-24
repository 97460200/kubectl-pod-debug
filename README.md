<h1 align="center">
  <img src="https://img.shields.io/badge/kubectl--plugin-pod--debug-blue?style=for-the-badge" alt="kubectl plugin"/>
  <img src="https://img.shields.io/badge/Rust-1.95-orange?style=flat-square&logo=rust" alt="Rust"/>
  <img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="License"/>
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20macOS-lightgrey?style=flat-square" alt="Platform"/>
</h1>

<p align="center">
  <strong>kubectl plugin for debugging pod network/process via nsenter on host node</strong><br/>
  No extra container needed — use host-native debug tools directly.
</p>

---

## Why kubectl-pod-debug?

| | `kubectl debug` | **kubectl-pod-debug** |
|---|---|---|
| **How** | Creates ephemeral container | SSH → `nsenter` on host |
| **Tools** | Limited to debug image | All host-native tools |
| **Network view** | Shared pod namespace | Direct pod namespace |
| **Extra resources** | Creates temp container/Pod | **Zero overhead** |
| **Prerequisite** | EphemeralContainer feature | SSH access to nodes |

## Quick Start

```bash
# One-line install
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-pod-debug/main/install.sh | bash

# Enter pod's all namespaces interactively
kubectl pod-debug my-pod -n my-namespace

# Capture network packets
kubectl pod-debug my-pod -- tcpdump -i eth0 -c 100

# Check routing table
kubectl pod-debug my-pod --ns-type network -- ip route show
```

## How It Works

```
kubectl pod-debug <pod>
       │
       ├─ 1. Query K8s API → get pod info (node, containerID)
       ├─ 2. SSH to host node
       ├─ 3. Detect runtime (containerd/docker) → get container PID
       ├─ 4. nsenter -t <PID> -a
       └─ 5. Your debug commands run inside pod's namespaces
```

## Installation

### One-line Install (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/97460200/kubectl-pod-debug/main/install.sh | bash
```

Options:

```bash
# Install specific version
curl -fsSL ... | bash -s -- --tag v0.1.0

# Custom install path
curl -fsSL ... | bash -s -- --path /opt/bin

# Force overwrite
curl -fsSL ... | bash -s -- --force
```

### Download Pre-built Binaries

| Platform | Architecture | File |
|----------|-------------|------|
| Linux | x86_64 (AMD64) | `kubectl-pod-debug-linux-amd64` |
| Linux | ARM64 | `kubectl-pod-debug-linux-arm64` |
| macOS | x86_64 (Intel) | `kubectl-pod-debug-darwin-amd64` |
| macOS | ARM64 (Apple Silicon) | `kubectl-pod-debug-darwin-arm64` |

**Latest Release**: https://github.com/97460200/kubectl-pod-debug/releases/latest

### Build from Source

```bash
git clone https://github.com/97460200/kubectl-pod-debug.git
cd kubectl-pod-debug
cargo build --release
sudo cp target/release/kubectl-pod-debug /usr/local/bin/
```

## Usage Examples

### Network Debugging

```bash
# Enter pod's network namespace interactively
kubectl pod-debug my-pod --ns-type network

# Capture packets on eth0
kubectl pod-debug my-pod -- tcpdump -i eth0 -w /tmp/capture.pcap

# Check DNS resolution
kubectl pod-debug my-pod --ns-type network -- nslookup kubernetes.default.svc.cluster.local

# View iptables rules
kubectl pod-debug my-pod --ns-type network -- iptables -L -n -v

# Test connectivity
kubectl pod-debug my-pod --ns-type network -- ping -c 3 10.96.0.1
```

### Process Debugging

```bash
# View container processes
kubectl pod-debug my-pod --ns-type pid -- ps aux

# Trace system calls
kubectl pod-debug my-pod --ns-type pid -- strace -p 1

# View file descriptors
kubectl pod-debug my-pod --ns-type pid -- ls -la /proc/1/fd
```

### Full Namespace Debugging

```bash
# Enter all namespaces (default)
kubectl pod-debug my-pod -n production

# Specify container in multi-container pod
kubectl pod-debug my-pod -c sidecar-container

# Dry run — see what would be executed
kubectl pod-debug my-pod --dry-run
```

## CLI Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `<POD_NAME>` | | (required) | Target pod name |
| `--namespace` | `-n` | `default` | Kubernetes namespace |
| `--container` | `-c` | first container | Target container name |
| `--ssh-user` | | `root` | SSH user for node connection |
| `--ssh-key` | `-i` | `~/.ssh/id_rsa` | SSH private key path |
| `--ssh-port` | | `22` | SSH port |
| `--ns-type` | | `all` | Namespace: `network`, `pid`, `mount`, `uts`, `ipc`, `all` |
| `--runtime` | | `auto` | Container runtime: `auto`, `containerd`, `docker` |
| `--kubeconfig` | | auto | Path to kubeconfig file |
| `--context` | | current | Kubernetes context |
| `--dry-run` | | `false` | Show commands without executing |
| `--verbose` | `-v` | `false` | Enable verbose output |
| `-- <cmd>` | | | Command to execute (use `--` to separate) |

## Prerequisites

- SSH access to all Kubernetes nodes
- `crictl` or `docker` installed on nodes
- `nsenter` installed on nodes (included in most Linux distributions)

## Tech Stack

- **Rust** — performance, safety, single binary distribution
- **kube + k8s-openapi** — Kubernetes API client
- **russh** — pure Rust SSH client
- **clap** — CLI argument parsing
- **tokio** — async runtime

## License

[MIT](LICENSE)

---

<p align="center">
  <sub>Built with ❤️ for Kubernetes developers</sub>
</p>
