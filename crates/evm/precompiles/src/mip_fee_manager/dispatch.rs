//! ABI dispatch for the [`MipFeeManager`] precompile.

use crate::{
    Precompile, charge_input_cost, dispatch_call, mip_fee_manager::MipFeeManager, mutate,
    mutate_void, storage::Handler, view,
};
use alloy::{primitives::Address, sol_types::SolInterface};
use magnus_contracts::precompiles::IFeeManager::IFeeManagerCalls;
use revm::precompile::PrecompileResult;

impl Precompile for MipFeeManager {
    fn call(&mut self, calldata: &[u8], msg_sender: Address) -> PrecompileResult {
        if let Some(err) = charge_input_cost(&mut self.storage, calldata) {
            return err;
        }

        dispatch_call(
            calldata,
            &[],
            IFeeManagerCalls::abi_decode,
            |call| match call {
                // Fee distribution
                IFeeManagerCalls::collectedFees(call) => {
                    view(call, |c| self.collected_fees[c.validator][c.token].read())
                }
                IFeeManagerCalls::distributeFees(call) => {
                    mutate_void(call, msg_sender, |_, c| {
                        self.distribute_fees(c.validator, c.token)
                    })
                }

                // Currency registry
                IFeeManagerCalls::governanceAdmin(call) => {
                    view(call, |_| self.governance_admin())
                }
                IFeeManagerCalls::getCurrencyConfig(call) => view(call, |c| {
                    let config = self.get_currency_config(&c.code)?;
                    Ok(magnus_contracts::precompiles::IFeeManager::CurrencyConfig {
                        registered: config.registered,
                        enabled: config.enabled,
                        deprecating: config.deprecating,
                        addedAtBlock: config.added_at_block,
                        enabledAtBlock: config.enabled_at_block,
                        deprecationActivatesAt: config.deprecation_activates_at,
                        lastPrunedAtBlock: config.last_pruned_at_block,
                    })
                }),
                IFeeManagerCalls::isCurrencyEnabled(call) => {
                    view(call, |c| self.is_currency_enabled(&c.code))
                }
                IFeeManagerCalls::deprecationGracePeriod(call) => {
                    view(call, |_| self.deprecation_grace_period())
                }
                IFeeManagerCalls::emergencyDisableThreshold(call) => {
                    view(call, |_| self.emergency_disable_threshold())
                }
                IFeeManagerCalls::addCurrency(call) => mutate_void(call, msg_sender, |s, c| {
                    let block = self.storage.block_number();
                    self.add_currency(s, &c.code, block)
                }),
                IFeeManagerCalls::enableCurrency(call) => mutate_void(call, msg_sender, |s, c| {
                    let block = self.storage.block_number();
                    self.enable_currency(s, &c.code, block)
                }),
                IFeeManagerCalls::disableCurrency(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        let now_ts = self.storage.timestamp().saturating_to::<u64>();
                        self.disable_currency(s, &c.code, now_ts)
                    })
                }
                IFeeManagerCalls::emergencyDisableCurrency(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        self.emergency_disable_currency(s, &c.code)
                    })
                }
                IFeeManagerCalls::pruneCurrency(call) => mutate_void(call, msg_sender, |_, c| {
                    let block = self.storage.block_number();
                    self.prune_currency(&c.code, c.maxIterations.saturating_to::<u64>(), block)
                }),
                IFeeManagerCalls::pruneToken(call) => mutate(call, msg_sender, |_, c| {
                    let block = self.storage.block_number();
                    let removed = self.prune_token(
                        c.token,
                        c.maxIterations.saturating_to::<u64>(),
                        block,
                    )?;
                    Ok(alloy::primitives::U256::from(removed))
                }),
                IFeeManagerCalls::setDeprecationGracePeriod(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        self.set_deprecation_grace_period(s, c.newGracePeriod)
                    })
                }
                IFeeManagerCalls::setEmergencyDisableThreshold(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        self.set_emergency_disable_threshold(s, c.newThreshold)
                    })
                }
                IFeeManagerCalls::setGovernanceAdmin(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        self.set_governance_admin(s, c.newAdmin)
                    })
                }

                // Validator accept-set
                IFeeManagerCalls::addAcceptedToken(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        let beneficiary = self.storage.beneficiary();
                        self.add_accepted_token(s, c.token, beneficiary)
                    })
                }
                IFeeManagerCalls::removeAcceptedToken(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        let beneficiary = self.storage.beneficiary();
                        self.remove_accepted_token(s, c.token, beneficiary)
                    })
                }
                IFeeManagerCalls::acceptsToken(call) => {
                    view(call, |c| self.accepts_token(c.validator, c.token))
                }
                IFeeManagerCalls::getAcceptedTokens(call) => {
                    view(call, |c| self.get_accepted_tokens(c.validator))
                }
                IFeeManagerCalls::isAcceptedByAnyValidator(call) => {
                    view(call, |c| self.is_accepted_by_any_validator(c.token))
                }

                // Off-boarding + escrow
                IFeeManagerCalls::offboardValidator(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        self.offboard_validator(s, c.validator)
                    })
                }
                IFeeManagerCalls::claimEscrowedFees(call) => mutate(call, msg_sender, |s, c| {
                    self.claim_escrowed_fees(s, c.validator, c.token, c.recipient)
                }),
                IFeeManagerCalls::sweepExpiredEscrow(call) => mutate(call, msg_sender, |s, c| {
                    self.sweep_expired_escrow(s, c.validator, c.token)
                }),
                IFeeManagerCalls::setEscrowClaimWindow(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        self.set_escrow_claim_window(s, c.newWindow)
                    })
                }
                IFeeManagerCalls::setFoundationEscrowAddress(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        self.set_foundation_escrow_address(s, c.newAddress)
                    })
                }
                IFeeManagerCalls::escrowedFees(call) => {
                    view(call, |c| self.escrowed_fees_amount(c.validator, c.token))
                }
                IFeeManagerCalls::escrowClaim(call) => view(call, |c| {
                    let r = self.escrow_claim(c.validator)?;
                    Ok((r.offboarded_at, r.claim_deadline, r.offboarded).into())
                }),
                IFeeManagerCalls::escrowClaimWindow(call) => {
                    view(call, |_| self.escrow_claim_window())
                }
                IFeeManagerCalls::foundationEscrowAddress(call) => {
                    view(call, |_| self.foundation_escrow_address())
                }

                // Router selector registry
                IFeeManagerCalls::registerRouterSelector(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        self.register_router_selector(
                            s,
                            c.router,
                            c.selector,
                            c.tokenInputArgIndex,
                        )
                    })
                }
                IFeeManagerCalls::unregisterRouterSelector(call) => {
                    mutate_void(call, msg_sender, |s, c| {
                        self.unregister_router_selector(s, c.router, c.selector)
                    })
                }
                IFeeManagerCalls::lookupRouterSelector(call) => view(call, |c| {
                    let (registered, arg_index) =
                        self.lookup_router_selector(c.router, c.selector)?;
                    Ok((registered, arg_index).into())
                }),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        storage::{StorageCtx, hashmap::HashMapStorageProvider},
        test_util::{assert_full_coverage, check_selector_coverage},
    };
    use magnus_contracts::precompiles::IFeeManager::IFeeManagerCalls;

    #[test]
    fn test_tip_fee_manager_selector_coverage() -> eyre::Result<()> {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut fee_manager = MipFeeManager::new();

            let unsupported = check_selector_coverage(
                &mut fee_manager,
                IFeeManagerCalls::SELECTORS,
                "IFeeManager",
                IFeeManagerCalls::name_by_selector,
            );

            assert_full_coverage([unsupported]);

            Ok(())
        })
    }
}
