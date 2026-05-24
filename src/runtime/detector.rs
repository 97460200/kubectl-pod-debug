use super::RuntimeType;
use crate::error::{PodDebugError, Result};
use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;
use russh::client::Handle;

/// 自动检测节点上的容器运行时
pub async fn detect_runtime(session: &Handle<SshClient>) -> Result<RuntimeType> {
    let output = exec_command(
        session,
        "which crictl 2>/dev/null && echo 'containerd' || (which docker 2>/dev/null && echo 'docker' || echo 'unknown')"
    ).await?;

    match output.trim() {
        "containerd" => Ok(RuntimeType::Containerd),
        "docker" => Ok(RuntimeType::Docker),
        _ => Err(PodDebugError::RuntimeDetectionFailed {
            node: "unknown".to_string(),
        }),
    }
}
