use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::Result;
use crate::ssh::connect::SshClient;
use kube::Client;
use russh::client::Handle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub meta: ReportMeta,
    pub pod_info: PodInfo,
    pub container_info: ContainerInfo,
    pub resources: ResourceUsage,
    pub network: NetworkDiagnostics,
    pub processes: Vec<ProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMeta {
    pub tool_version: String,
    pub generated_at: DateTime<Utc>,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodInfo {
    pub name: String,
    pub namespace: String,
    pub node: String,
    pub node_ip: String,
    pub status: String,
    pub ip: Option<String>,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub name: String,
    pub image: String,
    pub image_id: Option<String>,
    pub runtime: String,
    pub pid: u32,
    pub created: Option<String>,
    pub ports: Vec<String>,
    pub limits: ResourceLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub cpu_limit: Option<String>,
    pub memory_limit: Option<String>,
    pub cpu_request: Option<String>,
    pub memory_request: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_usage: String,
    pub memory_usage: String,
    pub memory_working_set: String,
    pub network_rx_bytes: String,
    pub network_tx_bytes: String,
    pub fs_reads: String,
    pub fs_writes: String,
    pub io_throttle: Option<IoThrottle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoThrottle {
    pub read_iops: String,
    pub write_iops: String,
    pub read_bps: String,
    pub write_bps: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDiagnostics {
    pub connectivity: Vec<ConnectivityResult>,
    pub dns_config: DnsConfig,
    pub dns_queries: Vec<DnsQueryResult>,
    pub listening_ports: Vec<String>,
    pub active_connections: Vec<ConnectionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityResult {
    pub target: String,
    pub success: bool,
    pub latency_ms: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub nameservers: Vec<String>,
    pub search: Vec<String>,
    pub ndots: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsQueryResult {
    pub name: String,
    pub queries: Vec<DnsQuery>,
    pub success: bool,
    pub final_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsQuery {
    pub query: String,
    pub result: String,
    pub success: bool,
    pub latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub local: String,
    pub remote: String,
    pub state: String,
    pub proto: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub command: String,
    pub cpu_percent: Option<String>,
    pub memory_mb: Option<String>,
}

pub struct ReportCollector<'a> {
    session: &'a Handle<SshClient>,
    k8s_client: &'a Client,
    container_id: &'a str,
    container_pid: u32,
    nsenter_arg: &'a str,
    pod_name: &'a str,
    namespace: &'a str,
    node_name: &'a str,
    node_ip: &'a str,
    container_name: &'a str,
    container_image: &'a str,
}

impl<'a> ReportCollector<'a> {
    pub fn new(
        session: &'a Handle<SshClient>,
        k8s_client: &'a Client,
        container_id: &'a str,
        container_pid: u32,
        nsenter_arg: &'a str,
        pod_name: &'a str,
        namespace: &'a str,
        node_name: &'a str,
        node_ip: &'a str,
        container_name: &'a str,
        container_image: &'a str,
    ) -> Self {
        Self {
            session,
            k8s_client,
            container_id,
            container_pid,
            nsenter_arg,
            pod_name,
            namespace,
            node_name,
            node_ip,
            container_name,
            container_image,
        }
    }

    pub async fn collect(&self) -> Result<DiagnosticReport> {
        println!("[1/5] Collecting pod and container info...");
        let pod_info = self.collect_pod_info().await?;
        let container_info = self.collect_container_info().await?;

        println!("[2/5] Collecting resource usage...");
        let resources = self.collect_resources().await?;

        println!("[3/5] Collecting network diagnostics...");
        let network = self.collect_network().await?;

        println!("[4/5] Collecting process list...");
        let processes = self.collect_processes().await?;

        println!("[5/5] Generating report...");

        let report = DiagnosticReport {
            meta: ReportMeta {
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                generated_at: Utc::now(),
                hostname: hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "unknown".to_string()),
            },
            pod_info,
            container_info,
            resources,
            network,
            processes,
        };

        Ok(report)
    }

    async fn collect_pod_info(&self) -> Result<PodInfo> {
        use crate::k8s;
        let pod = k8s::pod::get_pod(self.k8s_client, self.pod_name, self.namespace).await?;

        let status = pod.status.as_ref()
            .and_then(|s| s.phase.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let pod_ip = pod.status.as_ref()
            .and_then(|s| s.pod_ip.as_ref())
            .map(|s| s.as_str().to_string());

        let labels = pod.metadata.labels.as_ref()
            .map(|l| l.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let annotations = pod.metadata.annotations.as_ref()
            .map(|a| a.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        Ok(PodInfo {
            name: self.pod_name.to_string(),
            namespace: self.namespace.to_string(),
            node: self.node_name.to_string(),
            node_ip: self.node_ip.to_string(),
            status,
            ip: pod_ip,
            labels,
            annotations,
        })
    }

    async fn collect_container_info(&self) -> Result<ContainerInfo> {
        let limits = ResourceLimits {
            cpu_limit: None,
            memory_limit: None,
            cpu_request: None,
            memory_request: None,
        };

        let ports = self.get_container_ports().await.unwrap_or_default();
        let runtime = self.detect_runtime().await;

        Ok(ContainerInfo {
            name: self.container_name.to_string(),
            image: self.container_image.to_string(),
            image_id: None,
            runtime,
            pid: self.container_pid,
            created: None,
            ports,
            limits,
        })
    }

    async fn get_container_ports(&self) -> Result<Vec<String>> {
        let cmd = format!(
            "{} nslookup {} 2>/dev/null || echo 'not_found'",
            self.nsenter_arg, self.pod_name
        );
        Ok(vec![])
    }

    async fn detect_runtime(&self) -> String {
        "auto".to_string()
    }

    async fn collect_resources(&self) -> Result<ResourceUsage> {
        let cgroup_path = format!("/proc/{}/cgroup", self.container_pid);

        let cpu_cmd = self.exec_in_container("cat /sys/fs/cgroup/cpu/cpuacct.usage 2>/dev/null || echo '0'").await;
        let mem_cmd = self.exec_in_container("cat /sys/fs/cgroup/memory/memory.usage_in_bytes 2>/dev/null || cat /sys/fs/cgroup/memory/memory.current 2>/dev/null || echo '0'").await;
        let mem_wset = self.exec_in_container("cat /sys/fs/cgroup/memory/memory.working_set_bytes 2>/dev/null || echo '0'").await;
        let net_rx = self.exec_in_container("cat /sys/class/net/eth0/statistics/rx_bytes 2>/dev/null || echo '0'").await;
        let net_tx = self.exec_in_container("cat /sys/class/net/eth0/statistics/tx_bytes 2>/dev/null || echo '0'").await;

        let cpu_usage = self.format_bytes(&cpu_cmd.trim().to_string());
        let memory_usage = self.format_bytes(&mem_cmd.trim().to_string());
        let memory_working_set = self.format_bytes(&mem_wset.trim().to_string());

        Ok(ResourceUsage {
            cpu_usage,
            memory_usage,
            memory_working_set,
            network_rx_bytes: net_rx.trim().to_string(),
            network_tx_bytes: net_tx.trim().to_string(),
            fs_reads: "0".to_string(),
            fs_writes: "0".to_string(),
            io_throttle: None,
        })
    }

    fn format_bytes(&self, s: &str) -> String {
        if let Ok(n) = s.parse::<u64>() {
            if n > 1_000_000_000 {
                format!("{:.2} GB", n as f64 / 1_000_000_000.0)
            } else if n > 1_000_000 {
                format!("{:.2} MB", n as f64 / 1_000_000.0)
            } else if n > 1_000 {
                format!("{:.2} KB", n as f64 / 1_000.0)
            } else {
                format!("{} B", n)
            }
        } else {
            s.to_string()
        }
    }

    async fn collect_network(&self) -> Result<NetworkDiagnostics> {
        use crate::network::connectivity::check_connectivity;
        use crate::network::dns::{read_resolv_conf, resolve_dns_chain};
        use crate::network::targets::parse_user_targets;

        let default_targets = vec![
            "kubernetes.default.svc.cluster.local:443".to_string(),
            "kubernetes.default.svc.cluster.local:80".to_string(),
        ];

        let targets = parse_user_targets(&default_targets.join(","));
        let connectivity = check_connectivity(self.session, self.nsenter_arg, &targets, 1).await;

        let connectivity_results = connectivity.into_iter().map(|r| ConnectivityResult {
            target: r.target.to_string(),
            success: r.ok,
            latency_ms: r.latency_ms,
            error: if r.ok { None } else { Some(r.error) },
        }).collect();

        let resolv = read_resolv_conf(self.session, self.container_pid).await;

        let dns_config = DnsConfig {
            nameservers: resolv.nameservers.clone(),
            search: resolv.search_domains.clone(),
            ndots: resolv.ndots,
        };

        let dns_names = vec![
            "kubernetes.default.svc.cluster.local".to_string(),
        ];

        let mut dns_queries = Vec::new();
        for name in &dns_names {
            let result = resolve_dns_chain(self.session, self.nsenter_arg, name, &resolv).await;
            dns_queries.push(DnsQueryResult {
                name: name.clone(),
                queries: result.steps.iter().map(|s| DnsQuery {
                    query: s.query_name.clone(),
                    result: s.result.clone(),
                    success: s.ok,
                    latency_ms: s.latency_ms,
                }).collect(),
                success: result.resolved,
                final_ip: result.final_ip,
            });
        }

        let listening_ports = self.get_listening_ports().await.unwrap_or_default();
        let active_connections = self.get_connections().await.unwrap_or_default();

        Ok(NetworkDiagnostics {
            connectivity: connectivity_results,
            dns_config,
            dns_queries,
            listening_ports,
            active_connections,
        })
    }

    async fn get_listening_ports(&self) -> Result<Vec<String>> {
        let output = self.exec_in_container("ss -tlnp 2>/dev/null || netstat -tlnp 2>/dev/null || echo ''").await;
        Ok(output.lines().map(|s| s.to_string()).collect())
    }

    async fn get_connections(&self) -> Result<Vec<ConnectionInfo>> {
        let output = self.exec_in_container("ss -tnp 2>/dev/null || netstat -tnp 2>/dev/null || echo ''").await;
        let mut connections = Vec::new();
        for line in output.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                connections.push(ConnectionInfo {
                    local: parts.get(3).unwrap_or(&"").to_string(),
                    remote: parts.get(4).unwrap_or(&"").to_string(),
                    state: parts.get(1).unwrap_or(&"").to_string(),
                    proto: parts.get(0).unwrap_or(&"").to_string(),
                });
            }
        }
        Ok(connections)
    }

    async fn collect_processes(&self) -> Result<Vec<ProcessInfo>> {
        let output = self.exec_in_container(
            "ps -eo pid,ppid,comm --no-headers 2>/dev/null | head -50"
        ).await;

        let mut processes = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                if let (Ok(pid), Ok(ppid)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                    processes.push(ProcessInfo {
                        pid,
                        ppid,
                        command: parts[2..].join(" "),
                        cpu_percent: None,
                        memory_mb: None,
                    });
                }
            }
        }
        Ok(processes)
    }

    async fn exec_in_container(&self, cmd: &str) -> String {
        use crate::ssh::exec::exec_command;
        let full_cmd = format!("{} {}", self.nsenter_arg, cmd);
        exec_command(self.session, &full_cmd).await.unwrap_or_default()
    }
}
