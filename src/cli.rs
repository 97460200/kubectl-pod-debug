use clap::Parser;

/// Advanced Kubernetes pod debugging tool - SSH to node, nsenter to pod namespace
#[derive(Parser, Debug)]
#[command(name = "kubectl-dbg", version, about)]
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

    /// Capture network packets in Pod namespace (use --pcap-filter for BPF filter)
    #[arg(long)]
    pub pcap: bool,

    /// BPF filter for packet capture, e.g. "tcp port 80" or "host example.com"
    #[arg(long, default_value = "")]
    pub pcap_filter: String,

    /// Number of packets to capture (default: 100)
    #[arg(long, default_value_t = 100)]
    pub pcap_count: usize,

    /// Save pcap file to this path (default: auto-generated in /tmp)
    #[arg(long)]
    pub pcap_output: Option<String>,

    /// Interactive debugging assistant with guided troubleshooting
    #[arg(long)]
    pub assist: bool,

    /// Enable AI-powered diagnosis (requires OPENAI_* env vars or --ai-* flags)
    #[arg(long)]
    pub ai: bool,

    /// AI model name (default: gpt-4)
    #[arg(long, default_value = "gpt-4")]
    pub ai_model: String,

    /// AI API endpoint URL (e.g., http://localhost:11434/v1 for Ollama)
    #[arg(long)]
    pub ai_endpoint: Option<String>,

    /// AI API key (or use OPENAI_API_KEY env var)
    #[arg(long)]
    pub ai_key: Option<String>,

    /// Enable timeline view of pod events
    #[arg(long)]
    pub timeline: bool,

    /// Time range for timeline (e.g., 1h, 6h, 24h, 48h, 168h)
    #[arg(long, default_value = "24h")]
    pub since: String,

    /// Enable config diff between pod and ReplicaSet
    #[arg(long)]
    pub diff: bool,

    /// Force execution of potentially risky operations
    #[arg(long)]
    pub force: bool,

    /// Command to execute inside the pod's namespace (use -- to separate from flags)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}
