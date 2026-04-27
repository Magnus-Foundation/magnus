use crate::{
    MagnusBlockEnv, MagnusInvalidTransaction, MagnusTxEnv,
    error::MagnusHaltReason,
    evm::{MagnusContext, MagnusEvm},
    handler::MagnusEvmHandler,
};
use alloy_evm::{Database, TransactionEnvMut};
use revm::{
    DatabaseCommit, ExecuteCommitEvm, ExecuteEvm,
    context::{ContextSetters, TxEnv, result::ExecResultAndState},
    context_interface::{
        ContextTr, JournalTr,
        result::{EVMError, ExecutionResult},
    },
    handler::{Handler, SystemCallTx, system_call::SystemCallEvm},
    inspector::{InspectCommitEvm, InspectEvm, InspectSystemCallEvm, Inspector, InspectorHandler},
    primitives::{Address, Bytes},
    state::EvmState,
};

/// Total gas system transactions are allowed to use.
const SYSTEM_CALL_GAS_LIMIT: u64 = 250_000_000;

impl<DB, I> ExecuteEvm for MagnusEvm<DB, I>
where
    DB: Database,
{
    type Tx = MagnusTxEnv;
    type Block = MagnusBlockEnv;
    type State = EvmState;
    type Error = EVMError<DB::Error, MagnusInvalidTransaction>;
    type ExecutionResult = ExecutionResult<MagnusHaltReason>;

    fn set_block(&mut self, block: Self::Block) {
        self.inner.ctx.set_block(block);
    }

    fn transact_one(&mut self, tx: Self::Tx) -> Result<Self::ExecutionResult, Self::Error> {
        self.inner.ctx.set_tx(tx);
        let mut h = MagnusEvmHandler::new();
        h.run(self)
    }

    fn finalize(&mut self) -> Self::State {
        self.inner.ctx.journal_mut().finalize()
    }

    fn replay(
        &mut self,
    ) -> Result<ExecResultAndState<Self::ExecutionResult, Self::State>, Self::Error> {
        let mut h = MagnusEvmHandler::new();
        h.run(self).map(|result| {
            let state = self.finalize();
            ExecResultAndState::new(result, state)
        })
    }
}

impl<DB, I> ExecuteCommitEvm for MagnusEvm<DB, I>
where
    DB: Database + DatabaseCommit,
{
    fn commit(&mut self, state: Self::State) {
        self.inner.ctx.db_mut().commit(state);
    }
}

impl<DB, I> InspectEvm for MagnusEvm<DB, I>
where
    DB: Database,
    I: Inspector<MagnusContext<DB>>,
{
    type Inspector = I;

    fn set_inspector(&mut self, inspector: Self::Inspector) {
        self.inner.inspector = inspector;
    }

    fn inspect_one_tx(&mut self, tx: Self::Tx) -> Result<Self::ExecutionResult, Self::Error> {
        self.inner.ctx.set_tx(tx);
        let mut h = MagnusEvmHandler::new();
        h.inspect_run(self)
    }
}

impl<DB, I> InspectCommitEvm for MagnusEvm<DB, I>
where
    DB: Database + DatabaseCommit,
    I: Inspector<MagnusContext<DB>>,
{
}

impl<DB, I> SystemCallEvm for MagnusEvm<DB, I>
where
    DB: Database,
{
    fn system_call_one_with_caller(
        &mut self,
        caller: Address,
        system_contract_address: Address,
        data: Bytes,
    ) -> Result<Self::ExecutionResult, Self::Error> {
        let mut tx = TxEnv::new_system_tx_with_caller(caller, system_contract_address, data);
        tx.set_gas_limit(SYSTEM_CALL_GAS_LIMIT);
        self.inner.ctx.set_tx(tx.into());
        let mut h = MagnusEvmHandler::new();
        h.run_system_call(self)
    }
}

impl<DB, I> InspectSystemCallEvm for MagnusEvm<DB, I>
where
    DB: Database,
    I: Inspector<MagnusContext<DB>>,
{
    fn inspect_one_system_call_with_caller(
        &mut self,
        caller: Address,
        system_contract_address: Address,
        data: Bytes,
    ) -> Result<Self::ExecutionResult, Self::Error> {
        let mut tx = TxEnv::new_system_tx_with_caller(caller, system_contract_address, data);
        tx.set_gas_limit(SYSTEM_CALL_GAS_LIMIT);
        self.inner.ctx.set_tx(tx.into());
        let mut h = MagnusEvmHandler::new();
        h.inspect_run_system_call(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::{Context, ExecuteEvm, MainContext, database::EmptyDB};

    /// Test set_block and replay with default MagnusEvm.
    #[test]
    fn test_set_block_and_replay() {
        let db = EmptyDB::new();
        let mut tx = MagnusTxEnv::default();
        tx.fee_token = Some(magnus_contracts::precompiles::MAGNUS_USD_ADDRESS);
        let ctx = Context::mainnet()
            .with_db(db)
            .with_block(MagnusBlockEnv::default())
            .with_cfg(Default::default())
            .with_tx(tx);
        let mut evm = MagnusEvm::new(ctx, ());

        // Set block with default fields
        evm.set_block(MagnusBlockEnv::default());

        // Replay executes the current transaction and returns result with state.
        // With default tx (no calls, system tx), it should succeed.
        let result = evm.replay();
        assert!(result.is_ok());

        let exec_result = result.unwrap();
        assert!(exec_result.result.is_success());
    }
}
