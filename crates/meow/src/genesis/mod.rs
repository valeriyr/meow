pub mod output;

use clap::Parser;
use meow_genesis::Genesis;
use meow_types::address::Address;

use crate::genesis::output::GenesisOutput;

/// Commands for interacting with a running meow node.
#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum GenesisCommand {
    /// Build a genesis state.
    Build {
        /// The file containing the initial allocations of MEOW coins in the genesis state.
        allocations: String,
        /// The path to the output file.
        to: String,
    },
}

impl GenesisCommand {
    pub fn run(self) -> anyhow::Result<GenesisOutput> {
        match self {
            GenesisCommand::Build { allocations, to } => {
                let allocations = std::fs::read_to_string(allocations)?
                    .lines()
                    .map(|line| match line.split_once(',') {
                        Some((address, amount)) => {
                            let address: Address = address.trim().parse()?;
                            let amount: u64 = amount.trim().parse()?;
                            Ok((address, amount))
                        }
                        None => anyhow::bail!("invalid allocation line: {}", line),
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;

                let genesis = Genesis::build(&allocations)?;
                let genesis_bytes = bcs::to_bytes(&genesis)?;

                std::fs::write(to, genesis_bytes)?;

                Ok(GenesisOutput::from(genesis))
            }
        }
    }
}
