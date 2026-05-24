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
