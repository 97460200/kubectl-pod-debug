use std::path::PathBuf;
use std::time::Duration;

use crate::error::Result;
use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;
use russh::client::Handle;
use tokio::time::sleep;
use tracing::info;

pub struct PcapCapture {
    session: Handle<SshClient>,
    container_pid: u32,
    nsenter_arg: String,
    filter: String,
    count: usize,
    output_path: PathBuf,
}

impl PcapCapture {
    pub fn new(
        session: Handle<SshClient>,
        container_pid: u32,
        nsenter_arg: String,
        filter: String,
        count: usize,
        output_path: PathBuf,
    ) -> Self {
        Self {
            session,
            container_pid,
            nsenter_arg,
            filter,
            count,
            output_path,
        }
    }

    pub async fn capture(&mut self) -> Result<PcapResult> {
        println!("\n🔍 Starting network packet capture...");
        println!("  Container PID: {}", self.container_pid);
        println!("  Capture count: {}", self.count);
        if !self.filter.is_empty() {
            println!("  Filter: {}", self.filter);
        }
        println!("  Output: {}\n", self.output_path.display());

        self.check_tcpdump().await?;

        let remote_path = format!("/tmp/pod_capture_{}.pcap", std::process::id());

        let mut tcpdump_cmd = format!(
            "{} tcpdump -i any -c {} -w {}",
            self.nsenter_arg, self.count, remote_path
        );

        if !self.filter.is_empty() {
            tcpdump_cmd.push_str(&format!(" -f '{}'", self.filter));
        }

        let bash_script = format!(
            "{} &\nTCPDUMP_PID=$!\nsleep 30\nkill $TCPDUMP_PID 2>/dev/null || true",
            tcpdump_cmd
        );

        info!("Starting tcpdump via bash");

        let output = exec_command(&self.session, &format!("/bin/bash -c '{}'", bash_script)).await?;

        println!("\n⏳ Waiting for capture to complete...");
        sleep(Duration::from_secs(3)).await;

        info!("Downloading pcap file from {}", remote_path);

        let scp_result = self.download_pcap(&remote_path).await;

        let cleanup_cmd = format!("rm -f {}", remote_path);
        let _ = exec_command(&self.session, &cleanup_cmd).await;

        if let Err(e) = scp_result {
            println!("⚠️  Failed to download pcap: {}", e);
            return Ok(PcapResult {
                output_path: None,
                packet_count: 0,
                error: Some(format!("Download failed: {}", e)),
                analysis: None,
            });
        }

        let packet_count = self.count_packets().await.unwrap_or(0);
        let analysis = self.analyze_pcap().await.ok();

        println!("\n✅ Packet capture completed!");
        println!("   Packets captured: {}", packet_count);

        Ok(PcapResult {
            output_path: Some(self.output_path.clone()),
            packet_count,
            error: None,
            analysis,
        })
    }

    async fn check_tcpdump(&self) -> Result<()> {
        let check_cmd = format!("{} which tcpdump", self.nsenter_arg);
        let output = exec_command(&self.session, &check_cmd).await?;

        if output.trim().is_empty() {
            println!("⚠️  tcpdump not found, checking host...");
        }

        Ok(())
    }

    async fn download_pcap(&self, remote_path: &str) -> Result<()> {
        let local_path = self.output_path.to_string_lossy();

        let check_exists = format!("{} test -f {} && echo 'EXISTS' || echo 'NOT_FOUND'", self.nsenter_arg, remote_path);
        let exists = exec_command(&self.session, &check_exists).await?;

        if !exists.contains("EXISTS") {
            return Err(crate::error::PodDebugError::Other {
                reason: "pcap file not found on remote".to_string(),
            });
        }

        let check_size = format!("{} stat -c %s {} 2>/dev/null || echo '0'", self.nsenter_arg, remote_path);
        let size_str = exec_command(&self.session, &check_size).await?;
        if let Ok(size) = size_str.trim().parse::<u64>() {
            println!("   Remote pcap size: {} bytes", size);
            if size == 0 {
                return Err(crate::error::PodDebugError::Other {
                    reason: "pcap file is empty".to_string(),
                });
            }
        }

        let encoded = exec_command(&self.session, &format!("{} cat {} 2>/dev/null | base64", self.nsenter_arg, remote_path)).await?;

        if encoded.len() > 100 {
            let decoded = base64_decode(encoded.trim());
            if !decoded.is_empty() {
                std::fs::write(&self.output_path, &decoded).map_err(|e| {
                    crate::error::PodDebugError::Other {
                        reason: format!("Failed to write pcap: {}", e),
                    }
                })?;
                info!("Pcap downloaded via base64 encoding");
                return Ok(());
            }
        }

        Err(crate::error::PodDebugError::Other {
            reason: "Failed to download pcap file".to_string(),
        })
    }

    async fn count_packets(&self) -> Result<usize> {
        if !self.output_path.exists() {
            return Ok(0);
        }

        let output = tokio::process::Command::new("tcpdump")
            .args(["-r", self.output_path.to_str().unwrap(), "-c"])
            .output()
            .await
            .map_err(|e| crate::error::PodDebugError::Other {
                reason: format!("Failed to read pcap: {}", e),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let count = stdout
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        Ok(count)
    }

    async fn analyze_pcap(&self) -> Result<PcapAnalysis> {
        if !self.output_path.exists() {
            return Ok(PcapAnalysis::default());
        }

        let output = tokio::process::Command::new("tcpdump")
            .args([
                "-r",
                self.output_path.to_str().unwrap(),
                "-nn",
                "-q",
            ])
            .output()
            .await
            .map_err(|e| crate::error::PodDebugError::Other {
                reason: format!("Failed to analyze pcap: {}", e),
            })?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = output_str.lines().take(100).collect();

        let mut tcp_count = 0;
        let mut udp_count = 0;
        let mut icmp_count = 0;
        let mut http_requests = Vec::new();
        let mut connections: std::collections::HashSet<String> = std::collections::HashSet::new();

        for line in &lines {
            let lower = line.to_lowercase();
            if lower.contains("tcp") {
                tcp_count += 1;
            } else if lower.contains("udp") {
                udp_count += 1;
            } else if lower.contains("icmp") {
                icmp_count += 1;
            }

            if lower.contains("http") || lower.contains("get ") || lower.contains("post ") {
                if let Some(http) = extract_http_info(line) {
                    http_requests.push(http);
                }
            }

            if let Some(conn) = extract_connection(line) {
                connections.insert(conn);
            }
        }

        Ok(PcapAnalysis {
            protocol_counts: ProtocolCounts {
                tcp: tcp_count,
                udp: udp_count,
                icmp: icmp_count,
            },
            http_requests,
            unique_connections: connections.len(),
        })
    }
}

fn base64_decode(input: &str) -> Vec<u8> {
    let input = input.trim();
    let chars: Vec<char> = input.chars().filter(|c| !c.is_whitespace()).collect();
    let input = chars.iter().collect::<String>();

    let decoded = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &input,
    );

    decoded.unwrap_or_default()
}

fn extract_http_info(line: &str) -> Option<HttpRequest> {
    let lower = line.to_lowercase();
    if lower.contains("http") || lower.contains("get ") || lower.contains("post ") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let method = if lower.contains("get") { "GET" } else if lower.contains("post") { "POST" } else { "HTTP" };
            let path = parts.get(1).unwrap_or(&"").to_string();
            return Some(HttpRequest {
                method: method.to_string(),
                path,
                timestamp: line.chars().take(20).collect(),
            });
        }
    }
    None
}

fn extract_connection(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        if part.contains(">") || part.contains(".") {
            let src = parts.get(i.saturating_sub(1)).unwrap_or(&"");
            let dst = *part;
            if src.contains('.') && (dst.contains('.') || dst.contains(">")) {
                return Some(format!("{} -> {}", src, dst));
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct PcapResult {
    pub output_path: Option<PathBuf>,
    pub packet_count: usize,
    pub error: Option<String>,
    pub analysis: Option<PcapAnalysis>,
}

#[derive(Debug, Clone, Default)]
pub struct PcapAnalysis {
    pub protocol_counts: ProtocolCounts,
    pub http_requests: Vec<HttpRequest>,
    pub unique_connections: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ProtocolCounts {
    pub tcp: usize,
    pub udp: usize,
    pub icmp: usize,
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub timestamp: String,
}

pub fn format_pcap_result(result: &PcapResult) -> String {
    let mut output = String::new();

    if let Some(ref path) = result.output_path {
        output.push_str(&format!("\n📁 PCAP file saved: {}\n", path.display()));
    }

    if let Some(ref err) = result.error {
        output.push_str(&format!("❌ Error: {}\n", err));
        return output;
    }

    output.push_str(&format!("📊 Packets captured: {}\n", result.packet_count));

    if let Some(ref analysis) = result.analysis {
        output.push_str("\n🌐 Protocol Distribution:\n");
        output.push_str(&format!("   TCP:  {} packets\n", analysis.protocol_counts.tcp));
        output.push_str(&format!("   UDP:  {} packets\n", analysis.protocol_counts.udp));
        output.push_str(&format!("   ICMP: {} packets\n", analysis.protocol_counts.icmp));
        output.push_str(&format!("\n🔗 Unique connections: {}\n", analysis.unique_connections));

        if !analysis.http_requests.is_empty() {
            output.push_str("\n📋 HTTP Requests:\n");
            for req in analysis.http_requests.iter().take(10) {
                output.push_str(&format!(
                    "   {} {}\n",
                    req.method, req.path
                ));
            }
        }
    }

    if let Some(ref path) = result.output_path {
        output.push_str(&format!(
            "\n💡 View in Wireshark: wireshark {} &\n",
            path.display()
        ));
        output.push_str(&format!(
            "   Or analyze: tcpdump -r {} -nn\n",
            path.display()
        ));
    }

    output
}
