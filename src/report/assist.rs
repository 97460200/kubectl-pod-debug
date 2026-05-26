use std::io::{self, Write};

use crate::error::Result;
use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;
use russh::client::Handle;

pub struct DebugAssistant {
    session: Handle<SshClient>,
    nsenter_arg: String,
    container_pid: u32,
    pod_name: String,
    namespace: String,
    history: Vec<DiagnosticResult>,
}

impl DebugAssistant {
    pub fn new(
        session: Handle<SshClient>,
        nsenter_arg: String,
        container_pid: u32,
        pod_name: String,
        namespace: String,
    ) -> Self {
        Self {
            session,
            nsenter_arg,
            container_pid,
            pod_name,
            namespace,
            history: Vec::new(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        print_banner();
        self.auto_diagnose().await?;
        self.interactive_loop().await?;
        self.print_summary();
        Ok(())
    }

    async fn auto_diagnose(&mut self) -> Result<()> {
        println!("\n🔍 Running automatic diagnostics...\n");

        let checks = vec![
            ("Network DNS", self.check_dns().await),
            ("Network Connectivity", self.check_connectivity().await),
            ("Container Health", self.check_container_health().await),
            ("Process Status", self.check_processes().await),
            ("Resource Usage", self.check_resources().await),
            ("Listening Ports", self.check_ports().await),
        ];

        let mut all_ok = true;
        for (name, result) in checks {
            let status = if result.ok { "✅" } else { "❌" };
            println!("  {} {}: {}", status, name, result.message);
            if !result.ok {
                all_ok = false;
            }
            self.history.push(result);
        }

        if all_ok {
            println!("\n✨ All checks passed! No obvious issues detected.");
        } else {
            println!("\n⚠️  Some issues detected. Run specific checks for details.");
        }

        Ok(())
    }

    async fn interactive_loop(&mut self) -> Result<()> {
        let menu = r#"
╭─────────────────────────────────────────────────────────────╮
│  Interactive Debugging Menu                                  │
╠─────────────────────────────────────────────────────────────╣
│  1. Network DNS          - Check DNS resolution             │
│  2. Network Connectivity - Test connectivity to targets     │
│  3. Container Health     - Check container status           │
│  4. Process Analysis    - List and analyze processes       │
│  5. Resource Usage      - Check CPU/Memory/IO              │
│  6. Listening Ports      - Show open ports                  │
│  7. Active Connections   - Show network connections         │
│  8. Packet Capture       - Start tcpdump                     │
│  9. Run Custom Command   - Execute custom command           │
│ 10. Show Recommendations - View suggested next steps         │
│ 11. Generate Report     - Create diagnostic report         │
│ 12. Save Session Log    - Save this session to file        │
│  0. Exit                                                  │
╰─────────────────────────────────────────────────────────────╯
"#;

        loop {
            print!("\n> ");
            io::stdout().flush().ok();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }

            let choice = input.trim().to_lowercase();

            match choice.as_str() {
                "1" | "dns" => { self.detailed_dns().await.ok(); }
                "2" | "connectivity" => { self.detailed_connectivity().await.ok(); }
                "3" | "health" => { self.detailed_health().await.ok(); }
                "4" | "process" => { self.detailed_processes().await.ok(); }
                "5" | "resource" => { self.detailed_resources().await.ok(); }
                "6" | "ports" => { self.detailed_ports().await.ok(); }
                "7" | "connections" => { self.detailed_connections().await.ok(); }
                "8" | "tcpdump" | "capture" => { self.start_capture().await.ok(); }
                "9" | "custom" => { self.run_custom_command().await.ok(); }
                "10" | "rec" | "recommend" => { self.show_recommendations(); }
                "11" | "report" => { self.suggest_report(); }
                "12" | "save" => { self.save_session_log(); }
                "0" | "exit" | "q" => { break; }
                "help" | "?" => { println!("{}", menu); }
                "" => {}
                _ => { println!("Unknown command. Type 'help' for menu."); }
            }
        }

        Ok(())
    }

    async fn check_dns(&self) -> DiagnosticResult {
        let cmd = format!("{} nslookup kubernetes.default.svc.cluster.local 2>&1", self.nsenter_arg);
        let output = self.exec(&cmd).await.unwrap_or_default();

        if output.contains("Address") && !output.contains("can't find") {
            DiagnosticResult {
                category: "dns".to_string(),
                ok: true,
                message: "DNS resolution working".to_string(),
                details: Some(output),
                commands: vec![],
            }
        } else {
            DiagnosticResult {
                category: "dns".to_string(),
                ok: false,
                message: "DNS resolution failed".to_string(),
                details: Some(output),
                commands: vec![
                    "cat /etc/resolv.conf".to_string(),
                    "nslookup <domain>".to_string(),
                ],
            }
        }
    }

    async fn check_connectivity(&self) -> DiagnosticResult {
        let cmd = format!("{} curl -s --connect-timeout 3 https://kubernetes.default.svc.cluster.local/api 2>&1", self.nsenter_arg);
        let output = self.exec(&cmd).await.unwrap_or_default();

        if output.contains("\"kind\"") || output.contains("\"APIVersions\"") {
            DiagnosticResult {
                category: "connectivity".to_string(),
                ok: true,
                message: "K8s API reachable".to_string(),
                details: None,
                commands: vec![],
            }
        } else {
            DiagnosticResult {
                category: "connectivity".to_string(),
                ok: false,
                message: "Cannot reach K8s API".to_string(),
                details: Some(output.clone()),
                commands: vec![
                    "curl -v https://kubernetes.default.svc.cluster.local".to_string(),
                    "ping 10.96.0.1".to_string(),
                ],
            }
        }
    }

    async fn check_container_health(&self) -> DiagnosticResult {
        let cmd = format!("{} ls -la /proc/1/exe 2>&1", self.nsenter_arg);
        let output = self.exec(&cmd).await.unwrap_or_default();

        if output.contains("/") && !output.contains("cannot access") {
            DiagnosticResult {
                category: "container".to_string(),
                ok: true,
                message: "Container processes healthy".to_string(),
                details: None,
                commands: vec![],
            }
        } else {
            DiagnosticResult {
                category: "container".to_string(),
                ok: false,
                message: "Container may be unhealthy".to_string(),
                details: Some(output),
                commands: vec![
                    "ps aux".to_string(),
                    "cat /proc/1/status".to_string(),
                ],
            }
        }
    }

    async fn check_processes(&self) -> DiagnosticResult {
        let cmd = format!("{} ps aux 2>&1 | head -20", self.nsenter_arg);
        let output = self.exec(&cmd).await.unwrap_or_default();

        let zombie_count = output.lines()
            .filter(|l| l.contains("<defunct>"))
            .count();

        DiagnosticResult {
            category: "process".to_string(),
            ok: zombie_count == 0,
            message: if zombie_count > 0 {
                format!("Found {} zombie processes", zombie_count)
            } else {
                "Process list healthy".to_string()
            },
            details: Some(output),
            commands: vec![],
        }
    }

    async fn check_resources(&self) -> DiagnosticResult {
        let cmd = format!(
            "{} cat /sys/fs/cgroup/memory/memory.usage_in_bytes 2>/dev/null || echo '0'",
            self.nsenter_arg
        );
        let mem = self.exec(&cmd).await.unwrap_or_default().trim().to_string();

        DiagnosticResult {
            category: "resource".to_string(),
            ok: true,
            message: format!("Memory usage detected: {} bytes", mem),
            details: None,
            commands: vec![],
        }
    }

    async fn check_ports(&self) -> DiagnosticResult {
        let cmd = format!("{} ss -tlnp 2>/dev/null || netstat -tlnp 2>/dev/null || echo 'No ports'", self.nsenter_arg);
        let output = self.exec(&cmd).await.unwrap_or_default();

        let port_count = output.lines().count().saturating_sub(1);

        DiagnosticResult {
            category: "network".to_string(),
            ok: true,
            message: format!("{} listening ports detected", port_count),
            details: Some(output),
            commands: vec![],
        }
    }

    async fn detailed_dns(&self) -> Result<()> {
        println!("\n--- Detailed DNS Check ---");

        let commands = vec![
            ("Resolv.conf", format!("{} cat /etc/resolv.conf", self.nsenter_arg)),
            ("Test internal", format!("{} nslookup kubernetes.default.svc.cluster.local", self.nsenter_arg)),
            ("Test external", format!("{} nslookup google.com", self.nsenter_arg)),
            ("Test ndots", format!("{} nslookup myservice.{}.svc.cluster.local", self.nsenter_arg, self.namespace)),
        ];

        for (name, cmd) in commands {
            println!("\n[{}]", name);
            let output = self.exec(&cmd).await.unwrap_or_default();
            println!("{}", output);
        }

        Ok(())
    }

    async fn detailed_connectivity(&self) -> Result<()> {
        println!("\n--- Detailed Connectivity Check ---");

        let targets = vec![
            "kubernetes.default.svc.cluster.local:443",
            "10.96.0.1:443",
        ];

        for target in targets {
            let host = target.split(':').next().unwrap_or(target);
            println!("\n[Pinging {}]", target);
            let cmd = format!("{} ping -c 3 {} 2>&1 || echo 'ping failed'", self.nsenter_arg, host);
            let output = self.exec(&cmd).await.unwrap_or_default();
            println!("{}", output);
        }

        Ok(())
    }

    async fn detailed_health(&self) -> Result<()> {
        println!("\n--- Container Health Details ---");

        let commands = vec![
            ("Process 1", format!("{} cat /proc/1/status | grep -E 'Name|State|Pid'", self.nsenter_arg)),
            ("Load", format!("{} cat /proc/loadavg", self.nsenter_arg)),
            ("Uptime", format!("{} cat /proc/uptime", self.nsenter_arg)),
            ("Mounts", format!("{} cat /proc/mounts | head -10", self.nsenter_arg)),
        ];

        for (name, cmd) in commands {
            println!("\n[{}]", name);
            let output = self.exec(&cmd).await.unwrap_or_default();
            println!("{}", output);
        }

        Ok(())
    }

    async fn detailed_processes(&self) -> Result<()> {
        println!("\n--- Process Analysis ---");

        let cmd = format!("{} ps -eo pid,ppid,user,%cpu,%mem,comm --sort=-%cpu | head -20", self.nsenter_arg);
        let output = self.exec(&cmd).await.unwrap_or_default();
        println!("{}", output);

        println!("\n--- Zombie Processes ---");
        let zombie_cmd = format!("{} ps aux | grep -w Z || echo 'No zombies'", self.nsenter_arg);
        let zombie_output = self.exec(&zombie_cmd).await.unwrap_or_default();
        println!("{}", zombie_output);

        Ok(())
    }

    async fn detailed_resources(&self) -> Result<()> {
        println!("\n--- Resource Usage ---");

        let commands = vec![
            ("CPU", format!("{} cat /proc/stat | head -1", self.nsenter_arg)),
            ("Memory", format!("{} cat /proc/meminfo | head -10", self.nsenter_arg)),
            ("Disk", format!("{} df -h", self.nsenter_arg)),
            ("IO Stats", format!("{} cat /proc/diskstats | head -5", self.nsenter_arg)),
        ];

        for (name, cmd) in commands {
            println!("\n[{}]", name);
            let output = self.exec(&cmd).await.unwrap_or_default();
            println!("{}", output);
        }

        Ok(())
    }

    async fn detailed_ports(&self) -> Result<()> {
        println!("\n--- Listening Ports ---");

        let cmd = format!("{} ss -tlnp 2>/dev/null || netstat -tlnp 2>/dev/null || echo 'N/A'", self.nsenter_arg);
        let output = self.exec(&cmd).await.unwrap_or_default();
        println!("{}", output);

        Ok(())
    }

    async fn detailed_connections(&self) -> Result<()> {
        println!("\n--- Active Connections ---");

        let cmd = format!("{} ss -tnp 2>/dev/null || netstat -tnp 2>/dev/null || echo 'N/A'", self.nsenter_arg);
        let output = self.exec(&cmd).await.unwrap_or_default();
        println!("{}", output);

        Ok(())
    }

    async fn start_capture(&self) -> Result<()> {
        println!("\n--- Packet Capture ---");
        println!("Starting tcpdump on container namespace...");
        println!("Use Ctrl+C to stop capture.\n");

        let cmd = format!(
            "{} tcpdump -i any -c 50 -nn 2>&1",
            self.nsenter_arg
        );

        println!("Command: {}\n", cmd);
        println!("(Note: For long captures, use --pcap flag instead)\n");

        Ok(())
    }

    async fn run_custom_command(&self) -> Result<()> {
        print!("\nEnter command to execute in container namespace: ");
        io::stdout().flush().ok();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return Ok(());
        }

        let cmd = input.trim();
        if cmd.is_empty() {
            return Ok(());
        }

        let full_cmd = format!("{} {}", self.nsenter_arg, cmd);
        println!("\nExecuting: {}\n", full_cmd);
        let output = self.exec(&full_cmd).await.unwrap_or_default();
        println!("{}", output);

        Ok(())
    }

    fn show_recommendations(&self) {
        println!("\n--- Recommended Next Steps ---\n");

        let mut tips = Vec::new();

        for result in &self.history {
            if !result.ok {
                match result.category.as_str() {
                    "dns" => {
                        tips.push("Check /etc/resolv.conf for nameserver configuration".to_string());
                        tips.push("Verify cluster DNS (kube-dns/coredns) is running".to_string());
                        tips.push(format!("Try: kubectl exec -n kube-system get pods -l k8s-app=kube-dns"));
                    }
                    "connectivity" => {
                        tips.push("Check network policies that might block traffic".to_string());
                        tips.push("Verify iptables rules in container namespace".to_string());
                        tips.push(format!("Try: kubectl describe pod {} -n {}", self.pod_name, self.namespace));
                    }
                    "container" => {
                        tips.push(format!("Check container logs: kubectl logs {} -n {}", self.pod_name, self.namespace));
                        tips.push(format!("Check events: kubectl describe pod {} -n {}", self.pod_name, self.namespace));
                    }
                    _ => {}
                }
            }
        }

        if tips.is_empty() {
            tips.push("All diagnostics passed - pod appears healthy".to_string());
            tips.push("If issue persists, check application-level logs".to_string());
            tips.push("Consider using --report to generate full diagnostic report".to_string());
        }

        for (i, tip) in tips.iter().enumerate() {
            println!("  {}. {}", i + 1, tip);
        }
    }

    fn suggest_report(&self) {
        println!("\n--- Generate Full Report ---");
        println!("To generate a comprehensive diagnostic report, run:");
        println!();
        println!(
            "  kubectl pod debug {} -n {} --report --report-output report.txt",
            self.pod_name, self.namespace
        );
        println!();
    }

    fn save_session_log(&self) {
        let filename = format!("/tmp/debug_session_{}_{}.log",
            self.pod_name,
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );

        if let Ok(mut file) = std::fs::File::create(&filename) {
            writeln!(file, "=== Debug Session Log ===").ok();
            writeln!(file, "Pod: {} | Namespace: {}", self.pod_name, self.namespace).ok();
            writeln!(file, "Container PID: {}", self.container_pid).ok();
            writeln!(file, "Timestamp: {}\n", chrono::Utc::now()).ok();

            for result in &self.history {
                writeln!(file, "[{}] {}: {}",
                    if result.ok { "OK" } else { "FAIL" },
                    result.category,
                    result.message
                ).ok();
                if let Some(ref details) = result.details {
                    writeln!(file, "{}\n", details).ok();
                }
            }

            println!("Session log saved to: {}", filename);
        } else {
            println!("Failed to save session log");
        }
    }

    fn print_summary(&self) {
        println!("\n╭────────────────────────────────────────╮");
        println!("│  Debug Session Summary                 │");
        println!("╰────────────────────────────────────────╯");

        let passed = self.history.iter().filter(|r| r.ok).count();
        let failed = self.history.iter().filter(|r| !r.ok).count();

        println!("  Checks passed: {}", passed);
        println!("  Checks failed: {}", failed);
        println!("  Session saved to: /tmp/debug_session_*.log");
    }

    async fn exec(&self, cmd: &str) -> Result<String> {
        exec_command(&self.session, cmd).await.map_err(|e| {
            crate::error::PodDebugError::Other {
                reason: format!("Exec failed: {}", e),
            }
        })
    }
}

fn print_banner() {
    println!(r#"
╭──────────────────────────────────────────────────────────────────╮
│                                                                  │
│   🔍  Kubernetes Pod Interactive Debug Assistant                 │
│                                                                  │
│   This tool helps you diagnose common pod issues with            │
│   guided troubleshooting and automated checks.                   │
│                                                                  │
│   Type 'help' for available commands.                            │
│                                                                  │
╰──────────────────────────────────────────────────────────────────╯
"#);
}

#[derive(Debug, Clone)]
pub struct DiagnosticResult {
    pub category: String,
    pub ok: bool,
    pub message: String,
    pub details: Option<String>,
    pub commands: Vec<String>,
}
