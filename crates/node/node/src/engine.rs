use crate::{MagnusExecutionData, MagnusPayloadTypes};
use reth_node_api::{InvalidPayloadAttributesError, NewPayloadError, PayloadValidator};
use reth_primitives_traits::{AlloyBlockHeader as _, SealedBlock};
use std::sync::Arc;
use magnus_payload_types::MagnusPayloadAttributes;
use magnus_primitives::{Block, MagnusHeader};

/// Type encapsulating Magnus engine validation logic.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct MagnusEngineValidator;

impl MagnusEngineValidator {
    /// Creates a new [`MagnusEngineValidator`] with the given chain spec.
    pub fn new() -> Self {
        Self {}
    }
}

impl PayloadValidator<MagnusPayloadTypes> for MagnusEngineValidator {
    type Block = Block;

    fn convert_payload_to_block(
        &self,
        payload: MagnusExecutionData,
    ) -> Result<SealedBlock<Self::Block>, NewPayloadError> {
        let MagnusExecutionData {
            block,
            validator_set: _,
        } = payload;
        Ok(Arc::unwrap_or_clone(block))
    }

    fn validate_payload_attributes_against_header(
        &self,
        attr: &MagnusPayloadAttributes,
        header: &MagnusHeader,
    ) -> Result<(), InvalidPayloadAttributesError> {
        // Ensure that payload attributes timestamp is not in the past
        if attr.timestamp < header.timestamp() {
            return Err(InvalidPayloadAttributesError::InvalidTimestamp);
        }
        Ok(())
    }
}
