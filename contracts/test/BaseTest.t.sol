// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import { AccountKeychain } from "../src/AccountKeychain.sol";
import { FeeManager } from "../src/FeeManager.sol";
import { Nonce } from "../src/Nonce.sol";
import { StablecoinDEX } from "../src/StablecoinDEX.sol";
import { MIP20 } from "../src/MIP20.sol";
import { MIP20Factory } from "../src/MIP20Factory.sol";
import { MIP403Registry } from "../src/MIP403Registry.sol";
import { IAccountKeychain } from "../src/interfaces/IAccountKeychain.sol";
import { INonce } from "../src/interfaces/INonce.sol";
import { IMIP20 } from "../src/interfaces/IMIP20.sol";
import { IValidatorConfig } from "../src/interfaces/IValidatorConfig.sol";
import { Test, console } from "forge-std/Test.sol";

/// @notice Base test framework for all spec tests
/// pathUSD is just a MIP20 at a special address (0x20C0...) with token_id=0
contract BaseTest is Test {

    // Registry precompiles
    address internal constant _ACCOUNT_KEYCHAIN = 0xaAAAaaAA00000000000000000000000000000000;
    address internal constant _MIP403REGISTRY = 0x403c000000000000000000000000000000000000;
    address internal constant _MIP20FACTORY = 0x20Fc000000000000000000000000000000000000;
    address internal constant _PATH_USD = 0x20C0000000000000000000000000000000000000;
    address internal constant _STABLECOIN_DEX = 0xDEc0000000000000000000000000000000000000;
    address internal constant _FEE_AMM = 0xfeEC000000000000000000000000000000000000;
    address internal constant _NONCE = 0x4e4F4E4345000000000000000000000000000000;
    address internal constant _VALIDATOR_CONFIG = 0xCccCcCCC00000000000000000000000000000000;

    // Role constants
    bytes32 internal constant _ISSUER_ROLE = keccak256("ISSUER_ROLE");
    bytes32 internal constant _PAUSE_ROLE = keccak256("PAUSE_ROLE");
    bytes32 internal constant _UNPAUSE_ROLE = keccak256("UNPAUSE_ROLE");
    bytes32 internal constant _TRANSFER_ROLE = keccak256("TRANSFER_ROLE");
    bytes32 internal constant _RECEIVE_WITH_MEMO_ROLE = keccak256("RECEIVE_WITH_MEMO_ROLE");

    // Common test addresses
    address public admin = address(this);
    address public alice = address(0x200);
    address public bob = address(0x300);
    address public charlie = address(0x400);
    address public pathUSDAdmin = address(0xb4c79daB8f259C7Aee6E5b2Aa729821864227e84);

    // Common test contracts
    IAccountKeychain public keychain = IAccountKeychain(_ACCOUNT_KEYCHAIN);
    MIP20Factory public factory = MIP20Factory(_MIP20FACTORY);
    MIP20 public pathUSD = MIP20(_PATH_USD); // pathUSD is just a MIP20 at token_id=0
    StablecoinDEX public exchange = StablecoinDEX(_STABLECOIN_DEX);
    FeeManager public amm = FeeManager(_FEE_AMM);
    MIP403Registry public registry = MIP403Registry(_MIP403REGISTRY);
    INonce public nonce = INonce(_NONCE);
    IValidatorConfig public validatorConfig = IValidatorConfig(_VALIDATOR_CONFIG);
    MIP20 public token1;
    MIP20 public token2;
    bool isMagnus;

    error MissingPrecompile(string name, address addr);
    error CallShouldHaveReverted();

    function setUp() public virtual {
        // Is this tempo chain?
        isMagnus = _MIP403REGISTRY.code.length + _MIP20FACTORY.code.length + _PATH_USD.code.length
                + _STABLECOIN_DEX.code.length + _NONCE.code.length + _ACCOUNT_KEYCHAIN.code.length
            > 0;

        console.log("Tests running with isMagnus =", isMagnus);

        // Deploy contracts if not tempo
        if (!isMagnus) {
            deployCodeTo("AccountKeychain", _ACCOUNT_KEYCHAIN);
            deployCodeTo("MIP403Registry", _MIP403REGISTRY);
            deployCodeTo("StablecoinDEX", _STABLECOIN_DEX);
            deployCodeTo("FeeManager", _FEE_AMM);
            deployCodeTo("MIP20Factory", _MIP20FACTORY);
            // Deploy pathUSD as a MIP20 at the special address
            deployCodeTo(
                "MIP20.sol",
                abi.encode("pathUSD", "pathUSD", "USD", address(0), pathUSDAdmin),
                _PATH_USD
            );
            deployCodeTo("Nonce", _NONCE);
            // Deploy ValidatorConfig with admin as owner
            deployCodeTo("ValidatorConfig.sol", abi.encode(admin), _VALIDATOR_CONFIG);
        }

        if (isMagnus) {
            if (_ACCOUNT_KEYCHAIN.code.length == 0) {
                revert MissingPrecompile("AccountKeychain", _ACCOUNT_KEYCHAIN);
            }
            if (_MIP403REGISTRY.code.length == 0) {
                revert MissingPrecompile("MIP403Registry", _MIP403REGISTRY);
            }
            if (_MIP20FACTORY.code.length == 0) {
                revert MissingPrecompile("MIP20Factory", _MIP20FACTORY);
            }
            if (_PATH_USD.code.length == 0) {
                revert MissingPrecompile("pathUSD", _PATH_USD);
            }
            if (_STABLECOIN_DEX.code.length == 0) {
                revert MissingPrecompile("StablecoinDEX", _STABLECOIN_DEX);
            }
            if (_FEE_AMM.code.length == 0) {
                revert MissingPrecompile("FeeManager", _STABLECOIN_DEX);
            }
            if (_NONCE.code.length == 0) {
                revert MissingPrecompile("Nonce", _NONCE);
            }
            if (_VALIDATOR_CONFIG.code.length == 0) {
                revert MissingPrecompile("ValidatorConfig", _VALIDATOR_CONFIG);
            }

            // Set ValidatorConfig owner to admin via direct storage write
            // owner is at slot 0 in ValidatorConfig
            vm.store(_VALIDATOR_CONFIG, bytes32(uint256(0)), bytes32(uint256(uint160(admin))));

            // Grant DEFAULT_ADMIN_ROLE to admin for pathUSD via direct storage write
            bytes32 adminRoleSlot = keccak256(
                abi.encode(
                    bytes32(0), // DEFAULT_ADMIN_ROLE
                    keccak256(abi.encode(admin, uint256(0)))
                )
            );
            vm.store(_PATH_USD, adminRoleSlot, bytes32(uint256(1)));

            // Grant DEFAULT_ADMIN_ROLE to pathUSDAdmin
            bytes32 tempoAdminRoleSlot = keccak256(
                abi.encode(
                    bytes32(0), // DEFAULT_ADMIN_ROLE
                    keccak256(abi.encode(pathUSDAdmin, uint256(0)))
                )
            );
            vm.store(_PATH_USD, tempoAdminRoleSlot, bytes32(uint256(1)));
        }

        token1 =
            MIP20(factory.createToken("TOKEN1", "T1", "USD", pathUSD, admin, bytes32("token1")));
        token2 =
            MIP20(factory.createToken("TOKEN2", "T2", "USD", pathUSD, admin, bytes32("token2")));
    }

}
