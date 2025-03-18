use clap::{command, Parser};

#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum Command {
    SayMeow,
}

impl Command {
    pub fn run(&self) {
        match self {
            Command::SayMeow => println!("Meow!"),
        }
    }
}
