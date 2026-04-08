use clap::Parser;
use meow_node::commands::Command;
use tracing_subscriber::{EnvFilter, fmt};

/// The main command line arguments.
#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    rename_all = "kebab-case",
    author,
    version = env!("CARGO_PKG_VERSION"),
    propagate_version = true,
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

/// The main function.
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    init_tracing();

    let args = Args::parse();

    args.command.run().await
}

/// Initializes tracing with a default filter that can be overridden by the `RUST_LOG` environment variable.
fn init_tracing() {
    let default_filter = "meow_node=info,meow_nakamoto=info,meow_gossip_network=info,warn";
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .compact()
        .init();
}
