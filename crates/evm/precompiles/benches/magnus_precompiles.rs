use alloy::primitives::{Address, FixedBytes, U256};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use magnus_precompiles::{
    storage::{StorageCtx, hashmap::HashMapStorageProvider},
    test_util::MIP20Setup,
    mip20::{ISSUER_ROLE, IMIP20, PAUSE_ROLE, UNPAUSE_ROLE},
    mip403_registry::{AuthRole, IMIP403Registry, MIP403Registry},
};

fn mip20_metadata(c: &mut Criterion) {
    c.bench_function("mip20_name", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();

            b.iter(|| {
                let token = black_box(&mut token);
                let result = token.name().unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_symbol", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();

            b.iter(|| {
                let token = black_box(&mut token);
                let result = token.symbol().unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_decimals", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();

            b.iter(|| {
                let token = black_box(&mut token);
                let result = token.decimals().unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_currency", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();

            b.iter(|| {
                let token = black_box(&mut token);
                let result = token.currency().unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_total_supply", |b| {
        let admin = Address::from([0u8; 20]);
        let user = Address::from([1u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let _ = token.grant_role_internal(admin, *ISSUER_ROLE);
            token
                .mint(
                    admin,
                    IMIP20::mintCall {
                        to: user,
                        amount: U256::from(1000),
                    },
                )
                .unwrap();

            b.iter(|| {
                let token = black_box(&mut token);
                let result = token.total_supply().unwrap();
                black_box(result);
            });
        });
    });
}

fn mip20_view(c: &mut Criterion) {
    c.bench_function("mip20_balance_of", |b| {
        let admin = Address::from([0u8; 20]);
        let user = Address::from([1u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let _ = token.grant_role_internal(admin, *ISSUER_ROLE);
            token
                .mint(
                    admin,
                    IMIP20::mintCall {
                        to: user,
                        amount: U256::from(1000),
                    },
                )
                .unwrap();

            b.iter(|| {
                let token = black_box(&mut token);
                let call = black_box(IMIP20::balanceOfCall { account: user });
                let result = token.balance_of(call).unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_allowance", |b| {
        let admin = Address::from([0u8; 20]);
        let owner = Address::from([1u8; 20]);
        let spender = Address::from([2u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            token
                .approve(
                    owner,
                    IMIP20::approveCall {
                        spender,
                        amount: U256::from(500),
                    },
                )
                .unwrap();

            b.iter(|| {
                let token = black_box(&mut token);
                let call = black_box(IMIP20::allowanceCall { owner, spender });
                let result = token.allowance(call).unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_supply_cap", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();

            b.iter(|| {
                let token = black_box(&mut token);
                let result = token.supply_cap().unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_paused", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();

            b.iter(|| {
                let token = black_box(&mut token);
                let result = token.paused().unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_transfer_policy_id", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();

            b.iter(|| {
                let token = black_box(&mut token);
                let result = token.transfer_policy_id().unwrap();
                black_box(result);
            });
        });
    });
}

fn mip20_mutate(c: &mut Criterion) {
    c.bench_function("mip20_mint", |b| {
        let admin = Address::from([0u8; 20]);
        let user = Address::from([1u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let _ = token.grant_role_internal(admin, *ISSUER_ROLE);

            let amount = U256::from(100);
            b.iter(|| {
                let token = black_box(&mut token);
                let admin = black_box(admin);
                let call = black_box(IMIP20::mintCall { to: user, amount });
                token.mint(admin, call).unwrap();
            });
        });
    });

    c.bench_function("mip20_burn", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let _ = token.grant_role_internal(admin, *ISSUER_ROLE);
            // Pre-mint tokens for burning
            token
                .mint(
                    admin,
                    IMIP20::mintCall {
                        to: admin,
                        amount: U256::from(u128::MAX),
                    },
                )
                .unwrap();

            let amount = U256::ONE;
            b.iter(|| {
                let token = black_box(&mut token);
                let admin = black_box(admin);
                let call = black_box(IMIP20::burnCall { amount });
                token.burn(admin, call).unwrap();
            });
        });
    });

    c.bench_function("mip20_approve", |b| {
        let admin = Address::from([0u8; 20]);
        let owner = Address::from([1u8; 20]);
        let spender = Address::from([2u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();

            let amount = U256::from(500);
            b.iter(|| {
                let token = black_box(&mut token);
                let owner = black_box(owner);
                let call = black_box(IMIP20::approveCall { spender, amount });
                let result = token.approve(owner, call).unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_transfer", |b| {
        let admin = Address::from([0u8; 20]);
        let from = Address::from([1u8; 20]);
        let to = Address::from([2u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let _ = token.grant_role_internal(admin, *ISSUER_ROLE);
            // Pre-mint tokens for transfers
            token
                .mint(
                    admin,
                    IMIP20::mintCall {
                        to: from,
                        amount: U256::from(u128::MAX),
                    },
                )
                .unwrap();

            let amount = U256::ONE;
            b.iter(|| {
                let token = black_box(&mut token);
                let from = black_box(from);
                let call = black_box(IMIP20::transferCall { to, amount });
                let result = token.transfer(from, call).unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_transfer_from", |b| {
        let admin = Address::from([0u8; 20]);
        let owner = Address::from([1u8; 20]);
        let spender = Address::from([2u8; 20]);
        let recipient = Address::from([3u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let _ = token.grant_role_internal(admin, *ISSUER_ROLE);
            // Pre-mint tokens and set allowance
            token
                .mint(
                    admin,
                    IMIP20::mintCall {
                        to: owner,
                        amount: U256::from(u128::MAX),
                    },
                )
                .unwrap();
            token
                .approve(
                    owner,
                    IMIP20::approveCall {
                        spender,
                        amount: U256::from(u128::MAX),
                    },
                )
                .unwrap();

            let amount = U256::ONE;

            b.iter(|| {
                let token = black_box(&mut token);
                let spender = black_box(spender);
                let call = black_box(IMIP20::transferFromCall {
                    from: owner,
                    to: recipient,
                    amount,
                });
                let result = token.transfer_from(spender, call).unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip20_transfer_with_memo", |b| {
        let admin = Address::from([0u8; 20]);
        let from = Address::from([1u8; 20]);
        let to = Address::from([2u8; 20]);
        let memo = FixedBytes::<32>::random();
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let _ = token.grant_role_internal(admin, *ISSUER_ROLE);
            // Pre-mint tokens for transfers
            token
                .mint(
                    admin,
                    IMIP20::mintCall {
                        to: from,
                        amount: U256::from(u128::MAX),
                    },
                )
                .unwrap();

            let amount = U256::ONE;
            b.iter(|| {
                let token = black_box(&mut token);
                let from = black_box(from);
                let call = black_box(IMIP20::transferWithMemoCall { to, amount, memo });
                token.transfer_with_memo(from, call).unwrap();
            });
        });
    });

    c.bench_function("mip20_pause", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let _ = token.grant_role_internal(admin, *PAUSE_ROLE);

            b.iter(|| {
                let token = black_box(&mut token);
                let admin = black_box(admin);
                let call = black_box(IMIP20::pauseCall {});
                token.pause(admin, call).unwrap();
            });
        });
    });

    c.bench_function("mip20_unpause", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let _ = token.grant_role_internal(admin, *UNPAUSE_ROLE);

            b.iter(|| {
                let token = black_box(&mut token);
                let admin = black_box(admin);
                let call = black_box(IMIP20::unpauseCall {});
                token.unpause(admin, call).unwrap();
            });
        });
    });

    c.bench_function("mip20_set_supply_cap", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let counter = U256::from(10000);

            b.iter(|| {
                let token = black_box(&mut token);
                let admin = black_box(admin);
                let call = black_box(IMIP20::setSupplyCapCall {
                    newSupplyCap: counter,
                });
                token.set_supply_cap(admin, call).unwrap();
            });
        });
    });

    c.bench_function("mip20_change_transfer_policy_id", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut token = MIP20Setup::create("TestToken", "TEST", admin)
                .apply()
                .unwrap();
            let policy_id = 2;

            b.iter(|| {
                let token = black_box(&mut token);
                let admin = black_box(admin);
                let call = black_box(IMIP20::changeTransferPolicyIdCall {
                    newPolicyId: policy_id,
                });
                token.change_transfer_policy_id(admin, call).unwrap();
            });
        });
    });
}

fn mip20_factory_mutate(c: &mut Criterion) {
    c.bench_function("mip20_factory_create_token", |b| {
        let sender = Address::from([1u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            // Setup MagnusUSD first
            MIP20Setup::magnus_usd(sender).apply().unwrap();
            let mut counter = 0u64;

            b.iter(|| {
                counter += 1;
                let result = MIP20Setup::create("Test", "TEST", sender)
                    .with_salt(FixedBytes::from(U256::from(counter)))
                    .apply()
                    .unwrap();
                black_box(result);
            });
        });
    });
}

fn mip403_registry_view(c: &mut Criterion) {
    c.bench_function("mip403_registry_policy_id_counter", |b| {
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            b.iter(|| {
                let registry = black_box(&mut registry);
                let result = registry.policy_id_counter().unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip403_registry_policy_data", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();
            let policy_id = registry
                .create_policy(
                    admin,
                    IMIP403Registry::createPolicyCall {
                        admin,
                        policyType: IMIP403Registry::PolicyType::WHITELIST,
                    },
                )
                .unwrap();

            b.iter(|| {
                let registry = black_box(&mut registry);
                let call = black_box(IMIP403Registry::policyDataCall {
                    policyId: policy_id,
                });
                let result = registry.policy_data(call).unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip403_registry_is_authorized", |b| {
        let admin = Address::from([0u8; 20]);
        let user = Address::from([1u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();
            let policy_id = registry
                .create_policy(
                    admin,
                    IMIP403Registry::createPolicyCall {
                        admin,
                        policyType: IMIP403Registry::PolicyType::WHITELIST,
                    },
                )
                .unwrap();

            b.iter(|| {
                let registry = black_box(&mut registry);
                let policy_id = black_box(policy_id);
                let user = black_box(user);
                let result = registry
                    .is_authorized_as(policy_id, user, AuthRole::Transfer)
                    .unwrap();
                black_box(result);
            });
        });
    });
}

fn mip403_registry_mutate(c: &mut Criterion) {
    c.bench_function("mip403_registry_create_policy", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            b.iter(|| {
                let registry = black_box(&mut registry);
                let admin = black_box(admin);
                let call = black_box(IMIP403Registry::createPolicyCall {
                    admin,
                    policyType: IMIP403Registry::PolicyType::WHITELIST,
                });
                let result = registry.create_policy(admin, call).unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip403_registry_create_policy_with_accounts", |b| {
        let admin = Address::from([0u8; 20]);
        let account1 = Address::from([1u8; 20]);
        let account2 = Address::from([2u8; 20]);
        let accounts = vec![account1, account2];
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();

            b.iter(|| {
                let registry = black_box(&mut registry);
                let admin = black_box(admin);
                let call = black_box(IMIP403Registry::createPolicyWithAccountsCall {
                    admin,
                    policyType: IMIP403Registry::PolicyType::WHITELIST,
                    accounts: accounts.clone(),
                });
                let result = registry.create_policy_with_accounts(admin, call).unwrap();
                black_box(result);
            });
        });
    });

    c.bench_function("mip403_registry_set_policy_admin", |b| {
        let admin = Address::from([0u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();
            let policy_id = registry
                .create_policy(
                    admin,
                    IMIP403Registry::createPolicyCall {
                        admin,
                        policyType: IMIP403Registry::PolicyType::WHITELIST,
                    },
                )
                .unwrap();

            b.iter(|| {
                let registry = black_box(&mut registry);
                let admin = black_box(admin);
                let call = black_box(IMIP403Registry::setPolicyAdminCall {
                    policyId: policy_id,
                    admin,
                });
                registry.set_policy_admin(admin, call).unwrap();
            });
        });
    });

    c.bench_function("mip403_registry_modify_policy_whitelist", |b| {
        let admin = Address::from([0u8; 20]);
        let user = Address::from([1u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();
            let policy_id = registry
                .create_policy(
                    admin,
                    IMIP403Registry::createPolicyCall {
                        admin,
                        policyType: IMIP403Registry::PolicyType::WHITELIST,
                    },
                )
                .unwrap();

            b.iter(|| {
                let registry = black_box(&mut registry);
                let admin = black_box(admin);
                let call = black_box(IMIP403Registry::modifyPolicyWhitelistCall {
                    policyId: policy_id,
                    account: user,
                    allowed: true,
                });
                registry.modify_policy_whitelist(admin, call).unwrap();
            });
        });
    });

    c.bench_function("mip403_registry_modify_policy_blacklist", |b| {
        let admin = Address::from([0u8; 20]);
        let user = Address::from([1u8; 20]);
        let mut storage = HashMapStorageProvider::new(1);
        StorageCtx::enter(&mut storage, || {
            let mut registry = MIP403Registry::new();
            let policy_id = registry
                .create_policy(
                    admin,
                    IMIP403Registry::createPolicyCall {
                        admin,
                        policyType: IMIP403Registry::PolicyType::BLACKLIST,
                    },
                )
                .unwrap();

            b.iter(|| {
                let registry = black_box(&mut registry);
                let admin = black_box(admin);
                let call = black_box(IMIP403Registry::modifyPolicyBlacklistCall {
                    policyId: policy_id,
                    account: user,
                    restricted: true,
                });
                registry.modify_policy_blacklist(admin, call).unwrap();
            });
        });
    });
}

criterion_group!(
    benches,
    mip20_metadata,
    mip20_view,
    mip20_mutate,
    mip20_factory_mutate,
    mip403_registry_view,
    mip403_registry_mutate
);
criterion_main!(benches);
