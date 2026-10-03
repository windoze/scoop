//! Inferred callable effect; parameter identities belong to the owning graph.

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum ConditionalReleaseCallability<P> {
    #[default]
    Unavailable,
    NoTransition {
        requirements: Vec<P>,
    },
}

pub type ReleaseCallability = ConditionalReleaseCallability<crate::TypeParamId>;
