use crate::error::{PodDebugError, Result};
use k8s_openapi::api::core::v1::Node;
use kube::{Api, Client};

/// 获取节点的 InternalIP 地址
pub async fn get_node_ip(client: &Client, node_name: &str) -> Result<String> {
    let nodes: Api<Node> = Api::all(client.clone());
    let node = nodes
        .get(node_name)
        .await
        .map_err(PodDebugError::KubeError)?;

    let addresses = node
        .status
        .as_ref()
        .and_then(|s| s.addresses.as_ref())
        .ok_or_else(|| PodDebugError::SshConnectFailed {
            node: node_name.to_string(),
            reason: "Node has no addresses in status".to_string(),
        })?;

    addresses
        .iter()
        .find(|addr| addr.type_ == "InternalIP")
        .map(|addr| addr.address.clone())
        .ok_or_else(|| PodDebugError::SshConnectFailed {
            node: node_name.to_string(),
            reason: "Node has no InternalIP address".to_string(),
        })
}
