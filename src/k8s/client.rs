use crate::cli::Cli;
use crate::error::Result;
use kube::{Client, Config};

/// 加载 kubeconfig 并创建 Kubernetes Client
pub async fn build_client(cli: &Cli) -> Result<Client> {
    let mut config = if let Some(kubeconfig_path) = &cli.kubeconfig {
        Config::from_custom_kubeconfig(
            kube::config::Kubeconfig::read_from(kubeconfig_path)?,
            &kube::config::KubeConfigOptions {
                context: cli.context.clone(),
                ..Default::default()
            },
        )
        .await?
    } else {
        Config::from_kubeconfig(&kube::config::KubeConfigOptions {
            context: cli.context.clone(),
            ..Default::default()
        })
        .await?
    };

    config.accept_invalid_certs = false;

    let client = Client::try_from(config)?;
    Ok(client)
}
