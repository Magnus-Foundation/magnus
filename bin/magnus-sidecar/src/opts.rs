use crate::cmd::{synthetic_load::SyntheticLoadArgs, tx_latency::TxLatencyArgs};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct MagnusSidecar {
    // TODO: add node args
    #[command(subcommand)]
    pub cmd: MagnusSidecarSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum MagnusSidecarSubcommand {
    SyntheticLoad(SyntheticLoadArgs),
    TxLatencyMonitor(TxLatencyArgs),
}
