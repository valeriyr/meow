use clap::{command, Parser};

/// The main command line commands.
#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum Command {
    /// Say meow!
    SayMeow,
}

impl Command {
    /// Runs the command.
    pub fn run(self) -> Result<(), anyhow::Error> {
        match self {
            Command::SayMeow => {
                println!("Meow!");
                Ok(())
            }
        }
    }
}
