//! Borrowed type access demands, without declaration or visibility authority.
use scoop_identity::{
    CallingConvention, PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};
mod walk;

/// Every identity-bearing or scope-dependent constituent is reported before
/// its children. Pointer wrappers require the corresponding checked core role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultSourceTypeAccessDemandV1<'t> {
    Nominal(PersistentTypeId),
    NominalApplication {
        origin: PersistentGenericTypeId,
        arguments: &'t [SignatureTypeKey],
    },
    RawPointer {
        pointee: &'t SignatureTypeKey,
    },
    NativeFunctionPointer {
        calling_convention: CallingConvention,
        parameters: &'t [SignatureTypeKey],
        result: &'t SignatureTypeKey,
    },
    Binder {
        depth: u32,
        index: u32,
    },
}

/// Walks the complete signature in wire order, retaining repeated occurrences.
/// The visitor must resolve actual artifact sources and provider binder frames;
/// successful traversal alone proves neither type validity nor accessibility.
pub fn visit_default_source_type_access_demands<'t, E>(
    ty: &'t SignatureTypeKey,
    meter: &mut BudgetMeter,
    path: &WirePath,
    visitor: &mut impl FnMut(
        DefaultSourceTypeAccessDemandV1<'t>,
        &mut BudgetMeter,
        &WirePath,
    ) -> Result<(), E>,
) -> Result<(), DefaultSourceTypeAccessVisitError<E>> {
    walk::visit(ty, meter, path, 1, visitor)
}

#[derive(Debug)]
pub enum DefaultSourceTypeAccessVisitError<E> {
    Resource(WireError),
    Visitor(E),
}
impl<E> From<WireError> for DefaultSourceTypeAccessVisitError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: std::fmt::Display> std::fmt::Display for DefaultSourceTypeAccessVisitError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Visitor(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for DefaultSourceTypeAccessVisitError<E> {}
