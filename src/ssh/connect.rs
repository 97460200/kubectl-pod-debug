use std::sync::Arc;

use async_trait::async_trait;
use russh::client;
use russh::keys::{self, key};
use russh::Disconnect;
use tracing::{debug, info};

use crate::error::{PodDebugError, Result};

/// SSH client handler that implements the russh client::Handler trait.
#[derive(Debug)]
pub struct SshClient;

#[async_trait]
impl client::Handler for SshClient {
    type Error = PodDebugError;

    async fn check_server_key(
        &mut self,
        server_public_key: &key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // TODO: implement proper host key verification in a future version
        debug!(
            "Accepting server key (fingerprint: {})",
            server_public_key.fingerprint()
        );
        Ok(true)
    }
}

/// Connect to a remote host via SSH using public key authentication.
///
/// # Arguments
/// * `host` - Remote hostname or IP address
/// * `port` - SSH port number
/// * `user` - SSH username
/// * `key_path` - Path to the SSH private key file (supports `~` expansion)
///
/// # Returns
/// A `client::Handle<SshClient>` that can be used to open channels and execute commands.
pub async fn connect(
    host: &str,
    port: u16,
    user: &str,
    key_path: &str,
) -> Result<client::Handle<SshClient>> {
    // Expand ~ in the key path
    let expanded_path = shellexpand::tilde(key_path);
    let key_path_str = expanded_path.as_ref();

    info!(
        "Connecting to {}:{} as {} with key {}",
        host, port, user, key_path_str
    );

    // Read the private key
    let key_pair = keys::load_secret_key(key_path_str, None).map_err(|e| {
        PodDebugError::SshAuthFailed {
            user: user.to_string(),
            reason: format!("Failed to read private key '{}': {}", key_path_str, e),
        }
    })?;

    // Configure the SSH client
    let config = Arc::new(client::Config::default());
    let handler = SshClient;

    // Establish the SSH connection
    let addr = format!("{}:{}", host, port);
    let mut session = client::connect(config, addr.as_str(), handler)
        .await
        .map_err(|e| PodDebugError::SshConnectFailed {
            node: host.to_string(),
            reason: format!("{}", e),
        })?;

    // Authenticate with public key
    let authenticated = session
        .authenticate_publickey(user, Arc::new(key_pair))
        .await
        .map_err(|e| PodDebugError::SshAuthFailed {
            user: user.to_string(),
            reason: format!("Authentication error: {}", e),
        })?;

    if !authenticated {
        return Err(PodDebugError::SshAuthFailed {
            user: user.to_string(),
            reason: "Public key authentication rejected by server".to_string(),
        });
    }

    info!("SSH authenticated successfully to {}:{}", host, port);
    Ok(session)
}

/// Disconnect an SSH session gracefully.
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
