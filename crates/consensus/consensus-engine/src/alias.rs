//! A collection of aliases for frequently used (primarily Magnus core) types.

pub(crate) mod marshal {
    use magnus_bft::{
        marshal,
        simplex::{scheme::bls12381_threshold::Scheme, types::Finalization},
        types::FixedEpocher,
    };
    use magnus_cryptography::{bls12381::primitives::variant::MinSig, ed25519::PublicKey};
    use magnus_storage::archive::immutable;
    use magnus_utils::acknowledgement::Exact;

    use crate::consensus::{Digest, block::Block};

    pub(crate) type Actor<TContext> = marshal::Actor<
        TContext,
        Block,
        crate::epoch::SchemeProvider,
        immutable::Archive<TContext, Digest, Finalization<Scheme<PublicKey, MinSig>, Digest>>,
        immutable::Archive<TContext, Digest, Block>,
        FixedEpocher,
        Exact,
    >;

    pub(crate) type Mailbox = marshal::Mailbox<Scheme<PublicKey, MinSig>, Block>;
}
