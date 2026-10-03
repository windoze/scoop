use crate::{Body, ExportReleaseHookId, SourceNominalId, Span, TypeId, TypeParamId};

/// A nominal's complete release contract, before the physical nullable ABI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ReleasePolicy<H> {
    #[default]
    None,
    SynchronousGcFree {
        hook: H,
    },
}

/// A dependency's non-generic hook remains a definition of its provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportReleaseHookRef {
    Template(ExportReleaseHookId),
    Imported { requirements: Vec<TypeParamId> },
}

#[derive(Debug, Clone)]
pub struct ExportReleaseHook {
    pub owner: SourceNominalId,
    pub requirements: Vec<TypeParamId>,
    pub body: Body,
    pub span: Span,
}

/// A value copy from this hook's own reclaiming object. No managed receiver
/// can be expressed or recovered through this reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseFieldRef {
    pub owner: TypeId,
    pub field: scoop_identity::PersistentFieldId,
}
