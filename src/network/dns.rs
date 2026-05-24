use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;
use russh::client::Handle;

#[derive(Debug, Clone)]
pub struct DnsStep {
    pub query_name: String,
    pub result: String,
    pub latency_ms: f64,
    pub ok: bool,
}

#[derive(Debug, Clone)]
pub struct DnsResult {
    pub test_hostname: String,
    pub ndots: u32,
    pub steps: Vec<DnsStep>,
    pub total_ms: f64,
    pub final_ip: Option<String>,
    pub resolved: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvConf {
    pub nameservers: Vec<String>,
    pub search_domains: Vec<String>,
    pub ndots: u32,
}

fn extract_ip(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    for word in raw.split_whitespace() {
        let parts: Vec<&str> = word.split('.').collect();
        if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
            return Some(word.to_string());
        }
    }
    let first = raw.split_whitespace().next().unwrap_or(raw);
    if !first.is_empty() && first != "no-dns-tool" {
        Some(first.to_string())
    } else {
        None
    }
}

pub async fn read_resolv_conf(session: &Handle<SshClient>, pid: u32) -> ResolvConf {
    let cmd = format!("cat /proc/{}/root/etc/resolv.conf 2>/dev/null", pid);
    let output = exec_command(session, &cmd).await.unwrap_or_default();

    let mut nameservers = Vec::new();
    let mut search_domains = Vec::new();
    let mut ndots = 5u32;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            "nameserver" if parts.len() > 1 => {
                nameservers.push(parts[1].to_string());
            }
            "search" => {
                search_domains = parts[1..].iter().map(|s| s.to_string()).collect();
            }
            "options" => {
                for opt in &parts[1..] {
                    if let Some(val) = opt.strip_prefix("ndots:") {
                        ndots = val.parse().unwrap_or(5);
                    }
                }
            }
            _ => {}
        }
    }

    ResolvConf {
        nameservers,
        search_domains,
        ndots,
    }
}

pub async fn resolve_dns_chain(
    session: &Handle<SshClient>,
    nsenter_arg: &str,
    hostname: &str,
    resolv: &ResolvConf,
) -> DnsResult {
    let ns = resolv.nameservers.first().cloned().unwrap_or_else(|| "10.96.0.10".into());
    let dot_count = hostname.chars().filter(|c| *c == '.').count() as u32;
    let suffix_search = dot_count < resolv.ndots;
    let mut steps = Vec::new();
    let mut resolved = false;
    let mut final_ip = None;
    let mut total_ms = 0.0f64;

    if suffix_search && !resolv.search_domains.is_empty() {
        for domain in &resolv.search_domains {
            let query_name = format!("{}.{}.", hostname, domain);
            let step = dig_one(session, nsenter_arg, &query_name, &ns).await;
            let ok = step.ok;
            total_ms += step.latency_ms;
            steps.push(step);
            if ok {
                resolved = true;
                break;
            }
        }
    }

    if !resolved {
        let query_name = if hostname.ends_with('.') {
            hostname.to_string()
        } else {
            format!("{}.", hostname)
        };
        let step = dig_one(session, nsenter_arg, &query_name, &ns).await;
        total_ms += step.latency_ms;
        if step.ok {
            final_ip = extract_ip(&step.result);
            resolved = true;
        }
        steps.push(step);
    } else if let Some(last) = steps.last() {
        final_ip = extract_ip(&last.result);
    }

    DnsResult {
        test_hostname: hostname.to_string(),
        ndots: resolv.ndots,
        steps,
        total_ms,
        final_ip,
        resolved,
    }
}

async fn dig_one(
    session: &Handle<SshClient>,
    nsenter_arg: &str,
    query: &str,
    ns: &str,
) -> DnsStep {
    let cmd = format!(
        "{} /bin/bash -c ' \
         start=$(date +%s%N); \
         if command -v dig >/dev/null 2>&1; then \
           result=$(dig +short +time=3 +tries=1 {} @{} 2>&1 | head -1); \
         elif command -v nslookup >/dev/null 2>&1; then \
           result=$(nslookup -timeout=3 {} {} 2>&1 | grep -A1 Name: | tail -1 | awk \"{{print \\$NF}}\"); \
         elif command -v getent >/dev/null 2>&1; then \
           result=$(getent hosts {} 2>&1 | head -1 | awk \"{{print \\$1}}\"); \
         else \
           result=\"no-dns-tool\"; \
         fi; \
         end=$(date +%s%N); \
         echo RESULT=$result; \
         echo ELAPSED=$(( (end-start)/1000000 ))'",
        nsenter_arg, query, ns, query, ns, query
    );

    let output = exec_command(session, &cmd).await.unwrap_or_default();

    let mut result = String::new();
    let mut latency = 0.0f64;

    for line in output.lines() {
        if let Some(val) = line.strip_prefix("RESULT=") {
            result.push_str(val);
        }
        if let Some(val) = line.strip_prefix("ELAPSED=") {
            latency = val.trim().parse().unwrap_or(0.0);
        }
    }

    let result = result.trim().to_string();
    let ok = !result.is_empty()
        && !result.contains("SERVFAIL")
        && !result.contains("NXDOMAIN")
        && !result.contains(";; connection timed out");

    DnsStep {
        query_name: query.to_string(),
        result,
        latency_ms: latency,
        ok,
    }
}
