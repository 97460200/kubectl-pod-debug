use thiserror::Error;

#[derive(Debug, Error)]
pub enum PodDebugError {
    #[error("Pod '{name}' not found in namespace '{namespace}'")]
    PodNotFound { name: String, namespace: String },

    #[error("Container '{container}' not found in pod '{pod}'")]
    ContainerNotFound { container: String, pod: String },

    #[error("Container '{container}' is not running (state: {state})")]
    ContainerNotRunning { container: String, state: String },

    #[error("Failed to connect to node '{node}' via SSH: {reason}")]
    SshConnectFailed { node: String, reason: String },

    #[error("SSH authentication failed for user '{user}': {reason}")]
    SshAuthFailed { user: String, reason: String },

    #[error("Failed to detect container runtime on node '{node}'")]
    RuntimeDetectionFailed { node: String },

    #[error("Failed to get container PID: {reason}")]
    PidLookupFailed { reason: String },

    #[error("nsenter execution failed: {reason}")]
    NsenterFailed { reason: String },

    #[error("Kubernetes API error: {0}")]
    KubeError(#[from] kube::Error),

    #[error("Kubernetes config error: {0}")]
    KubeConfigError(#[from] kube::config::KubeconfigError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PodDebugError>;
