mod cli;
mod error;
mod k8s;
mod nsenter;
mod runtime;
mod ssh;

use cli::Cli;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> error::Result<()> {
    let cli = Cli::parse();

    // 2. 初始化 tracing
    let filter = if cli.verbose {
        EnvFilter::new("kubectl_pod_debug=debug")
    } else {
        EnvFilter::new("kubectl_pod_debug=warn")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    tracing::info!("kubectl-pod-debug starting");
    tracing::debug!("CLI args: {:?}", cli);

    // 3. 构建 K8s Client
    let k8s_client = k8s::client::build_client(&cli).await?;
    tracing::info!("Kubernetes client initialized");

    // 4. 获取 Pod 信息，确定容器名和 container ID
    let pod = k8s::pod::get_pod(&k8s_client, &cli.pod_name, &cli.namespace).await?;
    tracing::info!("Pod '{}' found in namespace '{}'", cli.pod_name, cli.namespace);

    let container_name = cli
        .container
        .clone()
        .unwrap_or_else(|| k8s::pod::get_first_container_name(&pod).unwrap_or_default());
    tracing::info!("Using container: {}", container_name);

    let container_id = k8s::pod::get_container_id(&pod, &container_name)?;
    tracing::info!("Container ID: {}", container_id);

    // 5. 获取节点 IP
    let node_name = k8s::pod::get_node_name(&pod)?;
    let node_ip = k8s::node::get_node_ip(&k8s_client, &node_name).await?;
    tracing::info!("Pod is running on node '{}' ({})", node_name, node_ip);

    // 6. dry-run 模式：打印信息并返回
    if cli.dry_run {
        println!("=== Dry Run ===");
        println!("Pod:        {}", cli.pod_name);
        println!("Namespace:  {}", cli.namespace);
        println!("Container:  {}", container_name);
        println!("Container ID: {}", container_id);
        println!("Node:       {} ({})", node_name, node_ip);
        println!("SSH:        {}@{}:{}", cli.ssh_user, node_ip, cli.ssh_port);
        println!("NS Type:    {}", cli.ns_type);
        println!("Enter Mount: {}", cli.enter_mount);
        println!("Runtime:    {}", cli.runtime);
        println!("Command:    {}", if cli.command.is_empty() { "/bin/bash (interactive)".to_string() } else { cli.command.join(" ") });
        return Ok(());
    }

    // 7. 建立 SSH 连接
    let session = ssh::connect::connect(&node_ip, cli.ssh_port, &cli.ssh_user, &cli.ssh_key).await?;
    tracing::info!("SSH connection established to {}", node_ip);

    // 8. 检测/确定容器运行时
    let runtime_type = if cli.runtime == "auto" {
        let detected = runtime::detector::detect_runtime(&session, &node_name).await?;
        tracing::info!("Detected container runtime: {:?}", detected);
        detected
    } else {
        let rt = match cli.runtime.as_str() {
            "containerd" => runtime::RuntimeType::Containerd,
            "docker" => runtime::RuntimeType::Docker,
            _ => unreachable!("clap already validates runtime value"),
        };
        tracing::info!("Using specified container runtime: {:?}", rt);
        rt
    };

    // 9. 获取容器 PID
    let pid = match runtime_type {
        runtime::RuntimeType::Containerd => {
            runtime::containerd::get_container_pid(&session, &container_id).await?
        }
        runtime::RuntimeType::Docker => {
            runtime::docker::get_container_pid(&session, &container_id).await?
        }
    };
    tracing::info!("Container PID: {}", pid);

    // 10. 构建 nsenter 命令
    let nsenter_cmd = nsenter::builder::build_nsenter_command(pid, &cli.ns_type, cli.enter_mount, &cli.command);
    tracing::info!("nsenter command: {}", nsenter_cmd);

    // 11. 执行命令
    if cli.command.is_empty() {
        ssh::exec::interactive_shell(&session, &nsenter_cmd).await?;
    } else {
        let output = ssh::exec::exec_command(&session, &nsenter_cmd).await?;
        print!("{}", output);
    }

    Ok(())
}
