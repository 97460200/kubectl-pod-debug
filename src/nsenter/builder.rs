/// namespace 类型到 nsenter 标志的映射
static NS_FLAGS: &[(&str, &str)] = &[
    ("network", "-n"),
    ("pid", "-p"),
    ("mount", "-m"),
    ("uts", "-u"),
    ("ipc", "-i"),
];

/// 构建 nsenter 命令
///
/// # 参数
/// - `pid`: 容器进程 PID
/// - `ns_type`: namespace 类型（"all", "network", "pid" 等）
/// - `command`: 要在 namespace 中执行的命令（空则使用 /bin/bash）
pub fn build_nsenter_command(pid: u32, ns_type: &str, enter_mount: bool, command: &[String]) -> String {
    let ns_flags = if ns_type == "all" {
        if enter_mount {
            "-a".to_string()
        } else {
            "-n -p -u -i".to_string()
        }
    } else {
        NS_FLAGS
            .iter()
            .filter(|(name, _)| *name == ns_type)
            .map(|(_, flag)| flag.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };

    let cmd = if command.is_empty() {
        "/bin/bash".to_string()
    } else {
        command.join(" ")
    };

    format!("nsenter -t {} {} -- {}", pid, ns_flags, cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_all_ns_no_mount() {
        let cmd = build_nsenter_command(12345, "all", false, &[]);
        assert_eq!(cmd, "nsenter -t 12345 -n -p -u -i -- /bin/bash");
    }

    #[test]
    fn test_build_all_ns_with_mount() {
        let cmd = build_nsenter_command(12345, "all", true, &[]);
        assert_eq!(cmd, "nsenter -t 12345 -a -- /bin/bash");
    }

    #[test]
    fn test_build_network_ns() {
        let cmd = build_nsenter_command(12345, "network", false, &[]);
        assert_eq!(cmd, "nsenter -t 12345 -n -- /bin/bash");
    }

    #[test]
    fn test_build_with_command() {
        let cmd = build_nsenter_command(12345, "network", false, &["tcpdump".to_string(), "-i".to_string(), "eth0".to_string()]);
        assert_eq!(cmd, "nsenter -t 12345 -n -- tcpdump -i eth0");
    }

    #[test]
    fn test_build_pid_ns() {
        let cmd = build_nsenter_command(12345, "pid", false, &[]);
        assert_eq!(cmd, "nsenter -t 12345 -p -- /bin/bash");
    }
}
