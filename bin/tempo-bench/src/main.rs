mod cmd;
mod opts;

use clap::Parser;
use mimalloc::MiMalloc;
use opts::{MagnusBench, MagnusBenchSubcommand};

#[global_allocator]
// Increases RPS by ~5.5% at the time of
// writing. ~3.3% faster than jemalloc.
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = MagnusBench::parse();

    match args.cmd {
        MagnusBenchSubcommand::RunMaxTps(cmd) => cmd.run().await,
    }
}
