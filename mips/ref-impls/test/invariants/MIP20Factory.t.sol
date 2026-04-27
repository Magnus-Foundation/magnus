// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import { MIP20 } from "../../src/MIP20.sol";
import { MIP20Factory } from "../../src/MIP20Factory.sol";
import { IMIP20 } from "../../src/interfaces/IMIP20.sol";
import { IMIP20Factory } from "../../src/interfaces/IMIP20Factory.sol";
import { InvariantBaseTest } from "./InvariantBaseTest.t.sol";

/// @title MIP20Factory Invariant Tests
/// @notice Fuzz-based invariant tests for the MIP20Factory implementation
/// @dev Tests invariants MAGNUS-FAC1 through MAGNUS-FAC12 as documented in README.md
contract MIP20FactoryInvariantTest is InvariantBaseTest {

    /// @dev Ghost variables for tracking operations
    uint256 private _totalTokensCreated;
    uint256 private _totalReservedAttempts;
    uint256 private _totalDuplicateAttempts;
    uint256 private _totalInvalidQuoteAttempts;
    uint256 private _totalNonUsdCurrencyCreated;
    uint256 private _totalUsdWithNonUsdQuoteRejected;
    uint256 private _totalReservedCreateAttempts;
    uint256 private _totalIsTIP20Checks;

    /// @dev Track created tokens and their properties
    address[] private _createdTokens;
    mapping(address => bool) private _isCreatedToken;
    mapping(bytes32 => address) private _saltToToken;
    mapping(address => bytes32) private _tokenToSalt;
    mapping(address => address) private _tokenToSender;

    /// @dev Track salts used by each sender
    mapping(address => bytes32[]) private _senderSalts;

    /// @notice Sets up the test environment
    function setUp() public override {
        super.setUp();

        targetContract(address(this));

        _setupInvariantBase();
        (_actors,) = _buildActors(10);

        // One-time constant checks (immutable after deployment)
        // MAGNUS-FAC8: isTIP20 consistency for system contracts
        assertTrue(factory.isTIP20(address(MagnusUSD)), "MAGNUS-FAC8: MagnusUSD should be MIP20");
        assertFalse(factory.isTIP20(address(factory)), "MAGNUS-FAC8: Factory should not be MIP20");
        assertFalse(factory.isTIP20(address(amm)), "MAGNUS-FAC8: AMM should not be MIP20");
    }

    /*//////////////////////////////////////////////////////////////
                            FUZZ HANDLERS
    //////////////////////////////////////////////////////////////*/

    /// @notice Handler for creating tokens
    /// @dev Tests MAGNUS-FAC1 (deterministic addresses), MAGNUS-FAC2 (address uniqueness)
    function createToken(
        uint256 actorSeed,
        bytes32 salt,
        uint256 nameIdx,
        uint256 symbolIdx
    )
        external
    {
        address actor = _selectActor(actorSeed);

        // Generate varied names and symbols
        string memory name = _generateName(nameIdx);
        string memory symbol = _generateSymbol(symbolIdx);

        // Predict the address before creation
        address predictedAddr;
        try factory.getTokenAddress(actor, salt) returns (address addr) {
            predictedAddr = addr;
        } catch (bytes memory reason) {
            // MAGNUS-FAC5: Reserved address range is enforced
            if (bytes4(reason) == IMIP20Factory.AddressReserved.selector) {
                _totalReservedAttempts++;
                return;
            }
            revert("Unknown error in getTokenAddress");
        }

        // Check if token already exists at this address
        if (predictedAddr.code.length != 0) {
            vm.startPrank(actor);
            try factory.createToken(name, symbol, "USD", MagnusUSD, admin, salt) {
                vm.stopPrank();
                revert("MAGNUS-FAC3: Should revert for existing token");
            } catch (bytes memory reason) {
                vm.stopPrank();
                if (bytes4(reason) == IMIP20Factory.TokenAlreadyExists.selector) {
                    _totalDuplicateAttempts++;
                    return;
                }
                _assertKnownError(reason);
            }
            return;
        }

        vm.startPrank(actor);
        try factory.createToken(name, symbol, "USD", MagnusUSD, admin, salt) returns (
            address tokenAddr
        ) {
            vm.stopPrank();

            _totalTokensCreated++;
            _recordCreatedToken(actor, salt, tokenAddr);

            // MAGNUS-FAC1: Created address matches predicted address
            assertEq(
                tokenAddr,
                predictedAddr,
                "MAGNUS-FAC1: Created address does not match predicted address"
            );

            // MAGNUS-FAC2: Token is recognized as MIP20
            assertTrue(
                factory.isTIP20(tokenAddr), "MAGNUS-FAC2: Created token not recognized as MIP20"
            );

            // MAGNUS-FAC6: Token has correct properties
            MIP20 newToken = MIP20(tokenAddr);
            assertEq(
                keccak256(bytes(newToken.name())),
                keccak256(bytes(name)),
                "MAGNUS-FAC6: Token name mismatch"
            );
            assertEq(
                keccak256(bytes(newToken.symbol())),
                keccak256(bytes(symbol)),
                "MAGNUS-FAC6: Token symbol mismatch"
            );
            assertEq(
                keccak256(bytes(newToken.currency())),
                keccak256(bytes("USD")),
                "MAGNUS-FAC6: Token currency mismatch"
            );
        } catch (bytes memory reason) {
            vm.stopPrank();
            _assertKnownError(reason);
        }
    }

    /// @notice Handler for creating tokens with invalid quote token
    /// @dev Tests MAGNUS-FAC4 (quote token validation)
    function createTokenInvalidQuote(uint256 actorSeed, bytes32 salt) external {
        address actor = _selectActor(actorSeed);

        // Skip if salt is reserved or token already exists
        try factory.getTokenAddress(actor, salt) returns (address predictedAddr) {
            if (predictedAddr.code.length != 0) {
                return;
            }
        } catch (bytes memory reason) {
            if (bytes4(reason) == IMIP20Factory.AddressReserved.selector) {
                return;
            }
            revert("Unknown error in getTokenAddress");
        }

        // Use a non-MIP20 address as quote token
        address invalidQuote = makeAddr("InvalidQuote");

        vm.startPrank(actor);
        try factory.createToken("Test", "TST", "USD", IMIP20(invalidQuote), admin, salt) {
            vm.stopPrank();
            revert("MAGNUS-FAC4: Should revert for invalid quote token");
        } catch (bytes memory reason) {
            vm.stopPrank();
            // Must be InvalidQuoteToken since we filtered out reserved addresses and existing tokens
            assertEq(
                bytes4(reason),
                IMIP20Factory.InvalidQuoteToken.selector,
                "MAGNUS-FAC4: Expected InvalidQuoteToken error"
            );
            _totalInvalidQuoteAttempts++;
        }
    }

    /// @notice Handler for creating tokens with mismatched currency
    /// @dev Tests MAGNUS-FAC7 (currency/quote token consistency)
    function createTokenMismatchedCurrency(
        uint256 actorSeed,
        bytes32 salt,
        uint256 currencyIdx
    )
        external
    {
        address actor = _selectActor(actorSeed);

        // Skip if salt is reserved or token already exists
        try factory.getTokenAddress(actor, salt) returns (address predictedAddr) {
            if (predictedAddr.code.length != 0) {
                return;
            }
        } catch (bytes memory reason) {
            if (bytes4(reason) == IMIP20Factory.AddressReserved.selector) {
                return;
            }
            revert("Unknown error in getTokenAddress");
        }

        string memory currency = _generateNonUsdCurrency(currencyIdx);

        vm.startPrank(actor);
        try factory.createToken("Test", "TST", currency, MagnusUSD, admin, salt) returns (
            address tokenAddr
        ) {
            vm.stopPrank();

            if (tokenAddr != address(0)) {
                _totalNonUsdCurrencyCreated++;
                _recordCreatedToken(actor, salt, tokenAddr);

                MIP20 newToken = MIP20(tokenAddr);
                assertEq(
                    keccak256(bytes(newToken.currency())),
                    keccak256(bytes(currency)),
                    "MAGNUS-FAC7: Currency mismatch"
                );
            }
        } catch (bytes memory reason) {
            vm.stopPrank();
            _assertKnownError(reason);
        }
    }

    /// @notice Handler for attempting to create USD token with non-USD quote
    /// @dev Tests MAGNUS-FAC7 (USD tokens must have USD quote tokens)
    function createUsdTokenWithNonUsdQuote(uint256 actorSeed, bytes32 salt) external {
        address actor = _selectActor(actorSeed);

        bytes32 eurSalt = keccak256(abi.encode(salt, "EUR"));
        address eurToken;

        // Get or create a EUR token to use as quote
        try factory.getTokenAddress(actor, eurSalt) returns (address predictedEurAddr) {
            if (predictedEurAddr.code.length != 0) {
                // Verify the existing token is actually a EUR token (not some other token
                // that happened to be created at this address by another handler)
                if (keccak256(bytes(MIP20(predictedEurAddr).currency())) != keccak256(bytes("EUR")))
                {
                    // Token exists but is not EUR - skip this test case
                    return;
                }
                eurToken = predictedEurAddr;
            } else {
                vm.startPrank(actor);
                try factory.createToken(
                    "EUR Token", "EUR", "EUR", MagnusUSD, admin, eurSalt
                ) returns (
                    address addr
                ) {
                    eurToken = addr;
                    _recordCreatedToken(actor, eurSalt, addr);
                } catch (bytes memory reason) {
                    vm.stopPrank();
                    _assertKnownError(reason);
                    return;
                }
                vm.stopPrank();
            }
        } catch (bytes memory reason) {
            if (bytes4(reason) == IMIP20Factory.AddressReserved.selector) {
                return;
            }
            revert("Unknown error in getTokenAddress");
        }

        // Try to create a USD token with EUR quote - should fail
        bytes32 usdSalt = keccak256(abi.encode(salt, "USD_WITH_EUR"));

        try factory.getTokenAddress(actor, usdSalt) returns (address) { }
        catch (bytes memory reason) {
            if (bytes4(reason) == IMIP20Factory.AddressReserved.selector) {
                return;
            }
            revert("Unknown error in getTokenAddress");
        }

        vm.startPrank(actor);
        try factory.createToken("Bad USD", "BUSD", "USD", IMIP20(eurToken), admin, usdSalt) {
            vm.stopPrank();
            revert("MAGNUS-FAC7: USD token with non-USD quote should fail");
        } catch (bytes memory reason) {
            vm.stopPrank();
            // Accept either InvalidQuoteToken or TokenAlreadyExists since validation order
            // may vary between Solidity spec and Rust precompile. The precompile checks
            // TokenAlreadyExists before InvalidQuoteToken, so if the computed address
            // collides with an existing token, we get TokenAlreadyExists instead.
            bytes4 selector = bytes4(reason);
            bool isExpectedError = selector == IMIP20Factory.InvalidQuoteToken.selector
                || selector == IMIP20Factory.TokenAlreadyExists.selector;
            assertTrue(
                isExpectedError,
                "MAGNUS-FAC7: Should revert with InvalidQuoteToken or TokenAlreadyExists"
            );
            _totalUsdWithNonUsdQuoteRejected++;
        }
    }

    /// @notice Handler for testing reserved address enforcement on createToken
    /// @dev Tests MAGNUS-FAC5 (reserved address enforcement on createToken, not just getTokenAddress)
    function createTokenReservedAddress(uint256 actorSeed, bytes32 salt) external {
        address actor = _selectActor(actorSeed);

        // Only proceed if salt produces a reserved address
        try factory.getTokenAddress(actor, salt) returns (address) {
            return;
        } catch (bytes memory reason) {
            if (bytes4(reason) != IMIP20Factory.AddressReserved.selector) {
                revert("Unknown error in getTokenAddress");
            }
        }

        vm.startPrank(actor);
        try factory.createToken("Reserved", "RES", "USD", MagnusUSD, admin, salt) {
            vm.stopPrank();
            revert("MAGNUS-FAC5: Should revert for reserved address on createToken");
        } catch (bytes memory reason) {
            vm.stopPrank();
            assertEq(
                bytes4(reason),
                IMIP20Factory.AddressReserved.selector,
                "MAGNUS-FAC5: createToken should revert with AddressReserved"
            );
            _totalReservedCreateAttempts++;
        }
    }

    /// @notice Handler for verifying isTIP20 on controlled addresses
    /// @dev Tests MAGNUS-FAC8 (isTIP20 consistency)
    function checkIsTIP20(uint256 addrSeed) external {
        _totalIsTIP20Checks++;

        if (addrSeed % 4 == 0 && _createdTokens.length > 0) {
            // Check a created token - must be MIP20
            address checkAddr = _createdTokens[addrSeed % _createdTokens.length];
            assertTrue(factory.isTIP20(checkAddr), "MAGNUS-FAC8: Created token should be MIP20");
        } else if (addrSeed % 4 == 1) {
            // Check MagnusUSD (known MIP20)
            assertTrue(factory.isTIP20(address(MagnusUSD)), "MAGNUS-FAC8: MagnusUSD should be MIP20");
        } else if (addrSeed % 4 == 2) {
            // Check factory address - should NOT be MIP20
            assertFalse(
                factory.isTIP20(address(factory)), "MAGNUS-FAC8: Factory should not be MIP20"
            );
            // Check AMM address - should NOT be MIP20
            assertFalse(factory.isTIP20(address(amm)), "MAGNUS-FAC8: AMM should not be MIP20");
        } else {
            // Check a random address - exclude known MIP20s and reserved range
            address checkAddr = address(uint160(addrSeed));

            // Skip addresses in the reserved MIP20 range (prefix 0x20C0... with lower 64 bits < 1024)
            // These addresses may have code from genesis/hardfork deployments
            bool hasPrefix = bytes12(bytes20(checkAddr)) == bytes12(0x20c000000000000000000000);
            uint64 lowerBytes = uint64(uint160(checkAddr));
            bool isReserved = hasPrefix && lowerBytes < 1024;

            if (
                !_isCreatedToken[checkAddr] && checkAddr != address(MagnusUSD)
                    && checkAddr != address(token1) && checkAddr != address(token2)
                    && checkAddr != address(token3) && checkAddr != address(token4) && !isReserved
            ) {
                assertFalse(
                    factory.isTIP20(checkAddr), "MAGNUS-FAC8: Random address should not be MIP20"
                );
            }
        }
    }

    /// @notice Handler for verifying getTokenAddress determinism
    /// @dev Tests MAGNUS-FAC9 (address prediction is deterministic), MAGNUS-FAC10 (sender differentiation)
    function verifyAddressDeterminism(uint256 actorSeed, bytes32 salt) external view {
        address actor = _selectActor(actorSeed);
        address otherActor = _selectActorExcluding(actorSeed, actor);

        try factory.getTokenAddress(actor, salt) returns (address addr1) {
            // MAGNUS-FAC9: Same inputs always produce same output
            address addr2 = factory.getTokenAddress(actor, salt);
            assertEq(addr1, addr2, "MAGNUS-FAC9: getTokenAddress not deterministic");

            // MAGNUS-FAC10: Different senders produce different addresses
            try factory.getTokenAddress(otherActor, salt) returns (address otherAddr) {
                assertTrue(
                    addr1 != otherAddr,
                    "MAGNUS-FAC10: Different senders should produce different addresses"
                );
            } catch (bytes memory reason) {
                // Other actor's salt might be reserved - that's OK
                if (bytes4(reason) != IMIP20Factory.AddressReserved.selector) {
                    _assertKnownError(reason);
                }
            }
        } catch (bytes memory reason) {
            // Actor's salt might be reserved - that's OK
            if (bytes4(reason) != IMIP20Factory.AddressReserved.selector) {
                _assertKnownError(reason);
            }
        }
    }

    /*//////////////////////////////////////////////////////////////
                         GLOBAL INVARIANTS
    //////////////////////////////////////////////////////////////*/

    /// @notice Lightweight global invariant - most checks done inline in handlers
    /// @dev FAC1 verified at creation time, FAC2/FAC11/FAC12 verified inline
    ///      FAC8 system contract checks in setUp() as they're immutable
    ///      This function uses sampling to avoid O(n) on every call
    function invariant_globalInvariants() public view {
        // Only sample-check if we have created tokens
        if (_createdTokens.length == 0) return;

        // Sample up to 3 tokens per call using block.number for variation
        uint256 sampleCount = _createdTokens.length < 3 ? _createdTokens.length : 3;
        bytes32 usdHash = keccak256(bytes("USD"));

        for (uint256 i = 0; i < sampleCount; i++) {
            uint256 idx = (block.number + i) % _createdTokens.length;
            address tokenAddr = _createdTokens[idx];
            MIP20 token = MIP20(tokenAddr);

            // MAGNUS-FAC2: Created token is recognized as MIP20
            assertTrue(
                factory.isTIP20(tokenAddr), "MAGNUS-FAC2: Created token not recognized as MIP20"
            );

            // MAGNUS-FAC11: Token address has correct prefix
            uint160 addrValue = uint160(tokenAddr);
            uint96 prefix = uint96(addrValue >> 64);
            assertEq(
                prefix,
                0x20C000000000000000000000,
                "MAGNUS-FAC11: Token address has incorrect prefix"
            );

            // MAGNUS-FAC12 (reverse): Given a token address, verify the salt/sender that produced it
            {
                address sender = _tokenToSender[tokenAddr];
                bytes32 salt = _tokenToSalt[tokenAddr];
                assertTrue(sender != address(0), "MAGNUS-FAC12: Missing sender ghost state");
                assertEq(
                    factory.getTokenAddress(sender, salt),
                    tokenAddr,
                    "MAGNUS-FAC12: Reverse invariant - token address does not match (sender, salt)"
                );
                bytes32 uniqueKey = keccak256(abi.encode(sender, salt));
                assertEq(
                    _saltToToken[uniqueKey],
                    tokenAddr,
                    "MAGNUS-FAC12: Ghost maps inconsistent (forward vs reverse)"
                );
            }

            // MAGNUS-FAC12: USD tokens must have USD quote tokens
            if (keccak256(bytes(token.currency())) == usdHash) {
                IMIP20 quote = token.quoteToken();
                if (address(quote) != address(0)) {
                    assertEq(
                        keccak256(bytes(MIP20(address(quote)).currency())),
                        usdHash,
                        "MAGNUS-FAC12: USD token has non-USD quote token"
                    );
                }
            }
        }
    }

    /*//////////////////////////////////////////////////////////////
                            HELPERS
    //////////////////////////////////////////////////////////////*/

    /// @dev Records a newly created token in ghost state and verifies invariants inline
    /// @param actor The actor who created the token
    /// @param salt The salt used for creation
    /// @param tokenAddr The address of the created token
    function _recordCreatedToken(address actor, bytes32 salt, address tokenAddr) internal {
        // Defensive: ensure we're not recording duplicates
        assertFalse(_isCreatedToken[tokenAddr], "MAGNUS-FAC3: Duplicate token address detected");

        bytes32 uniqueKey = keccak256(abi.encode(actor, salt));
        assertEq(
            _saltToToken[uniqueKey], address(0), "Ghost state: salt already used for this actor"
        );

        // MAGNUS-FAC1: Verify salt-to-token mapping consistency immediately
        address factoryAddr = factory.getTokenAddress(actor, salt);
        assertEq(tokenAddr, factoryAddr, "MAGNUS-FAC1: Created address inconsistent with factory");

        // MAGNUS-FAC11: Verify token address has correct prefix
        uint160 addrValue = uint160(tokenAddr);
        uint96 prefix = uint96(addrValue >> 64);
        assertEq(
            prefix, 0x20C000000000000000000000, "MAGNUS-FAC11: Token address has incorrect prefix"
        );

        _createdTokens.push(tokenAddr);
        _isCreatedToken[tokenAddr] = true;
        _saltToToken[uniqueKey] = tokenAddr;
        _tokenToSalt[tokenAddr] = salt;
        _tokenToSender[tokenAddr] = actor;
        _senderSalts[actor].push(salt);
    }

    /// @dev Generates a token name based on index
    function _generateName(uint256 idx) internal pure returns (string memory) {
        string[5] memory names =
            ["Token Alpha", "Token Beta", "Token Gamma", "Token Delta", "Token Epsilon"];
        return names[idx % names.length];
    }

    /// @dev Generates a token symbol based on index
    function _generateSymbol(uint256 idx) internal pure returns (string memory) {
        string[5] memory symbols = ["TALP", "TBET", "TGAM", "TDEL", "TEPS"];
        return symbols[idx % symbols.length];
    }

    /// @dev Generates a non-USD currency based on index
    function _generateNonUsdCurrency(uint256 idx) internal pure returns (string memory) {
        string[4] memory currencies = ["EUR", "GBP", "JPY", "CHF"];
        return currencies[idx % currencies.length];
    }

    /// @dev Checks if an error is known/expected
    /// @dev Only accepts known custom error selectors - Panic and Error(string) should fail
    ///      the test as they may indicate bugs in the factory implementation
    function _assertKnownError(bytes memory reason) internal pure {
        bytes4 selector = bytes4(reason);
        bool isKnown = selector == IMIP20Factory.AddressReserved.selector
            || selector == IMIP20Factory.InvalidQuoteToken.selector
            || selector == IMIP20Factory.TokenAlreadyExists.selector;
        assertTrue(isKnown, "Unknown error encountered");
    }

}
