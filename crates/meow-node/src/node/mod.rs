use error::NodeError;

pub mod error;

pub type Result<T> = std::result::Result<T, NodeError>;

/// It is a MEOW network node implementation.
pub struct Node {}

impl Node {
    /// Creates a new node.
    pub fn new() -> Self {
        Self {}
    }

    /// Runs the node.
    pub async fn run(&self) -> Result<()> {
        println!("Meow!");
        Ok(())
    }
}
