mod cli;
mod error;
mod k8s;
mod network;
mod nsenter;
mod report;
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
        println!("Diag:       {}", cli.diag);
        println!("Targets:    {:?}", cli.targets);
        println!("Runtime:    {}", cli.runtime);
        println!("Command:    {}", if cli.command.is_empty() { "/bin/bash (interactive)".to_string() } else { cli.command.join(" ") });
        return Ok(());
    }

    // 7. 建立 SSH 连接
    let session = ssh::connect::connect(
        &node_ip,
        cli.ssh_port,
        &cli.ssh_user,
        &cli.ssh_key,
        cli.ssh_password.as_deref(),
    )
    .await?;
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

    let procs = runtime::containerd::list_container_processes(&session, pid).await;
    if !procs.is_empty() {
        tracing::info!("=== Container Processes (host PID -> cmd) ===");
        println!("=== Container Processes (host PID -> cmd) ===");
        for (host_pid, cmdline) in procs {
            tracing::info!("  HOST_PID: {}  CMD: {}", host_pid, cmdline);
            println!("  HOST_PID: {}  CMD: {}", host_pid, cmdline);
        }
        println!();
    }

    // 10. 构建 nsenter 参数前缀
    let nsenter_arg = format!("nsenter -t {} -n", pid);
    let nsenter_cmd = nsenter::builder::build_nsenter_command(pid, &cli.ns_type, cli.enter_mount, &cli.command);
    tracing::info!("nsenter command: {}", nsenter_cmd);

    // 11. 诊断模式
    if cli.diag {
        let dns_names = if cli.command.is_empty() {
            let svc_name = guess_service_name(&cli.pod_name);
            vec![
                "kubernetes.default.svc.cluster.local".to_string(),
                format!("{}.{}.svc.cluster.local", svc_name, cli.namespace),
            ]
        } else {
            cli.command.clone()
        };
        let diag = network::NetworkDiag::run(
            &session,
            &k8s_client,
            &container_id,
            &node_ip,
            &nsenter_arg,
            &cli.namespace,
            pid,
            cli.targets.as_deref(),
            &dns_names,
        )
        .await;
        diag.print_report(&cli.pod_name, &cli.namespace, &node_name);
        return Ok(());
    }

    // 11. 报告生成模式
    if cli.report {
        let nsenter_arg = format!("nsenter -t {} -n", pid);
        let container_image = pod.spec.as_ref()
            .and_then(|s| s.containers.iter().find(|c| c.name == container_name))
            .and_then(|c| c.image.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string();

        let collector = report::ReportCollector::new(
            &session,
            &k8s_client,
            &container_id,
            pid,
            &nsenter_arg,
            &cli.pod_name,
            &cli.namespace,
            &node_name,
            &node_ip,
            &container_name,
            &container_image,
        );

        let report_data = collector.collect().await?;

        let formatted = report::ReportFormatter::format(&report_data, &cli.report_format);

        if let Some(ref path) = cli.report_output {
            std::fs::write(path, &formatted).map_err(|e| error::PodDebugError::Other {
                reason: format!("Failed to write report to '{}': {}", path, e),
            })?;
            println!("Report saved to: {}", path);
        } else {
            println!("{}", formatted);
        }
        return Ok(());
    }

    // 12. 交互式调试助手模式
    if cli.assist {
        let nsenter_arg = format!("nsenter -t {} -n", pid);
        let mut assistant = report::assist::DebugAssistant::new(
            session,
            nsenter_arg,
            pid,
            cli.pod_name.clone(),
            cli.namespace.clone(),
        );
        assistant.run().await?;
        return Ok(());
    }

    // 13. 抓包模式
    if cli.pcap {
        use std::path::PathBuf;
        let nsenter_arg = format!("nsenter -t {} -n", pid);
        let output_path = cli.pcap_output.as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(format!(
                    "/tmp/{}_capture_{}.pcap",
                    cli.pod_name,
                    chrono::Utc::now().format("%Y%m%d_%H%M%S")
                ))
            });

        let mut capture = report::pcap::PcapCapture::new(
            session,
            pid,
            nsenter_arg,
            cli.pcap_filter.clone(),
            cli.pcap_count,
            output_path,
        );

        let result = capture.capture().await?;
        println!("{}", report::pcap::format_pcap_result(&result));
        return Ok(());
    }

    // 14. 执行命令
    if cli.command.is_empty() {
        ssh::exec::interactive_shell(&session, &nsenter_cmd).await?;
    } else {
        let output = ssh::exec::exec_command(&session, &nsenter_cmd).await?;
        print!("{}", output);
    }

    Ok(())
}

fn guess_service_name(pod_name: &str) -> &str {
    let dashes: Vec<usize> = pod_name.match_indices('-').map(|(i, _)| i).collect();
    if dashes.len() >= 2 {
        &pod_name[..dashes[dashes.len() - 2]]
    } else {
        pod_name
    }
}
