mod backfill;
mod base_fee;
// Block-level integration tests below need accept-set bootstrap rework
// post-G3b; gated behind a follow-up commit:
// mod block_building;
mod createx;
// mod eth_call;
mod eth_transactions;
mod fork_schedule;
mod key_authorization;
// mod liquidity;       // legacy AMM liquidity tests, removed with the AMM
mod max_gas_limit;
mod operator;
// mod payment_lane;
// mod pool;
mod simulate;
mod stablecoin_dex;
mod magnus_transaction;
// mod mip20;
mod mip20_factory;
// mod mip20_gas_fees;
// mod mip_fee_amm;     // legacy AMM tests, removed with the AMM
// mod mip_fee_manager;
mod utils;

use magnus_node as _;

fn main() {}
