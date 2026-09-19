use scoop_identity::{
    CallableTemplateOrigin, DecodedPersistentId, DefinitionOrigin, PersistentDispatchSlotId,
    PersistentExactTypeId, PersistentIdResolver, SignatureTypeKey, SourceContextKey, SourceSpan,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

pub(super) use super::super::*;
pub(super) use crate::cross_cone_interface::expression_test_support::{
    Fixture, ResolutionError, Resolver,
};
pub(super) use crate::cross_cone_type_semantics::protected_defaults::{
    CanonicalProtectedDefaultExpressionUsesV1, CanonicalProtectedDefaultSlotCallDomainsV1,
    ProtectedDefaultAccessWitnessV1, ProtectedDefaultExpressionUseV1,
    ProtectedDefaultReceiverUseV1,
};
use crate::{
    DefaultConstructorRefV1, DefaultFieldRefV1, ExportDefaultCallableTargetV1,
    ExportDefinitionSourceV1, PersistentAccessDomainV1, PersistentLookupDomainV1,
};

// This source-expression fixture has no exact-type or dispatch-slot records.
macro_rules! absent_identity {
    ($identity:ty) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;
            fn resolve(
                &mut self,
                _: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                Err(ResolutionError)
            }
        }
    };
}
absent_identity!(PersistentExactTypeId);
absent_identity!(PersistentDispatchSlotId);

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

pub(super) fn uses(index: u32) -> CanonicalProtectedDefaultExpressionUsesV1 {
    CanonicalProtectedDefaultExpressionUsesV1::try_new(vec![ProtectedDefaultExpressionUseV1::new(
        index,
        ProtectedDefaultReceiverUseV1::ImplicitThis,
    )])
    .unwrap()
}
pub(super) fn witness(f: &Fixture) -> ProtectedDefaultAccessWitnessV1 {
    let domain = PersistentLookupDomainV1::new(PersistentAccessDomainV1::universal());
    ProtectedDefaultAccessWitnessV1::param_free(
        CallableTemplateOrigin::Function(f.function),
        domain.clone(),
        CanonicalProtectedDefaultSlotCallDomainsV1::try_new(vec![]).unwrap(),
        domain,
    )
    .unwrap()
}
pub(super) fn record<T>(f: &Fixture, target: T) -> ProtectedDefaultReferenceV1<T> {
    ProtectedDefaultReferenceV1::new(target, f.origin(), witness(f), uses(0))
}
pub(super) fn other_origin(f: &Fixture) -> ExportDefinitionSourceV1 {
    let source = f.origin().origin().source().clone();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(8, 12).unwrap(), &context).unwrap(),
    )
}
pub(super) fn empty_set() -> ProtectedDefaultReferenceSetV1 {
    ProtectedDefaultReferenceSetV1::try_new(vec![], vec![], vec![], vec![], vec![], vec![]).unwrap()
}
pub(super) fn full_set(f: &Fixture) -> ProtectedDefaultReferenceSetV1 {
    let owner = SignatureTypeKey::Nominal(f.type_id);
    ProtectedDefaultReferenceSetV1::try_new(
        vec![record(
            f,
            ExportDefaultCallableTargetV1::Callable(f.callable()),
        )],
        vec![record(
            f,
            DefaultConstructorRefV1::Struct {
                declaration: f.constructor,
                owner_type: owner.clone(),
            },
        )],
        vec![record(f, owner.clone())],
        vec![record(f, f.property)],
        vec![record(f, f.object)],
        vec![record(
            f,
            DefaultFieldRefV1::Struct {
                declaration: f.field,
                owner_type: owner,
            },
        )],
    )
    .unwrap()
}
pub(super) fn decoded(
    set: &ProtectedDefaultReferenceSetV1,
) -> DecodedProtectedDefaultReferenceSetV1 {
    decode_canonical(&encode(set).unwrap(), DecodeLimits::default()).unwrap()
}
pub(super) fn singleton_domains(f: &Fixture) -> Vec<ProtectedDefaultReferenceSetV1> {
    let full = full_set(f);
    let mut sets = vec![empty_set(); 6];
    sets[0].callables = full.callables;
    sets[1].constructors = full.constructors;
    sets[2].types = full.types;
    sets[3].globals = full.globals;
    sets[4].singleton_values = full.singleton_values;
    sets[5].fields = full.fields;
    sets
}
