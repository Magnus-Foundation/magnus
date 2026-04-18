use crate::cmd::max_tps::MaxTpsArgs;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct MagnusBench {
    #[command(subcommand)]
    pub cmd: MagnusBenchSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum MagnusBenchSubcommand {
    RunMaxTps(MaxTpsArgs),
}
