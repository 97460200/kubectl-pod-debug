mod cli;
mod error;

use cli::Cli;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> error::Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose {
        EnvFilter::new("kubectl_pod_debug=debug")
    } else {
        EnvFilter::new("kubectl_pod_debug=warn")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    tracing::info!("kubectl-pod-debug starting");
    tracing::debug!("CLI args: {:?}", cli);

    println!("Pod: {}, Namespace: {}", cli.pod_name, cli.namespace);

    Ok(())
}
