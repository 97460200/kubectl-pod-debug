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

fn find_child_in_container_pid_ns(
    session: &Handle<SshClient>,
    shim_pid: u32,
) -> impl std::future::Future<Output = Option<u32>> + '_ {
    async move {
        let cmd = format!("ps --ppid {} -o pid= 2>/dev/null | head -1", shim_pid);
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
        if let Some(child_pid) = find_child_in_container_pid_ns(session, shim_pid).await {
            if child_pid > 1 {
                return Ok(child_pid);
            }
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
