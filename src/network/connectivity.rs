use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;
use crate::network::targets::Target;
use russh::client::Handle;

#[derive(Debug, Clone)]
pub struct ConnectivityResult {
    pub target: Target,
    pub ok: bool,
    pub latency_ms: f64,
    pub error: String,
}

impl std::fmt::Display for ConnectivityResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.ok { "✅ OK" } else { "❌ FAIL" };
        write!(
            f,
            "{:<30} TCP  {:<8} {:<8.1}ms",
            self.target.to_string(),
            status,
            self.latency_ms
        )?;
        if !self.ok {
            write!(f, "  {}", self.error)?;
        }
        Ok(())
    }
}

pub async fn check_connectivity(
    session: &Handle<SshClient>,
    nsenter_arg: &str,
    targets: &[Target],
    timeout_secs: u8,
) -> Vec<ConnectivityResult> {
    let mut results = Vec::new();

    for target in targets {
        let cmd = format!(
            "{} /bin/bash -c 'start=$(date +%s%N); timeout {} bash -c \"echo >/dev/tcp/{}/{}\" 2>&1; rc=$?; end=$(date +%s%N); echo \"RC=$rc ELAPSED=$(( (end-start)/1000000 ))\"'",
            nsenter_arg, timeout_secs, target.host, target.port
        );

        match exec_command(session, &cmd).await {
            Ok(output) => {
                let mut ok = false;
                let mut latency = 0.0f64;
                let mut error = String::new();

                for line in output.lines() {
                    if line.starts_with("RC=0") {
                        ok = true;
                    }
                    if let Some(val) = line.strip_prefix("RC=") {
                        if val.starts_with('1') || val.starts_with('2') {
                            ok = false;
                        }
                    }
                    if let Some(val) = line.strip_prefix("ELAPSED=") {
                        latency = val.trim().parse().unwrap_or(0.0);
                    }
                    if !ok && line.starts_with("RC=") && !line.starts_with("RC=0") {
                        if let Some(rest) = line[3..].split_whitespace().next() {
                            error = rest.to_string();
                        }
                    }
                }

                if !ok && error.is_empty() {
                    error = output.trim().to_string();
                }

                results.push(ConnectivityResult {
                    target: target.clone(),
                    ok,
                    latency_ms: latency,
                    error,
                });
            }
            Err(e) => {
                results.push(ConnectivityResult {
                    target: target.clone(),
                    ok: false,
                    latency_ms: 0.0,
                    error: format!("exec failed: {}", e),
                });
            }
        }
    }

    results
}
