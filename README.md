# kubectl-pod-debug

A kubectl plugin for debugging pod network/process via nsenter on the host node.

## How it works

1. Connects to the Kubernetes API to get pod info (node name, container ID)
2. SSHs into the pod's host node
3. Detects the container runtime (containerd/docker) and gets the container PID
4. Uses `nsenter` to enter the pod's Linux namespaces
5. Executes debug commands or opens an interactive shell

## Installation

### Download Pre-built Binaries

Download the latest release for your platform:

| Platform | Architecture | Download |
|----------|-------------|----------|
| Linux | x86_64 (AMD64) | `kubectl-pod-debug-linux-amd64` |
| Linux | ARM64 | `kubectl-pod-debug-linux-arm64` |
| macOS | x86_64 (Intel) | `kubectl-pod-debug-darwin-amd64` |
| macOS | ARM64 (Apple Silicon) | `kubectl-pod-debug-darwin-arm64` |

**Latest Release**: https://github.com/97460200/kubectl-pod-debug/releases/latest

```bash
# Example: Install on Linux AMD64
curl -sL https://github.com/97460200/kubectl-pod-debug/releases/latest/download/kubectl-pod-debug-linux-amd64 -o kubectl-pod-debug
chmod +x kubectl-pod-debug
sudo mv kubectl-pod-debug /usr/local/bin/
```

### Build from Source

```bash
git clone https://github.com/97460200/kubectl-pod-debug.git
cd kubectl-pod-debug
cargo build --release
sudo cp target/release/kubectl-pod-debug /usr/local/bin/
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

## CLI Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--namespace` | `-n` | `default` | Kubernetes namespace |
| `--container` | `-c` | first container | Target container name |
| `--ssh-user` | | `root` | SSH user for node connection |
| `--ssh-key` | `-i` | `~/.ssh/id_rsa` | SSH private key path |
| `--ssh-port` | | `22` | SSH port |
| `--ns-type` | | `all` | Namespace type: `network`, `pid`, `mount`, `uts`, `ipc`, `all` |
| `--runtime` | | `auto` | Container runtime: `auto`, `containerd`, `docker` |
| `--kubeconfig` | | auto | Path to kubeconfig file |
| `--context` | | current | Kubernetes context |
| `--dry-run` | | `false` | Show commands without executing |
| `--verbose` | `-v` | `false` | Enable verbose output |

## Prerequisites

- SSH access to all Kubernetes nodes
- `crictl` or `docker` installed on nodes
- `nsenter` installed on nodes

## License

MIT
