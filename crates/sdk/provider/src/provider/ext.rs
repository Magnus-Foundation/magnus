use alloy_provider::{
    Identity, ProviderBuilder,
    fillers::{JoinFill, RecommendedFillers},
};

use crate::{MagnusFillers, MagnusNetwork, fillers::Random2DNonceFiller};

/// Extension trait for [`ProviderBuilder`] with Magnus-specific functionality.
pub trait MagnusProviderBuilderExt {
    /// Returns a provider builder with the recommended Magnus fillers and the random 2D nonce filler.
    ///
    /// See [`Random2DNonceFiller`] for more information on random 2D nonces.
    fn with_random_2d_nonces(
        self,
    ) -> ProviderBuilder<
        Identity,
        JoinFill<Identity, MagnusFillers<Random2DNonceFiller>>,
        MagnusNetwork,
    >;
}

impl MagnusProviderBuilderExt
    for ProviderBuilder<
        Identity,
        JoinFill<Identity, <MagnusNetwork as RecommendedFillers>::RecommendedFillers>,
        MagnusNetwork,
    >
{
    fn with_random_2d_nonces(
        self,
    ) -> ProviderBuilder<
        Identity,
        JoinFill<Identity, MagnusFillers<Random2DNonceFiller>>,
        MagnusNetwork,
    > {
        ProviderBuilder::default().filler(MagnusFillers::default())
    }
}

#[cfg(test)]
mod tests {
    use alloy_provider::{Identity, ProviderBuilder, fillers::JoinFill};

    use crate::{
        MagnusFillers, MagnusNetwork, fillers::Random2DNonceFiller,
        provider::ext::MagnusProviderBuilderExt,
    };

    #[test]
    fn test_with_random_nonces() {
        let _: ProviderBuilder<_, JoinFill<Identity, MagnusFillers<Random2DNonceFiller>>, _> =
            ProviderBuilder::new_with_network::<MagnusNetwork>().with_random_2d_nonces();
    }
}
