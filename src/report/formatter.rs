use crate::report::collector::DiagnosticReport;
use serde_json;

pub struct ReportFormatter;

impl ReportFormatter {
    pub fn format(report: &DiagnosticReport, format: &str) -> String {
        match format {
            "json" => Self::format_json(report),
            _ => Self::format_text(report),
        }
    }

    fn format_text(report: &DiagnosticReport) -> String {
        let mut output = String::new();

        output.push_str(&format!("{}\n", "═".repeat(80)));
        output.push_str(&format!("{:^80}\n", "KUBERNETES POD DIAGNOSTIC REPORT"));
        output.push_str(&format!("{}\n", "═".repeat(80)));

        output.push_str(&format!("\n📋 Report Meta\n"));
        output.push_str(&format!("  Tool Version:  v{}\n", report.meta.tool_version));
        output.push_str(&format!("  Generated:     {}\n", report.meta.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        output.push_str(&format!("  Hostname:      {}\n", report.meta.hostname));

        output.push_str(&format!("\n📦 Pod Information\n"));
        output.push_str(&format!("  Name:          {}\n", report.pod_info.name));
        output.push_str(&format!("  Namespace:     {}\n", report.pod_info.namespace));
        output.push_str(&format!("  Status:        {}\n", report.pod_info.status));
        output.push_str(&format!("  Node:          {} ({})\n", report.pod_info.node, report.pod_info.node_ip));
        if let Some(ref ip) = report.pod_info.ip {
            output.push_str(&format!("  Pod IP:        {}\n", ip));
        }
        if !report.pod_info.labels.is_empty() {
            output.push_str(&format!("  Labels:        {:?}\n", report.pod_info.labels));
        }

        output.push_str(&format!("\n🐳 Container Information\n"));
        output.push_str(&format!("  Name:          {}\n", report.container_info.name));
        output.push_str(&format!("  Image:         {}\n", report.container_info.image));
        output.push_str(&format!("  Runtime:       {}\n", report.container_info.runtime));
        output.push_str(&format!("  PID:           {}\n", report.container_info.pid));
        if !report.container_info.ports.is_empty() {
            output.push_str(&format!("  Ports:         {:?}\n", report.container_info.ports));
        }

        output.push_str(&format!("\n📊 Resource Usage\n"));
        output.push_str(&format!("  CPU Usage:     {}\n", report.resources.cpu_usage));
        output.push_str(&format!("  Memory Usage:  {}\n", report.resources.memory_usage));
        output.push_str(&format!("  Memory Working Set: {}\n", report.resources.memory_working_set));
        output.push_str(&format!("  Network RX:    {} bytes\n", report.resources.network_rx_bytes));
        output.push_str(&format!("  Network TX:    {} bytes\n", report.resources.network_tx_bytes));

        if let Some(ref io) = report.resources.io_throttle {
            output.push_str(&format!("  IO Read:       {} IOPS, {} B/s\n", io.read_iops, io.read_bps));
            output.push_str(&format!("  IO Write:      {} IOPS, {} B/s\n", io.write_iops, io.write_bps));
        }

        output.push_str(&format!("\n🌐 Network Connectivity\n"));
        if report.network.connectivity.is_empty() {
            output.push_str("  No connectivity tests performed\n");
        } else {
            output.push_str(&format!("  {:<45} {:>8} {:>12}\n", "TARGET", "RESULT", "LATENCY"));
            output.push_str(&format!("  {}\n", "-".repeat(65)));
            for conn in &report.network.connectivity {
                let status = if conn.success { "✅ OK" } else { "❌ FAIL" };
                output.push_str(&format!(
                    "  {:<45} {:>8} {:>10.2}ms\n",
                    conn.target,
                    status,
                    conn.latency_ms
                ));
                if let Some(ref err) = conn.error {
                    output.push_str(&format!("    Error: {}\n", err));
                }
            }
        }

        output.push_str(&format!("\n🔍 DNS Configuration\n"));
        output.push_str(&format!("  Nameservers:  {}\n", report.network.dns_config.nameservers.join(", ")));
        output.push_str(&format!("  Search:        {}\n", report.network.dns_config.search.join(" ")));
        output.push_str(&format!("  ndots:         {}\n", report.network.dns_config.ndots));

        if !report.network.dns_queries.is_empty() {
            output.push_str(&format!("\n📡 DNS Queries\n"));
            for query in &report.network.dns_queries {
                let status = if query.success { "✅" } else { "❌" };
                output.push_str(&format!("  {} {}\n", status, query.name));
                for step in &query.queries {
                    let s = if step.success { "✅" } else { "❌" };
                    output.push_str(&format!("    {} {:<50} → {:<20} ({:.1}ms)\n",
                        s, step.query, step.result, step.latency_ms));
                }
                if let Some(ref ip) = query.final_ip {
                    output.push_str(&format!("    Final IP: {}\n", ip));
                }
            }
        }

        if !report.network.listening_ports.is_empty() {
            output.push_str(&format!("\n🔌 Listening Ports\n"));
            for port in report.network.listening_ports.iter().take(20) {
                output.push_str(&format!("  {}\n", port));
            }
        }

        if !report.network.active_connections.is_empty() {
            output.push_str(&format!("\n📡 Active Connections (Top 20)\n"));
            for conn in report.network.active_connections.iter().take(20) {
                output.push_str(&format!("  {} {} -> {} [{}]\n",
                    conn.proto, conn.local, conn.remote, conn.state));
            }
        }

        output.push_str(&format!("\n⚙️ Process List (Top 30)\n"));
        output.push_str(&format!("  {:>8} {:>8} {}\n", "PID", "PPID", "COMMAND"));
        output.push_str(&format!("  {}\n", "-".repeat(60)));
        for proc in report.processes.iter().take(30) {
            let cmd = if proc.command.len() > 40 {
                format!("{}...", &proc.command[..37])
            } else {
                proc.command.clone()
            };
            output.push_str(&format!("  {:>8} {:>8} {}\n", proc.pid, proc.ppid, cmd));
        }

        output.push_str(&format!("\n{}\n", "═".repeat(80)));
        output.push_str(&format!("{:^80}\n", "END OF REPORT"));
        output.push_str(&format!("{}\n", "═".repeat(80)));

        output
    }

    fn format_json(report: &DiagnosticReport) -> String {
        serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
    }
}
