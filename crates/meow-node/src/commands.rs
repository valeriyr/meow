use clap::{Parser, command};

use crate::node::Node;

/// The main command line commands.
#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum Command {
    /// Run the node.
    Run,
}

impl Command {
    /// Runs the command.
    pub async fn run(self) -> Result<(), anyhow::Error> {
        Ok(match self {
            Command::Run => Node::new().run().await?,
        })
    }
}
