// Module for mip20_factory precompile
pub mod dispatch;

pub use magnus_contracts::precompiles::{IMIP20Factory, MIP20FactoryError, MIP20FactoryEvent};
use magnus_precompile_macros::contract;

use crate::{
    PATH_USD_ADDRESS, MIP20_FACTORY_ADDRESS,
    error::{Result, MagnusPrecompileError},
    mip20::{MIP20Error, MIP20Token, USD_CURRENCY, is_mip20_prefix},
};
use alloy::{
    primitives::{Address, B256, keccak256},
    sol_types::SolValue,
};
use tracing::trace;

/// Number of reserved addresses (0 to RESERVED_SIZE-1) that cannot be deployed via factory
const RESERVED_SIZE: u64 = 1024;

/// MIP20 token address prefix (12 bytes): 0x20C000000000000000000000
const MIP20_PREFIX_BYTES: [u8; 12] = [
    0x20, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[contract(addr = MIP20_FACTORY_ADDRESS)]
pub struct MIP20Factory {}

/// Computes the deterministic MIP20 address from sender and salt.
/// Returns the address and the lower bytes used for derivation.
fn compute_mip20_address(sender: Address, salt: B256) -> (Address, u64) {
    let hash = keccak256((sender, salt).abi_encode());

    // Take first 8 bytes of hash as lower bytes
    let mut padded = [0u8; 8];
    padded.copy_from_slice(&hash[..8]);
    let lower_bytes = u64::from_be_bytes(padded);

    // Construct the address: MIP20_PREFIX (12 bytes) || hash[..8] (8 bytes)
    let mut address_bytes = [0u8; 20];
    address_bytes[..12].copy_from_slice(&MIP20_PREFIX_BYTES);
    address_bytes[12..].copy_from_slice(&hash[..8]);

    (Address::from(address_bytes), lower_bytes)
}

// Precompile functions
impl MIP20Factory {
    /// Initializes the MIP20 factory contract.
    pub fn initialize(&mut self) -> Result<()> {
        // must ensure the account is not empty, by setting some code
        self.__initialize()
    }

    /// Computes the deterministic address for a token given sender and salt.
    /// Reverts if the computed address would be in the reserved range.
    pub fn get_token_address(&self, call: IMIP20Factory::getTokenAddressCall) -> Result<Address> {
        let (address, lower_bytes) = compute_mip20_address(call.sender, call.salt);

        // Check if address would be in reserved range
        if lower_bytes < RESERVED_SIZE {
            return Err(MagnusPrecompileError::MIP20Factory(
                MIP20FactoryError::address_reserved(),
            ));
        }

        Ok(address)
    }

    /// Returns true if the address is a valid MIP20 token.
    ///
    /// Checks both:
    /// 1. The address has the correct MIP20 prefix
    /// 2. The address has code deployed (non-empty code hash)
    pub fn is_mip20(&self, token: Address) -> Result<bool> {
        if !is_mip20_prefix(token) {
            return Ok(false);
        }
        // Check if the token has code deployed (non-empty code hash)
        self.storage
            .with_account_info(token, |info| Ok(!info.is_empty_code_hash()))
    }

    pub fn create_token(
        &mut self,
        sender: Address,
        call: IMIP20Factory::createTokenCall,
    ) -> Result<Address> {
        trace!(%sender, ?call, "Create token");

        // Compute the deterministic address from sender and salt
        let (token_address, lower_bytes) = compute_mip20_address(sender, call.salt);

        if self.is_mip20(token_address)? {
            return Err(MagnusPrecompileError::MIP20Factory(
                MIP20FactoryError::token_already_exists(token_address),
            ));
        }

        // Ensure that the quote token is a valid MIP20 that is currently deployed.
        if !self.is_mip20(call.quoteToken)? {
            return Err(MIP20Error::invalid_quote_token().into());
        }

        // If token is USD, its quote token must also be USD
        if call.currency == USD_CURRENCY
            && MIP20Token::from_address(call.quoteToken)?.currency()? != USD_CURRENCY
        {
            return Err(MIP20Error::invalid_quote_token().into());
        }

        // Check if address is in reserved range
        if lower_bytes < RESERVED_SIZE {
            return Err(MagnusPrecompileError::MIP20Factory(
                MIP20FactoryError::address_reserved(),
            ));
        }

        MIP20Token::from_address(token_address)?.initialize(
            sender,
            &call.name,
            &call.symbol,
            &call.currency,
            call.quoteToken,
            call.admin,
        )?;

        self.emit_event(MIP20FactoryEvent::TokenCreated(
            IMIP20Factory::TokenCreated {
                token: token_address,
                name: call.name,
                symbol: call.symbol,
                currency: call.currency,
                quoteToken: call.quoteToken,
                admin: call.admin,
                salt: call.salt,
            },
        ))?;

        Ok(token_address)
    }

    /// Creates a token at a reserved address
    /// Internal function used to deploy MIP20s at reserved addresses at genesis or hardforks
    pub fn create_token_reserved_address(
        &mut self,
        address: Address,
        name: &str,
        symbol: &str,
        currency: &str,
        quote_token: Address,
        admin: Address,
    ) -> Result<Address> {
        // Validate that the address has a MIP20 prefix
        if !is_mip20_prefix(address) {
            return Err(MIP20Error::invalid_token().into());
        }

        // Validate that the address is not already deployed
        if self.is_mip20(address)? {
            return Err(MagnusPrecompileError::MIP20Factory(
                MIP20FactoryError::token_already_exists(address),
            ));
        }

        // quote_token must be address(0) or a valid MIP20
        if !quote_token.is_zero() {
            // pathUSD must set address(0) as the quote token
            // or the mip20 must be a valid deployed token
            if address == PATH_USD_ADDRESS || !self.is_mip20(quote_token)? {
                return Err(MIP20Error::invalid_quote_token().into());
            }
            // If token is USD, its quote token must also be USD
            if currency == USD_CURRENCY
                && MIP20Token::from_address(quote_token)?.currency()? != USD_CURRENCY
            {
                return Err(MIP20Error::invalid_quote_token().into());
            }
        }

        // Validate that the address is within the reserved range
        // Reserved addresses have their last 8 bytes represent a value < RESERVED_SIZE
        let mut padded = [0u8; 8];
        padded.copy_from_slice(&address.as_slice()[12..]);
        let lower_bytes = u64::from_be_bytes(padded);
        if lower_bytes >= RESERVED_SIZE {
            return Err(MagnusPrecompileError::MIP20Factory(
                MIP20FactoryError::address_not_reserved(),
            ));
        }

        let mut token = MIP20Token::from_address(address)?;
        token.initialize(admin, name, symbol, currency, quote_token, admin)?;

        self.emit_event(MIP20FactoryEvent::TokenCreated(
            IMIP20Factory::TokenCreated {
                token: address,
                name: name.into(),
                symbol: symbol.into(),
                currency: currency.into(),
                quoteToken: quote_token,
                admin,
                salt: B256::ZERO,
            },
        ))?;

        Ok(address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PATH_USD_ADDRESS,
        error::MagnusPrecompileError,
        storage::{ContractStorage, StorageCtx, hashmap::HashMapStorageProvider},
        test_util::MIP20Setup,
    };
    use alloy::primitives::{Address, address};

    #[test]
    fn test_is_initialized() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);

        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Factory::new();

            // Factory should not be initialized before initialize() call
            assert!(!factory.is_initialized()?);

            // After initialize(), factory should be initialized
            factory.initialize()?;
            assert!(factory.is_initialized()?);

            // Creating a new handle should still see initialized state
            let factory2 = MIP20Factory::new();
            assert!(factory2.is_initialized()?);

            Ok(())
        })
    }

    #[test]
    fn test_is_mip20_prefix() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);

        StorageCtx::enter(&mut storage, || {
            // PATH_USD has correct prefix
            assert!(is_mip20_prefix(PATH_USD_ADDRESS));

            // Address with MIP20 prefix (0x20C0...)
            let mip20_addr = Address::from(alloy::hex!("20C0000000000000000000000000000000001234"));
            assert!(is_mip20_prefix(mip20_addr));

            // Random address does not have MIP20 prefix
            let random = Address::random();
            assert!(!is_mip20_prefix(random));

            Ok(())
        })
    }

    #[test]
    fn test_is_mip20() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let sender = Address::random();

        StorageCtx::enter(&mut storage, || {
            // Initialize pathUSD
            let _path_usd = MIP20Setup::path_usd(sender).apply()?;

            let factory = MIP20Factory::new();

            // PATH_USD should be valid (has code deployed)
            assert!(factory.is_mip20(PATH_USD_ADDRESS)?);

            // Address with MIP20 prefix but no code should be invalid
            let no_code_mip20 = address!("20C0000000000000000000000000000000000002");
            assert!(!factory.is_mip20(no_code_mip20)?);

            // Random address (wrong prefix) should be invalid
            assert!(!factory.is_mip20(Address::random())?);

            // Create a token via factory and verify it's valid
            let token = MIP20Setup::create("Test", "TST", sender).apply()?;
            assert!(factory.is_mip20(token.address())?);

            Ok(())
        })
    }

    #[test]
    fn test_get_token_address() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);

        StorageCtx::enter(&mut storage, || {
            let factory = MIP20Factory::new();
            let sender = Address::random();
            let salt = B256::random();

            // get_token_address should return same address as compute_mip20_address
            let call = IMIP20Factory::getTokenAddressCall { sender, salt };
            let address = factory.get_token_address(call)?;
            let (expected, _) = compute_mip20_address(sender, salt);
            assert_eq!(address, expected);

            // Calling with same params should be deterministic
            let call2 = IMIP20Factory::getTokenAddressCall { sender, salt };
            assert_eq!(factory.get_token_address(call2)?, address);

            Ok(())
        })
    }

    #[test]
    fn test_compute_mip20_address_deterministic() {
        let sender1 = Address::random();
        let sender2 = Address::random();
        let salt1 = B256::random();
        let salt2 = B256::random();

        let (addr0, lower0) = compute_mip20_address(sender1, salt1);
        let (addr1, lower1) = compute_mip20_address(sender1, salt1);
        assert_eq!(addr0, addr1);
        assert_eq!(lower0, lower1);

        // Same salt with different senders should produce different addresses
        let (addr2, lower2) = compute_mip20_address(sender1, salt1);
        let (addr3, lower3) = compute_mip20_address(sender2, salt1);
        assert_ne!(addr2, addr3);
        assert_ne!(lower2, lower3);

        // Same sender with different salts should produce different addresses
        let (addr4, lower4) = compute_mip20_address(sender1, salt1);
        let (addr5, lower5) = compute_mip20_address(sender1, salt2);
        assert_ne!(addr4, addr5);
        assert_ne!(lower4, lower5);

        // All addresses should have MIP20 prefix
        assert!(is_mip20_prefix(addr1));
        assert!(is_mip20_prefix(addr2));
        assert!(is_mip20_prefix(addr3));
        assert!(is_mip20_prefix(addr4));
        assert!(is_mip20_prefix(addr5));
    }

    #[test]
    fn test_create_token() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let sender = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Setup::factory()?;
            let path_usd = MIP20Setup::path_usd(sender).apply()?;
            factory.clear_emitted_events();

            let salt1 = B256::random();
            let salt2 = B256::random();
            let call1 = IMIP20Factory::createTokenCall {
                name: "Test Token 1".to_string(),
                symbol: "TEST1".to_string(),
                currency: "USD".to_string(),
                quoteToken: path_usd.address(),
                admin: sender,
                salt: salt1,
            };
            let call2 = IMIP20Factory::createTokenCall {
                name: "Test Token 2".to_string(),
                symbol: "TEST2".to_string(),
                currency: "USD".to_string(),
                quoteToken: path_usd.address(),
                admin: sender,
                salt: salt2,
            };

            let token_addr_1 = factory.create_token(sender, call1.clone())?;
            let token_addr_2 = factory.create_token(sender, call2.clone())?;

            // Verify addresses are different
            assert_ne!(token_addr_1, token_addr_2);

            // Verify addresses have MIP20 prefix
            assert!(is_mip20_prefix(token_addr_1));
            assert!(is_mip20_prefix(token_addr_2));

            // Verify tokens are valid MIP20s
            assert!(factory.is_mip20(token_addr_1)?);
            assert!(factory.is_mip20(token_addr_2)?);

            // Verify event emission
            factory.assert_emitted_events(vec![
                MIP20FactoryEvent::TokenCreated(IMIP20Factory::TokenCreated {
                    token: token_addr_1,
                    name: call1.name,
                    symbol: call1.symbol,
                    currency: call1.currency,
                    quoteToken: call1.quoteToken,
                    admin: call1.admin,
                    salt: call1.salt,
                }),
                MIP20FactoryEvent::TokenCreated(IMIP20Factory::TokenCreated {
                    token: token_addr_2,
                    name: call2.name,
                    symbol: call2.symbol,
                    currency: call2.currency,
                    quoteToken: call2.quoteToken,
                    admin: call2.admin,
                    salt: call2.salt,
                }),
            ]);

            Ok(())
        })
    }

    #[test]
    fn test_create_token_invalid_quote_token() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let sender = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Setup::factory()?;
            MIP20Setup::path_usd(sender).apply()?;

            let invalid_call = IMIP20Factory::createTokenCall {
                name: "Test Token".to_string(),
                symbol: "TEST".to_string(),
                currency: "USD".to_string(),
                quoteToken: Address::random(),
                admin: sender,
                salt: B256::random(),
            };

            let result = factory.create_token(sender, invalid_call);
            assert_eq!(
                result.unwrap_err(),
                MagnusPrecompileError::MIP20(MIP20Error::invalid_quote_token())
            );
            Ok(())
        })
    }

    #[test]
    fn test_create_token_usd_with_non_usd_quote() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let sender = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Setup::factory()?;
            let _path_usd = MIP20Setup::path_usd(sender).apply()?;
            let eur_token = MIP20Setup::create("EUR Token", "EUR", sender)
                .currency("EUR")
                .apply()?;

            let invalid_call = IMIP20Factory::createTokenCall {
                name: "USD Token".to_string(),
                symbol: "USDT".to_string(),
                currency: "USD".to_string(),
                quoteToken: eur_token.address(),
                admin: sender,
                salt: B256::random(),
            };

            let result = factory.create_token(sender, invalid_call);
            assert_eq!(
                result.unwrap_err(),
                MagnusPrecompileError::MIP20(MIP20Error::invalid_quote_token())
            );
            Ok(())
        })
    }

    #[test]
    fn test_create_token_quote_token_not_deployed() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let sender = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Setup::factory()?;
            MIP20Setup::path_usd(sender).apply()?;

            // Create an address with MIP20 prefix but no code
            let non_existent_mip20 =
                Address::from(alloy::hex!("20C0000000000000000000000000000000009999"));
            let invalid_call = IMIP20Factory::createTokenCall {
                name: "Test Token".to_string(),
                symbol: "TEST".to_string(),
                currency: "USD".to_string(),
                quoteToken: non_existent_mip20,
                admin: sender,
                salt: B256::random(),
            };

            let result = factory.create_token(sender, invalid_call);
            assert_eq!(
                result.unwrap_err(),
                MagnusPrecompileError::MIP20(MIP20Error::invalid_quote_token())
            );
            Ok(())
        })
    }

    #[test]
    fn test_create_token_already_deployed() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let sender = Address::random();
        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Setup::factory()?;
            MIP20Setup::path_usd(sender).apply()?;

            let salt = B256::random();
            let create_token_call = IMIP20Factory::createTokenCall {
                name: "Test Token".to_string(),
                symbol: "TEST".to_string(),
                currency: "USD".to_string(),
                quoteToken: PATH_USD_ADDRESS,
                admin: sender,
                salt,
            };

            let token = factory.create_token(sender, create_token_call.clone())?;
            let result = factory.create_token(sender, create_token_call);
            assert_eq!(
                result.unwrap_err(),
                MagnusPrecompileError::MIP20Factory(MIP20FactoryError::TokenAlreadyExists(
                    IMIP20Factory::TokenAlreadyExists { token }
                ))
            );

            Ok(())
        })
    }

    #[test]
    fn test_create_token_reserved_address_rejects_invalid_prefix() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Factory::new();
            factory.initialize()?;

            let result = factory.create_token_reserved_address(
                Address::random(), // No MIP20 prefix
                "Test",
                "TST",
                "USD",
                Address::ZERO,
                admin,
            );

            assert_eq!(
                result.unwrap_err(),
                MagnusPrecompileError::MIP20(MIP20Error::invalid_token())
            );

            Ok(())
        })
    }

    #[test]
    fn test_create_token_reserved_address_rejects_already_deployed() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Factory::new();
            factory.initialize()?;

            factory.create_token_reserved_address(
                PATH_USD_ADDRESS,
                "pathUSD",
                "pathUSD",
                "USD",
                Address::ZERO,
                admin,
            )?;

            let result = factory.create_token_reserved_address(
                PATH_USD_ADDRESS,
                "pathUSD",
                "pathUSD",
                "USD",
                Address::ZERO,
                admin,
            );

            assert_eq!(
                result.unwrap_err(),
                MagnusPrecompileError::MIP20Factory(MIP20FactoryError::token_already_exists(
                    PATH_USD_ADDRESS
                ))
            );

            Ok(())
        })
    }

    #[test]
    fn test_create_token_reserved_address_rejects_non_usd_quote_for_usd_token() -> eyre::Result<()>
    {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();

        StorageCtx::enter(&mut storage, || {
            let eur_token = MIP20Setup::create("EUR Token", "EUR", admin)
                .currency("EUR")
                .apply()?;

            let mut factory = MIP20Factory::new();

            let result = factory.create_token_reserved_address(
                address!("20C0000000000000000000000000000000000001"), // reserved address
                "Test USD",
                "TUSD",
                "USD",
                eur_token.address(),
                admin,
            );

            assert_eq!(
                result.unwrap_err(),
                MagnusPrecompileError::MIP20(MIP20Error::invalid_quote_token())
            );

            Ok(())
        })
    }

    #[test]
    fn test_create_token_reserved_address_rejects_non_reserved_address() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();

        StorageCtx::enter(&mut storage, || {
            let _path_usd = MIP20Setup::path_usd(admin).apply()?;
            let mut factory = MIP20Factory::new();

            // 0x9999 = 39321 > 1024 (RESERVED_SIZE)
            let non_reserved = address!("20C0000000000000000000000000000000009999");

            let result = factory.create_token_reserved_address(
                non_reserved,
                "Test",
                "TST",
                "USD",
                PATH_USD_ADDRESS,
                admin,
            );

            assert_eq!(
                result.unwrap_err(),
                MagnusPrecompileError::MIP20Factory(MIP20FactoryError::address_not_reserved())
            );

            Ok(())
        })
    }

    #[test]
    fn test_create_token_reserved_address_requires_zero_addr_as_first_quote() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Factory::new();
            factory.initialize()?;

            // Try to create PATH_USD with a non-deployed MIP20 as quote_token
            let result = factory.create_token_reserved_address(
                PATH_USD_ADDRESS,
                "pathUSD",
                "pathUSD",
                "USD",
                address!("20C0000000000000000000000000000000000001"),
                admin,
            );
            assert!(matches!(
                result,
                Err(MagnusPrecompileError::MIP20(MIP20Error::InvalidQuoteToken(
                    _
                )))
            ));

            // Only possible to deploy PATH_USD (the first token) without a quote token
            factory.create_token_reserved_address(
                PATH_USD_ADDRESS,
                "pathUSD",
                "pathUSD",
                "USD",
                Address::ZERO,
                admin,
            )?;

            Ok(())
        })
    }

    #[test]
    fn test_path_usd_requires_zero_quote_token() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        let admin = Address::random();

        StorageCtx::enter(&mut storage, || {
            let mut factory = MIP20Factory::new();
            factory.initialize()?;

            let other_usd = factory.create_token_reserved_address(
                address!("20C0000000000000000000000000000000000001"),
                "testUSD",
                "testUSD",
                "USD",
                Address::ZERO,
                admin,
            )?;

            let result = factory.create_token_reserved_address(
                PATH_USD_ADDRESS,
                "pathUSD",
                "pathUSD",
                "USD",
                other_usd,
                admin,
            );
            assert!(matches!(
                result,
                Err(MagnusPrecompileError::MIP20(MIP20Error::InvalidQuoteToken(
                    _
                )))
            ));

            factory.create_token_reserved_address(
                PATH_USD_ADDRESS,
                "pathUSD",
                "pathUSD",
                "USD",
                Address::ZERO,
                admin,
            )?;

            assert!(MIP20Token::from_address(PATH_USD_ADDRESS)?.is_initialized()?);

            Ok(())
        })
    }
}
