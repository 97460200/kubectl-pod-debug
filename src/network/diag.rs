use crate::network::connectivity::{check_connectivity, ConnectivityResult};
use crate::network::dns::{read_resolv_conf, resolve_dns_chain, DnsResult, ResolvConf};
use crate::network::resolve::enrich_with_k8s_resources;
use crate::network::targets::{auto_discover, parse_user_targets, Target};
use crate::ssh::connect::SshClient;
use kube::Client;
use russh::client::Handle;

pub struct NetworkDiag {
    targets: Vec<Target>,
    connectivity: Vec<ConnectivityResult>,
    resolv: ResolvConf,
    dns_results: Vec<DnsResult>,
    nsenter_arg: String,
}

impl NetworkDiag {
    pub async fn run(
        session: &Handle<SshClient>,
        k8s_client: &Client,
        container_id: &str,
        host_ip: &str,
        nsenter_arg: &str,
        namespace: &str,
        container_pid: u32,
        user_targets: Option<&str>,
        dns_names: &[String],
    ) -> Self {
        let mut targets = auto_discover(session, container_id, host_ip, nsenter_arg).await;
        if let Some(raw) = user_targets {
            targets.extend(parse_user_targets(raw));
        }

        let mut connectivity = check_connectivity(session, nsenter_arg, &targets, 3).await;

        enrich_with_k8s_resources(k8s_client, namespace, &mut connectivity).await;

        let resolv = read_resolv_conf(session, container_pid).await;

        let mut dns_results = Vec::new();
        for name in dns_names {
            dns_results.push(resolve_dns_chain(session, nsenter_arg, name, &resolv).await);
        }

        NetworkDiag {
            targets,
            connectivity,
            resolv,
            dns_results,
            nsenter_arg: nsenter_arg.to_string(),
        }
    }

    pub fn print_report(&self, pod_name: &str, namespace: &str, node_name: &str) {
        println!(
            "\n=== Network Diagnostics for pod '{}/{}' on node '{}' ===\n",
            namespace, pod_name, node_name
        );

        self.print_connectivity();
        self.print_dns();
    }

    fn print_connectivity(&self) {
        println!("--- Connectivity Matrix ---");
        println!(
            "{:<30} {:<6} {:<8} {:<10} {:<28} {}",
            "TARGET", "PROTO", "RESULT", "LATENCY", "K8S RESOURCE", "ERROR"
        );

        let mut sorted = self.connectivity.clone();
        sorted.sort_by(|a, b| a.target.cmp(&b.target));

        for r in &sorted {
            let status = if r.ok { "✅ OK" } else { "❌ FAIL" };
            let error = if r.ok { "" } else { &r.error };
            println!(
                "{:<30} {:<6} {:<8} {:<8.1}ms {:<28} {}",
                r.target.to_string(),
                "TCP",
                status,
                r.latency_ms,
                r.resource,
                error
            );
        }
        println!();
    }

    fn print_dns(&self) {
        println!("--- DNS Configuration ---");
        println!("nameservers: {}", self.resolv.nameservers.join(" "));
        if !self.resolv.search_domains.is_empty() {
            println!("search: {}", self.resolv.search_domains.join(" "));
            println!("ndots: {}", self.resolv.ndots);
        }
        println!();

        for dns in &self.dns_results {
            println!(
                "--- DNS Resolution for: {} (ndots={}) ---",
                dns.test_hostname, dns.ndots
            );
            for step in &dns.steps {
                let status = if step.ok { "✅" } else { "❌" };
                println!(
                    "  {}  {:<50}  →  {:<20}  ({:.1}ms)",
                    status, step.query_name, step.result, step.latency_ms
                );
            }
            let resolved = if dns.resolved {
                format!("✅ {}", dns.final_ip.as_deref().unwrap_or("unknown"))
            } else {
                "❌ not resolved".to_string()
            };
            println!("  Total: {} queries, {:.1}ms, {}", dns.steps.len(), dns.total_ms, resolved);
            println!();
        }
    }
}
