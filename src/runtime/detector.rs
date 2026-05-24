use super::RuntimeType;
use crate::error::{PodDebugError, Result};
use crate::ssh::connect::SshClient;
use crate::ssh::exec::exec_command;
use russh::client::Handle;

pub async fn detect_runtime(session: &Handle<SshClient>, node: &str) -> Result<RuntimeType> {
    let output = exec_command(
        session,
        "if [ -S /run/containerd/containerd.sock ] || [ -S /var/run/containerd/containerd.sock ] || [ -S /run/k3s/containerd/containerd.sock ] || [ -S /var/snap/microk8s/common/run/containerd.sock ]; then echo containerd; elif [ -S /var/run/docker.sock ]; then echo docker; elif command -v crictl >/dev/null 2>&1; then echo containerd; elif command -v docker >/dev/null 2>&1; then echo docker; else echo unknown; fi",
    )
    .await?;

    match output.trim() {
        "containerd" => Ok(RuntimeType::Containerd),
        "docker" => Ok(RuntimeType::Docker),
        _ => Err(PodDebugError::RuntimeDetectionFailed {
            node: node.to_string(),
        }),
    }
}
