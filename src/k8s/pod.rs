use crate::error::{PodDebugError, Result};
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client};

/// 获取 Pod 信息
pub async fn get_pod(client: &Client, name: &str, namespace: &str) -> Result<Pod> {
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    pods.get(name)
        .await
        .map_err(|e| match e {
            kube::Error::Api(api_err) if api_err.code == 404 => PodDebugError::PodNotFound {
                name: name.to_string(),
                namespace: namespace.to_string(),
            },
            other => PodDebugError::KubeError(other),
        })
}

/// 获取 Pod 运行的节点名称
pub fn get_node_name(pod: &Pod) -> Result<String> {
    pod.spec
        .as_ref()
        .and_then(|s| s.node_name.clone())
        .ok_or_else(|| PodDebugError::NsenterFailed {
            reason: format!(
                "Pod '{}' has no node assigned",
                pod.metadata.name.as_deref().unwrap_or("?")
            ),
        })
}

/// 获取容器 ID（去掉运行时前缀，如 "containerd://" -> 纯 ID）
pub fn get_container_id(pod: &Pod, container_name: &str) -> Result<String> {
    let statuses = pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .ok_or_else(|| PodDebugError::ContainerNotFound {
            container: container_name.to_string(),
            pod: pod.metadata.name.as_deref().unwrap_or("?").to_string(),
        })?;

    let status = statuses
        .iter()
        .find(|cs| cs.name == container_name)
        .ok_or_else(|| PodDebugError::ContainerNotFound {
            container: container_name.to_string(),
            pod: pod.metadata.name.as_deref().unwrap_or("?").to_string(),
        })?;

    // 检查容器是否在运行
    let _state = status
        .state
        .as_ref()
        .and_then(|s| s.running.as_ref())
        .ok_or_else(|| PodDebugError::ContainerNotRunning {
            container: container_name.to_string(),
            state: "not running".to_string(),
        })?;

    let container_id = status
        .container_id
        .as_ref()
        .ok_or_else(|| PodDebugError::PidLookupFailed {
            reason: format!("Container '{}' has no containerID", container_name),
        })?;

    // 去掉运行时前缀（"containerd://xxxxx" -> "xxxxx"）
    let id = container_id
        .split_once("://")
        .map(|x| x.1)
        .unwrap_or(container_id)
        .to_string();

    Ok(id)
}

/// 获取 Pod 中第一个容器的名称
pub fn get_first_container_name(pod: &Pod) -> Option<String> {
    Some(pod.spec.as_ref()?.containers.first()?.name.clone())
}
