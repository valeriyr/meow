use clap::Parser;
use meow_node::commands::Command;

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
    let args = Args::parse();

    args.command.run().await
}
