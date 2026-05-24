use crate::error::{PodDebugError, Result};
use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;
use russh::client::Handle;

fn extract_json(output: &str) -> Option<&str> {
    let start = output.find('{')?;
    let end = output.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&output[start..=end])
}

fn collect_pids(value: &serde_json::Value, out: &mut Vec<u32>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if k.eq_ignore_ascii_case("pid") {
                    if let Some(pid) = v.as_u64().and_then(|x| u32::try_from(x).ok()) {
                        out.push(pid);
                    }
                }
                collect_pids(v, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_pids(v, out);
            }
        }
        _ => {}
    }
}

fn parse_pid(output: &str) -> Option<u32> {
    let json = extract_json(output).unwrap_or(output);
    let v = serde_json::from_str::<serde_json::Value>(json).ok()?;
    if let Some(pid) = v
        .pointer("/status/pid")
        .and_then(|x| x.as_u64())
        .and_then(|x| u32::try_from(x).ok())
    {
        return Some(pid);
    }
    let mut pids = Vec::new();
    collect_pids(&v, &mut pids);
    pids.iter().copied().filter(|p| *p > 2).max().or_else(|| pids.into_iter().max())
}

/// 列出与 init_pid 同 PID namespace 的所有进程，返回 (宿主机PID, 命令行)
pub fn list_container_processes(
    session: &Handle<SshClient>,
    init_pid: u32,
) -> impl std::future::Future<Output = Vec<(u32, String)>> + '_ {
    async move {
        let cmd = format!(
            "init_ns=$(readlink /proc/{0}/ns/pid 2>/dev/null); \
             for p in $(ls /proc 2>/dev/null | grep '^[0-9]*$' | sort -n); do \
               ns=$(readlink /proc/$p/ns/pid 2>/dev/null) || continue; \
               if [ \"$ns\" = \"$init_ns\" ]; then \
                 cmdline=$(cat /proc/$p/cmdline 2>/dev/null | tr '\\0' ' ' | head -c 120); \
                 echo \"PID:$p CMD:$cmdline\"; \
               fi; \
             done",
            init_pid
        );
        let output = exec_command(session, &cmd).await.unwrap_or_default();
        output
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                let pid_part = parts.first()?;
                let pid_str = pid_part.strip_prefix("PID:")?;
                let host_pid: u32 = pid_str.parse().ok()?;
                let cmd_part = parts.get(1).unwrap_or(&"");
                let cmdline = cmd_part.strip_prefix("CMD:").unwrap_or(cmd_part).to_string();
                Some((host_pid, cmdline))
            })
            .collect()
    }
}

fn find_container_init(
    session: &Handle<SshClient>,
    shim_pid: u32,
) -> impl std::future::Future<Output = Option<u32>> + '_ {
    async move {
        let cmd = format!(
            "host_ns=$(readlink /proc/1/ns/pid 2>/dev/null); \
             queue=$(cat /proc/{0}/task/{0}/children 2>/dev/null); \
             for p in $queue; do \
               if [ \"$(readlink /proc/$p/ns/pid 2>/dev/null)\" != \"$host_ns\" ]; then \
                 echo $p; exit 0; \
               fi; \
             done; \
             for p in $queue; do \
               for c in $(cat /proc/$p/task/$p/children 2>/dev/null); do \
                 if [ \"$(readlink /proc/$c/ns/pid 2>/dev/null)\" != \"$host_ns\" ]; then \
                   echo $c; exit 0; \
                 fi; \
               done; \
             done",
            shim_pid
        );
        exec_command(session, &cmd).await.ok().and_then(|s| s.trim().parse().ok())
    }
}

pub async fn get_container_pid(session: &Handle<SshClient>, container_id: &str) -> Result<u32> {
    let mut reasons = Vec::new();

    let crictl_cmd = format!("crictl inspect {} 2>/dev/null", container_id);
    let raw_pid = match exec_command(session, &crictl_cmd).await {
        Ok(output) => match parse_pid(&output) {
            Some(pid) => Some(pid),
            None => {
                reasons.push(format!(
                    "crictl inspect returned no pid (first 200 chars): {}",
                    output.chars().take(200).collect::<String>()
                ));
                None
            }
        },
        Err(e) => {
            reasons.push(format!("crictl inspect failed: {}", e));
            None
        }
    };

    if let Some(shim_pid) = raw_pid {
        if let Some(init_pid) = find_container_init(session, shim_pid).await {
            return Ok(init_pid);
        }
        return Ok(shim_pid);
    }

    let ctr_cmd = format!("ctr -n k8s.io tasks info {} 2>/dev/null", container_id);
    match exec_command(session, &ctr_cmd).await {
        Ok(output) => {
            if let Some(pid) = parse_pid(&output) {
                return Ok(pid);
            }
            reasons.push(format!(
                "ctr tasks info returned no pid (first 200 chars): {}",
                output.chars().take(200).collect::<String>()
            ));
        }
        Err(e) => reasons.push(format!("ctr tasks info failed: {}", e)),
    }

    Err(PodDebugError::PidLookupFailed {
        reason: format!(
            "Unable to determine container PID for '{}': {}",
            container_id,
            reasons.join(" | ")
        ),
    })
}
