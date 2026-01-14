use crate::opts::{MagnusSidecar, MagnusSidecarSubcommand};
use clap::Parser;

mod cmd;
pub mod monitor;
mod opts;
mod synthetic_load;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = MagnusSidecar::parse();

    match args.cmd {
        MagnusSidecarSubcommand::FeeAMMMonitor(cmd) => cmd.run().await,
        MagnusSidecarSubcommand::SimpleArb(cmd) => cmd.run().await,
        MagnusSidecarSubcommand::SyntheticLoad(cmd) => cmd.run().await,
        MagnusSidecarSubcommand::TxLatencyMonitor(cmd) => cmd.run().await,
    }
}
