//! ISO 20022 message types for cross-border payment interoperability.
//!
//! Supports key payment messages:
//! - pacs.008: FI to FI Customer Credit Transfer
//! - pacs.002: Payment Status Report
//! - camt.053: Bank to Customer Statement

pub mod messages;

use alloy_primitives::Address;
use crate::{
    addresses,
    error::{MagnusPrecompileError, Result},
};

/// ISO 20022 message type identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    /// pacs.008 - FI to FI Customer Credit Transfer
    CreditTransfer = 1,
    /// pacs.002 - Payment Status Report
    StatusReport = 2,
    /// camt.053 - Bank to Customer Statement
    Statement = 3,
}

impl TryFrom<u8> for MessageType {
    type Error = MagnusPrecompileError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::CreditTransfer),
            2 => Ok(Self::StatusReport),
            3 => Ok(Self::Statement),
            _ => Err(MagnusPrecompileError::InvalidInput(
                format!("unknown ISO 20022 message type: {}", value),
            )),
        }
    }
}

/// ISO 20022 precompile -- validates and emits structured payment messages.
pub struct ISO20022Processor {
    pub address: Address,
}

impl ISO20022Processor {
    pub fn new() -> Self {
        Self {
            address: addresses::ISO20022,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_type_from_u8() {
        assert_eq!(MessageType::try_from(1).unwrap(), MessageType::CreditTransfer);
        assert_eq!(MessageType::try_from(2).unwrap(), MessageType::StatusReport);
        assert_eq!(MessageType::try_from(3).unwrap(), MessageType::Statement);
        assert!(MessageType::try_from(0).is_err());
        assert!(MessageType::try_from(255).is_err());
    }
}
