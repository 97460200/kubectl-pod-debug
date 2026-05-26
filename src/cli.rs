use clap::Parser;

/// kubectl plugin for debugging pod network/process via nsenter on host node.
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

    /// SSH password (if not provided, will prompt for password if key auth fails)
    #[arg(long)]
    pub ssh_password: Option<String>,

    /// SSH port
    #[arg(long, default_value_t = 22)]
    pub ssh_port: u16,

    /// Namespace type to enter: network, pid, mount, uts, ipc, all
    #[arg(long, default_value = "all", value_parser = ["network", "pid", "mount", "uts", "ipc", "all"])]
    pub ns_type: String,

    /// Also enter container mount namespace (default: use host /bin/bash without mount ns)
    #[arg(long)]
    pub enter_mount: bool,

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

    /// Run automated network diagnostics (connectivity matrix + DNS chain)
    #[arg(long)]
    pub diag: bool,

    /// Generate comprehensive diagnostic report (network + resources + config)
    #[arg(long)]
    pub report: bool,

    /// Output format for --report: text, json
    #[arg(long, default_value = "text", value_parser = ["text", "json"])]
    pub report_format: String,

    /// Save report to file instead of stdout
    #[arg(long)]
    pub report_output: Option<String>,

    /// Comma-separated extra targets for --diag, e.g. example.com:443,10.0.0.1:8080
    #[arg(long)]
    pub targets: Option<String>,

    /// Command to execute inside the pod's namespace (use -- to separate from flags)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}
