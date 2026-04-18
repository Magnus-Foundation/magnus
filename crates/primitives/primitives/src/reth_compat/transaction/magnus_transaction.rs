use crate::MagnusTransaction;

impl reth_primitives_traits::InMemorySize for MagnusTransaction {
    fn size(&self) -> usize {
        Self::size(self)
    }
}
