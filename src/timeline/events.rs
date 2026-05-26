use kube::Client;
use k8s_openapi::api::core::v1::Pod;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ContainerRestart {
    pub count: u32,
    pub last_restart: Option<DateTime<Utc>>,
    pub restart_history: Vec<RestartEntry>,
}

#[derive(Debug, Clone)]
pub struct RestartEntry {
    pub timestamp: DateTime<Utc>,
    pub exit_code: i32,
    pub reason: Option<String>,
}

pub struct TimelineCollector {
    kube_client: Client,
}

impl TimelineCollector {
    pub fn new(kube_client: Client) -> Self {
        Self { kube_client }
    }
    
    pub async fn collect_events(&self, pod_name: &str, namespace: &str, _since: ChronoDuration) -> Result<Vec<TimelineEvent>, crate::error::PodDebugError> {
        let api: kube::Api<Pod> = kube::Api::namespaced(self.kube_client.clone(), namespace);
        
        let pod = api.get(pod_name).await
            .map_err(|e| crate::error::PodDebugError::TimelineError {
                reason: format!("Failed to get pod: {}", e),
            })?;
        
        let mut events = Vec::new();
        let creation_ts = pod.metadata.creation_timestamp.as_ref()
            .map(|t| t.0)
            .unwrap_or_else(Utc::now);
        
        if pod.metadata.creation_timestamp.is_some() {
            events.push(TimelineEvent {
                timestamp: creation_ts,
                event_type: "Created".to_string(),
                message: "Pod created".to_string(),
                reason: None,
            });
        }
        
        if let Some(spec) = &pod.spec {
            if let Some(node_name) = &spec.node_name {
                events.push(TimelineEvent {
                    timestamp: creation_ts,
                    event_type: "Scheduled".to_string(),
                    message: format!("Scheduled to node {}", node_name),
                    reason: None,
                });
            }
        }
        
        if let Some(status) = &pod.status {
            if let Some(conditions) = &status.conditions {
                for cond in conditions {
                    if let Some(last_transition) = &cond.last_transition_time {
                        if cond.type_ == "Ready" {
                            if cond.status == "True" {
                                events.push(TimelineEvent {
                                    timestamp: last_transition.0,
                                    event_type: "Ready".to_string(),
                                    message: "Container ready".to_string(),
                                    reason: None,
                                });
                            }
                        }
                    }
                }
            }
            
            if let Some(container_statuses) = &status.container_statuses {
                for cs in container_statuses {
                    if cs.restart_count > 0 {
                        if let Some(last_state) = &cs.last_state {
                            if let Some(terminated) = &last_state.terminated {
                                if let Some(finished_at) = &terminated.finished_at {
                                    events.push(TimelineEvent {
                                        timestamp: finished_at.0,
                                        event_type: "Restarted".to_string(),
                                        message: format!("Container restarted (exit code: {:?})", terminated.exit_code),
                                        reason: terminated.reason.clone(),
                                    });
                                }
                            }
                        }
                    }
                    
                    if let Some(state) = &cs.state {
                        if let Some(running) = &state.running {
                            let started_ts = running.started_at.as_ref()
                                .map(|t| t.0)
                                .unwrap_or_else(Utc::now);
                            events.push(TimelineEvent {
                                timestamp: started_ts,
                                event_type: "Started".to_string(),
                                message: format!("Container {} started", cs.name),
                                reason: None,
                            });
                        }
                        if state.waiting.is_some() {
                            if let Some(reason) = &state.waiting.as_ref().and_then(|w| w.reason.clone()) {
                                events.push(TimelineEvent {
                                    timestamp: Utc::now(),
                                    event_type: "Warning".to_string(),
                                    message: format!("Container {} waiting: {}", cs.name, reason),
                                    reason: Some(reason.clone()),
                                });
                            }
                        }
                    }
                }
            }
        }
        
        events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        
        Ok(events)
    }
    
    pub fn collect_restart_info(&self, pod: &Pod) -> ContainerRestart {
        let mut total_restarts = 0u32;
        let mut last_restart = None::<DateTime<Utc>>;
        
        if let Some(status) = &pod.status {
            if let Some(container_statuses) = &status.container_statuses {
                for cs in container_statuses {
                    total_restarts += cs.restart_count as u32;
                    if let Some(last_state) = &cs.last_state {
                        if let Some(terminated) = &last_state.terminated {
                            if let Some(finished_at) = &terminated.finished_at {
                                if last_restart.is_none() || last_restart.map(|t| finished_at.0 > t).unwrap_or(false) {
                                    last_restart = Some(finished_at.0);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        ContainerRestart {
            count: total_restarts,
            last_restart,
            restart_history: Vec::new(),
        }
    }
}
