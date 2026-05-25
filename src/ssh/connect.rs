use std::sync::Arc;

use async_trait::async_trait;
use russh::client;
use russh::keys::{self, key};
use russh::Disconnect;
use tracing::{debug, info};

use crate::error::{PodDebugError, Result};

#[derive(Debug)]
pub struct SshClient;

#[async_trait]
impl client::Handler for SshClient {
    type Error = PodDebugError;

    async fn check_server_key(
        &mut self,
        server_public_key: &key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        debug!(
            "Accepting server key (fingerprint: {})",
            server_public_key.fingerprint()
        );
        Ok(true)
    }
}

pub async fn connect(
    host: &str,
    port: u16,
    user: &str,
    key_path: &str,
    password: Option<&str>,
) -> Result<client::Handle<SshClient>> {
    let expanded_path = shellexpand::tilde(key_path);
    let key_path_str = expanded_path.as_ref();

    let config = Arc::new(client::Config::default());
    let handler = SshClient;

    let addr = format!("{}:{}", host, port);
    let mut session = client::connect(config, addr.as_str(), handler)
        .await
        .map_err(|e| PodDebugError::SshConnectFailed {
            node: host.to_string(),
            reason: format!("{}", e),
        })?;

    let key_pair = keys::load_secret_key(key_path_str, None);
    let mut authenticated = false;

    if let Ok(key_pair) = key_pair {
        info!(
            "Trying SSH key auth: {}@{}:{} with key {}",
            user, host, port, key_path_str
        );
        authenticated = session
            .authenticate_publickey(user, Arc::new(key_pair))
            .await
            .unwrap_or(false);
    }

    if !authenticated {
        if let Some(pwd) = password {
            info!(
                "Trying SSH password auth: {}@{}:{}",
                user, host, port
            );
            authenticated = session
                .authenticate_password(user, pwd)
                .await
                .map_err(|e| PodDebugError::SshAuthFailed {
                    user: user.to_string(),
                    reason: format!("Password authentication error: {}", e),
                })?;
        } else {
            info!(
                "SSH key auth failed, prompting for password: {}@{}:{}",
                user, host, port
            );
            authenticated = prompt_password_auth(&mut session, user).await?;
        }
    }

    if !authenticated {
        return Err(PodDebugError::SshAuthFailed {
            user: user.to_string(),
            reason: "All authentication methods failed".to_string(),
        });
    }

    info!("SSH authenticated successfully to {}:{}", host, port);
    Ok(session)
}

async fn prompt_password_auth(
    session: &mut client::Handle<SshClient>,
    user: &str,
) -> Result<bool> {
    use std::io::{self, Write};

    print!("Password for {}@SSH: ", user);
    io::stdout().flush().map_err(|e| PodDebugError::SshAuthFailed {
        user: user.to_string(),
        reason: format!("Failed to flush stdout: {}", e),
    })?;

    let password = rpassword::read_password().map_err(|e| PodDebugError::SshAuthFailed {
        user: user.to_string(),
        reason: format!("Failed to read password: {}", e),
    })?;

    if password.is_empty() {
        return Ok(false);
    }

    let result: bool = session
        .authenticate_password(user, &password)
        .await
        .map_err(|e| PodDebugError::SshAuthFailed {
            user: user.to_string(),
            reason: format!("Password authentication error: {}", e),
        })?;
    Ok(result)
}

#[allow(dead_code)]
pub async fn disconnect(session: &mut client::Handle<SshClient>) -> Result<()> {
    session
        .disconnect(Disconnect::ByApplication, "", "")
        .await
        .map_err(|e| PodDebugError::SshConnectFailed {
            node: "unknown".to_string(),
            reason: format!("Disconnect error: {}", e),
        })?;
    Ok(())
}
