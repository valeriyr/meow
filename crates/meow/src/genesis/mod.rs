//! `meow genesis` command: generate a genesis file from an allocations CSV.

pub mod output;

use std::path::PathBuf;

use clap::Parser;
use meow_genesis::Genesis;
use meow_types::address::Address;

use crate::genesis::output::GenesisOutput;

/// Commands for building genesis state.
#[derive(Parser)]
#[command(rename_all = "kebab-case", verbatim_doc_comment)]
pub enum GenesisCommand {
    /// Build a genesis state.
    Build {
        /// Path to a CSV file with initial coin allocations (one `<address>,<amount>` per line).
        allocations: PathBuf,
        /// Path to write the genesis output file.
        output: PathBuf,
    },
}

impl GenesisCommand {
    pub fn run(self, with_object_content: bool) -> anyhow::Result<GenesisOutput> {
        match self {
            GenesisCommand::Build {
                allocations,
                output,
            } => {
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

                std::fs::write(output, genesis_bytes)?;

                Ok(GenesisOutput::new(genesis, with_object_content))
            }
        }
    }
}
