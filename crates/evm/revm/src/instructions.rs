use crate::evm::MagnusContext;
use alloy_evm::Database;
use revm::{
    handler::instructions::EthInstructions,
    interpreter::{Instruction, InstructionContext, interpreter::EthInterpreter, push},
};
use magnus_chainspec::hardfork::MagnusHardfork;

/// Instruction ID for opcode returning milliseconds timestamp.
const MILLIS_TIMESTAMP: u8 = 0x4F;

/// Gas cost for [`MILLIS_TIMESTAMP`] instruction. Same as other opcodes accessing block information.
const MILLIS_TIMESTAMP_GAS_COST: u64 = 2;

/// Alias for Tempo-specific [`InstructionContext`].
type MagnusInstructionContext<'a, DB> = InstructionContext<'a, MagnusContext<DB>, EthInterpreter>;

/// Opcode returning current timestamp in milliseconds.
fn millis_timestamp<DB: Database>(context: MagnusInstructionContext<'_, DB>) {
    push!(context.interpreter, context.host.block.timestamp_millis());
}

/// Returns configured instructions table for Tempo.
pub(crate) fn magnus_instructions<DB: Database>(
    spec: MagnusHardfork,
) -> EthInstructions<EthInterpreter, MagnusContext<DB>> {
    let mut instructions = EthInstructions::new_mainnet_with_spec(spec.into());
    if !spec.is_t1c() {
        instructions.insert_instruction(
            MILLIS_TIMESTAMP,
            Instruction::new(millis_timestamp, MILLIS_TIMESTAMP_GAS_COST),
        );
    }
    instructions
}
