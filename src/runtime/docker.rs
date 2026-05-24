use crate::error::{PodDebugError, Result};
use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;
use russh::client::Handle;

/// 通过 docker inspect 获取容器 PID
pub async fn get_container_pid(session: &Handle<SshClient>, container_id: &str) -> Result<u32> {
    let cmd = format!(
        "docker inspect --format '{{{{.State.Pid}}}}' {} 2>/dev/null",
        container_id
    );

    let output = exec_command(session, &cmd).await?;
    let pid_str = output.trim();

    pid_str.parse::<u32>().map_err(|_| PodDebugError::PidLookupFailed {
        reason: format!("Failed to parse PID from docker inspect output: '{}'", pid_str),
    })
}
