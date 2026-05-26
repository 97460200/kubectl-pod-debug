use super::collector::{PodConfig, RsConfig};

#[derive(Debug)]
pub enum DiffLevel {
    Critical,
    Warning,
    Match,
}

#[derive(Debug)]
pub struct ConfigDiff {
    pub field: String,
    pub pod_value: String,
    pub rs_value: String,
    pub level: DiffLevel,
    pub impact: String,
}

pub struct ConfigComparator;

impl ConfigComparator {
    pub fn compare(pod_config: &PodConfig, rs_config: &RsConfig) -> Vec<ConfigDiff> {
        let mut diffs = Vec::new();
        
        if let (Some(pod_img), Some(rs_img)) = (&pod_config.image, &rs_config.image) {
            if pod_img != rs_img {
                diffs.push(ConfigDiff {
                    field: "Image".to_string(),
                    pod_value: pod_img.clone(),
                    rs_value: rs_img.clone(),
                    level: DiffLevel::Critical,
                    impact: "可能运行旧版本镜像".to_string(),
                });
            }
        }
        
        if let (Some(pod_cpu), Some(rs_cpu)) = (&pod_config.resources_limits_cpu, &rs_config.resources_limits_cpu) {
            if pod_cpu != rs_cpu {
                diffs.push(ConfigDiff {
                    field: "CPU Limit".to_string(),
                    pod_value: pod_cpu.clone(),
                    rs_value: rs_cpu.clone(),
                    level: DiffLevel::Warning,
                    impact: "资源限制与期望不符，可能导致性能问题".to_string(),
                });
            }
        }
        
        if let (Some(pod_mem), Some(rs_mem)) = (&pod_config.resources_limits_memory, &rs_config.resources_limits_memory) {
            if pod_mem != rs_mem {
                diffs.push(ConfigDiff {
                    field: "Memory Limit".to_string(),
                    pod_value: pod_mem.clone(),
                    rs_value: rs_mem.clone(),
                    level: DiffLevel::Warning,
                    impact: "内存限制与期望不符，可能导致 OOM".to_string(),
                });
            }
        }
        
        if let (Some(pod_cpu), Some(rs_cpu)) = (&pod_config.resources_requests_cpu, &rs_config.resources_requests_cpu) {
            if pod_cpu != rs_cpu {
                diffs.push(ConfigDiff {
                    field: "CPU Request".to_string(),
                    pod_value: pod_cpu.clone(),
                    rs_value: rs_cpu.clone(),
                    level: DiffLevel::Warning,
                    impact: "CPU 请求与期望不符".to_string(),
                });
            }
        }
        
        if let (Some(pod_mem), Some(rs_mem)) = (&pod_config.resources_requests_memory, &rs_config.resources_requests_memory) {
            if pod_mem != rs_mem {
                diffs.push(ConfigDiff {
                    field: "Memory Request".to_string(),
                    pod_value: pod_mem.clone(),
                    rs_value: rs_mem.clone(),
                    level: DiffLevel::Warning,
                    impact: "内存请求与期望不符".to_string(),
                });
            }
        }
        
        for (key, pod_val) in &pod_config.env_vars {
            if let Some(rs_val) = rs_config.env_vars.iter().find(|(k, _)| k == key) {
                if &rs_val.1 != pod_val {
                    diffs.push(ConfigDiff {
                        field: format!("ENV: {}", key),
                        pod_value: pod_val.clone(),
                        rs_value: rs_val.1.clone(),
                        level: DiffLevel::Warning,
                        impact: "环境变量与期望不符".to_string(),
                    });
                }
            }
        }
        
        diffs
    }
    
    pub fn format_diffs(diffs: &[ConfigDiff], pod_name: &str, namespace: &str, rs_name: &str) -> String {
        let mut output = format!("=== Configuration Diff ===\n");
        output.push_str(&format!("Namespace: {}\n", namespace));
        output.push_str(&format!("Pod: {}\n", pod_name));
        output.push_str(&format!("ReplicaSet: {}\n\n", rs_name));
        
        let has_diffs = diffs.iter().any(|d| !matches!(d.level, DiffLevel::Match));
        
        if has_diffs {
            for diff in diffs {
                let icon = match diff.level {
                    DiffLevel::Critical => "🔴",
                    DiffLevel::Warning => "⚠️",
                    DiffLevel::Match => "✅",
                };
                output.push_str(&format!("{} {} Mismatch\n", icon, diff.field));
                output.push_str(&format!("   Pod:     {}\n", diff.pod_value));
                output.push_str(&format!("   RS:      {}\n", diff.rs_value));
                output.push_str(&format!("   Impact:  {}\n\n", diff.impact));
            }
        } else {
            output.push_str("✅ All settings match\n");
        }
        
        output
    }
}
