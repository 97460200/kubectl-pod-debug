use kube::Client;
use k8s_openapi::api::apps::v1::ReplicaSet;
use k8s_openapi::api::core::v1::Pod;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PodConfig {
    pub image: Option<String>,
    pub resources_limits_cpu: Option<String>,
    pub resources_limits_memory: Option<String>,
    pub resources_requests_cpu: Option<String>,
    pub resources_requests_memory: Option<String>,
    pub env_vars: Vec<(String, String)>,
    pub volumes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RsConfig {
    pub name: String,
    pub replicas: i32,
    pub image: Option<String>,
    pub resources_limits_cpu: Option<String>,
    pub resources_limits_memory: Option<String>,
    pub resources_requests_cpu: Option<String>,
    pub resources_requests_memory: Option<String>,
    pub env_vars: Vec<(String, String)>,
    pub volumes: Vec<String>,
}

pub struct ConfigCollector {
    kube_client: Client,
}

impl ConfigCollector {
    pub fn new(kube_client: Client) -> Self {
        Self { kube_client }
    }
    
    pub async fn collect_pod_config(&self, pod: &Pod) -> PodConfig {
        let mut config = PodConfig::default();
        
        if let Some(spec) = &pod.spec {
            if let Some(containers) = spec.containers.first() {
                config.image = containers.image.clone();
                
                if let Some(resources) = &containers.resources {
                    if let Some(limits) = &resources.limits {
                        config.resources_limits_cpu = limits.get("cpu").map(|v| v.0.clone());
                        config.resources_limits_memory = limits.get("memory").map(|v| v.0.clone());
                    }
                    if let Some(requests) = &resources.requests {
                        config.resources_requests_cpu = requests.get("cpu").map(|v| v.0.clone());
                        config.resources_requests_memory = requests.get("memory").map(|v| v.0.clone());
                    }
                }
                
                if let Some(env) = &containers.env {
                    config.env_vars = env.iter()
                        .filter_map(|e| e.value.clone().map(|v| (e.name.clone(), v)))
                        .collect();
                }
            }
        }
        
        config
    }
    
    pub async fn find_replicaset(&self, pod: &Pod) -> Result<Option<(String, ReplicaSet)>, crate::error::PodDebugError> {
        let namespace = pod.metadata.namespace.as_ref()
            .ok_or_else(|| crate::error::PodDebugError::DiffError { 
                reason: "Pod has no namespace".to_string() 
            })?;
        
        if let Some(owner_refs) = &pod.metadata.owner_references {
            for owner in owner_refs {
                if owner.kind == "ReplicaSet" {
                    let api: kube::Api<ReplicaSet> = kube::Api::namespaced(self.kube_client.clone(), namespace);
                    if let Ok(rs) = api.get(&owner.name).await {
                        return Ok(Some((owner.name.clone(), rs)));
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    pub fn collect_rs_config(&self, rs: &ReplicaSet) -> RsConfig {
        let mut config = RsConfig::default();
        config.name = rs.metadata.name.clone().unwrap_or_default();
        config.replicas = rs.spec.as_ref().and_then(|s| s.replicas).unwrap_or(0);
        
        if let Some(spec) = &rs.spec {
            if let Some(template) = &spec.template {
                if let Some(containers) = template.spec.as_ref().and_then(|s| s.containers.first()) {
                    config.image = containers.image.clone();
                    
                    if let Some(resources) = &containers.resources {
                        if let Some(limits) = &resources.limits {
                            config.resources_limits_cpu = limits.get("cpu").map(|v| v.0.clone());
                            config.resources_limits_memory = limits.get("memory").map(|v| v.0.clone());
                        }
                        if let Some(requests) = &resources.requests {
                            config.resources_requests_cpu = requests.get("cpu").map(|v| v.0.clone());
                            config.resources_requests_memory = requests.get("memory").map(|v| v.0.clone());
                        }
                    }
                    
                    if let Some(env) = &containers.env {
                        config.env_vars = env.iter()
                            .filter_map(|e| e.value.clone().map(|v| (e.name.clone(), v)))
                            .collect();
                    }
                }
            }
        }
        
        config
    }
}
