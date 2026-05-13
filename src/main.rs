mod bittorent;

use clap::Parser;

use crate::bittorent::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.run().await {
        Ok(_) => {}
        Err(e) => eprintln!("Error: {e}"),
    }
}
