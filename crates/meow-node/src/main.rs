use clap::{command, Parser};
use meow_node::commands::Command;

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

fn main() {
    let args = Args::parse();

    args.command.run();
}
