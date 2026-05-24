use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;
use russh::client::Handle;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub source: String,
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

pub async fn auto_discover(
    session: &Handle<SshClient>,
    _container_id: &str,
    host_ip: &str,
    nsenter_arg: &str,
) -> Vec<Target> {
    let mut targets = BTreeSet::new();
    targets.extend(discover_from_env(session, nsenter_arg).await);
    targets.extend(discover_from_connections(session, nsenter_arg).await);
    targets.extend(well_known_targets(host_ip));
    targets.into_iter().collect()
}

async fn discover_from_env(session: &Handle<SshClient>, nsenter_arg: &str) -> Vec<Target> {
    let cmd = format!(
        "{} /bin/bash -c 'env 2>/dev/null | grep \"_SERVICE_HOST=\"'",
        nsenter_arg
    );
    let output = exec_command(session, &cmd).await.unwrap_or_default();

    let mut targets = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let svc_name = k.trim_end_matches("_SERVICE_HOST");
            let host = v.to_string();
            let port_cmd = format!(
                "{} /bin/bash -c 'printenv {}_PORT_ 2>/dev/null | grep -oP \":\\d+\" | head -1 | tr -d \":\"'",
                nsenter_arg, svc_name
            );
            if let Ok(port_out) = exec_command(session, &port_cmd).await {
                if let Ok(port) = port_out.trim().parse::<u16>() {
                    if port > 0 {
                        targets.push(Target {
                            host,
                            port,
                            source: "env".into(),
                        });
                    }
                }
            }
        }
    }

    targets
}

async fn discover_from_connections(session: &Handle<SshClient>, nsenter_arg: &str) -> Vec<Target> {
    let cmd = format!(
        "{} /bin/bash -c 'ss -tnp 2>/dev/null | awk \"/ESTAB/{{print \\$5}}\" | grep -oP \"[0-9.]+:\\d+\" | sort -u'",
        nsenter_arg
    );
    let output = exec_command(session, &cmd).await.unwrap_or_default();

    let mut targets = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((host, port_str)) = line.rsplit_once(':') {
            if let Ok(port) = port_str.trim_start_matches('[').trim_end_matches(']').parse::<u16>() {
                let host = host.trim_start_matches("[::ffff:").trim_end_matches(']');
                if !host.is_empty() && host != "*" {
                    targets.push(Target {
                        host: host.to_string(),
                        port,
                        source: "conn".into(),
                    });
                }
            }
        }
    }
    targets
}

fn well_known_targets(host_ip: &str) -> Vec<Target> {
    vec![
        Target { host: host_ip.into(), port: 6443, source: "kube".into() },
        Target { host: "10.96.0.1".into(), port: 443, source: "kube".into() },
        Target { host: "10.96.0.10".into(), port: 53, source: "kube".into() },
    ]
}

pub fn parse_user_targets(raw: &str) -> Vec<Target> {
    raw.split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            if let Some((host, port_str)) = s.rsplit_once(':') {
                let port: u16 = port_str.parse().ok()?;
                Some(Target {
                    host: host.to_string(),
                    port,
                    source: "user".into(),
                })
            } else {
                None
            }
        })
        .collect()
}
