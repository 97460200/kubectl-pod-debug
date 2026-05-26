use chrono::Local;
use super::events::{TimelineEvent, ContainerRestart};

pub struct TimelineFormatter;

impl TimelineFormatter {
    pub fn format_events(pod_name: &str, namespace: &str, events: &[TimelineEvent]) -> String {
        let mut output = format!("=== Pod Timeline ({}/{})\n\n", pod_name, namespace);
        
        for event in events {
            let ts = event.timestamp.format("%Y-%m-%d %H:%M:%S");
            let icon = match event.event_type.as_str() {
                "Created" => "✅",
                "Scheduled" => "📍",
                "Pulling" => "🐳",
                "Started" => "🚀",
                "Ready" => "✅",
                "Warning" | "Failed" => "⚠️",
                "Restarted" => "🔄",
                _ => "📌",
            };
            output.push_str(&format!("{}  {}  {}\n", ts, icon, event.message));
        }
        
        output
    }
    
    pub fn format_restarts(restarts: &ContainerRestart) -> String {
        let mut output = String::from("\n=== Container Restarts ===\n\n");
        output.push_str(&format!("Total restarts: {}\n", restarts.count));
        
        if let Some(last) = restarts.last_restart {
            let ago = Local::now().signed_duration_since(last);
            output.push_str(&format!("Last restart: {} ({} ago)\n", 
                last.format("%Y-%m-%d %H:%M:%S"), 
                Self::format_duration(ago)));
        } else if restarts.count == 0 {
            output.push_str("No restarts recorded\n");
        }
        
        output
    }
    
    fn format_duration(d: chrono::Duration) -> String {
        let total_secs = d.num_seconds().abs();
        if total_secs < 60 {
            format!("{} seconds", total_secs)
        } else if total_secs < 3600 {
            format!("{} minutes", total_secs / 60)
        } else if total_secs < 86400 {
            format!("{} hours", total_secs / 3600)
        } else {
            format!("{} days", total_secs / 86400)
        }
    }
}
