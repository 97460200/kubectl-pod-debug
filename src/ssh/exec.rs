use std::process::Command;

use russh::client;
use russh::ChannelMsg;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{PodDebugError, Result};
use crate::ssh::connect::SshClient;

fn set_raw_mode() {
    let _ = Command::new("stty").args(["raw", "-echo"]).status();
}

fn restore_terminal() {
    let _ = Command::new("stty").args(["sane"]).status();
}

/// Execute a command on the remote host via SSH and return its combined stdout/stderr output.
///
/// This opens a channel, executes the command, collects all output, and returns it as a String.
///
/// # Arguments
/// * `session` - An authenticated SSH session handle
/// * `command` - The command to execute on the remote host
///
/// # Returns
/// The combined stdout output of the command as a String.
pub async fn exec_command(
    session: &client::Handle<SshClient>,
    command: &str,
) -> Result<String> {
    let mut channel = session.channel_open_session().await.map_err(|e| {
        PodDebugError::SshConnectFailed {
            node: "unknown".to_string(),
            reason: format!("Failed to open channel: {}", e),
        }
    })?;

    channel.exec(true, command).await.map_err(|e| {
        PodDebugError::NsenterFailed {
            reason: format!("Failed to exec command: {}", e),
        }
    })?;

    let mut output = Vec::new();
    let mut exit_code = None;

    loop {
        let Some(msg) = channel.wait().await else {
            break;
        };
        match msg {
            ChannelMsg::Data { ref data } => {
                output.extend_from_slice(data);
            }
            ChannelMsg::ExtendedData { ref data, .. } => {
                // Also collect stderr
                output.extend_from_slice(data);
            }
            ChannelMsg::ExitStatus { exit_status } => {
                exit_code = Some(exit_status);
            }
            _ => {}
        }
    }

    let output_str = String::from_utf8_lossy(&output).to_string();

    if let Some(code) = exit_code {
        if code != 0 {
            return Err(PodDebugError::NsenterFailed {
                reason: format!(
                    "Command '{}' exited with status {}: {}",
                    command, code, output_str
                ),
            });
        }
    }

    Ok(output_str)
}

/// Execute a command on the remote host via SSH with an interactive PTY.
///
/// This opens a channel, requests a PTY, executes the command, and connects
/// the local stdin/stdout to the remote PTY for interactive use.
///
/// # Arguments
/// * `session` - An authenticated SSH session handle
/// * `command` - The command to execute on the remote host
pub async fn interactive_shell(
    session: &client::Handle<SshClient>,
    command: &str,
) -> Result<()> {
    let mut channel = session.channel_open_session().await.map_err(|e| {
        PodDebugError::SshConnectFailed {
            node: "unknown".to_string(),
            reason: format!("Failed to open channel: {}", e),
        }
    })?;

    // Request a pseudo-terminal
    channel
        .request_pty(false, "xterm-256color", 80, 24, 0, 0, &[])
        .await
        .map_err(|e| PodDebugError::NsenterFailed {
            reason: format!("Failed to request PTY: {}", e),
        })?;

    channel.exec(true, command).await.map_err(|e| {
        PodDebugError::NsenterFailed {
            reason: format!("Failed to exec command: {}", e),
        }
    })?;

    set_raw_mode();

    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut buf = vec![0; 1024];
    let mut stdin_closed = false;

    loop {
        tokio::select! {
            // Read from local stdin and send to remote
            r = stdin.read(&mut buf), if !stdin_closed => {
                match r {
                    Ok(0) => {
                        stdin_closed = true;
                        channel.eof().await.map_err(|e| PodDebugError::NsenterFailed {
                            reason: format!("Failed to send EOF: {}", e),
                        })?;
                    }
                    Ok(n) => {
                        channel.data(&buf[..n]).await.map_err(|e| PodDebugError::NsenterFailed {
                            reason: format!("Failed to send data: {}", e),
                        })?;
                    }
                    Err(e) => {
                        restore_terminal();
                        return Err(PodDebugError::NsenterFailed {
                            reason: format!("Stdin read error: {}", e),
                        });
                    }
                }
            }
            // Read from remote and write to local stdout
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { ref data }) => {
                        stdout.write_all(data).await.map_err(|e| PodDebugError::NsenterFailed {
                            reason: format!("Stdout write error: {}", e),
                        })?;
                        stdout.flush().await.map_err(|e| PodDebugError::NsenterFailed {
                            reason: format!("Stdout flush error: {}", e),
                        })?;
                    }
                    Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                        // Write stderr to stdout as well
                        stdout.write_all(data).await.map_err(|e| PodDebugError::NsenterFailed {
                            reason: format!("Stdout write error (stderr): {}", e),
                        })?;
                        stdout.flush().await.map_err(|e| PodDebugError::NsenterFailed {
                            reason: format!("Stdout flush error (stderr): {}", e),
                        })?;
                    }
                    Some(ChannelMsg::ExitStatus { .. }) => {
                        if !stdin_closed {
                            channel.eof().await.map_err(|e| PodDebugError::NsenterFailed {
                                reason: format!("Failed to send EOF: {}", e),
                            })?;
                        }
                        break;
                    }
                    None | Some(ChannelMsg::Close) => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    restore_terminal();

    Ok(())
}
