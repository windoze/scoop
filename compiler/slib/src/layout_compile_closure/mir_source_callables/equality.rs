use scoop_hir as hir;
use scoop_identity::{
    CallableOwner, CoreBuiltinNominal, Effect, ExactCallableSignature, ExactTypeKey,
    GeneratedCallableKey, PersistentExactTypeId, PersistentGeneratedCallableId, SignatureTypeKey,
    StrongCallableDefinitionOwner,
};
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeSet;

mod errors;
use SharedMirEqualityValidationError as Error;
pub use errors::SharedMirEqualityValidationError;

/// Joins source applications to the existing complete strong definition surface.
/// A source-only application never creates a callable export by itself.
pub fn validate_shared_mir_equality(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[hir::CheckedSharedTypeFoundationV1<'_>],
    strong: &mir::StrongCallableBridgeSurfaceV1,
    callables: &mir::CanonicalMirCallableBindingsV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let metadata = source.metadata();
    let applications = metadata.derived_equality_applications(meter)?;
    let mut required = BTreeSet::new();
    for definition in strong.bridges() {
        meter.charge_work(1, &WirePath::root())?;
        let CallableOwner::Generated(callable) = definition.implementation() else {
            continue;
        };
        lookup(metadata.identities.identity_count(), meter)?;
        let key = metadata
            .identities
            .canonical_key::<_, GeneratedCallableKey>(callable)?;
        let GeneratedCallableKey::DerivedEquality { exact_owner } = *key else {
            continue;
        };
        if !exported_owner(source, exact_owner, meter)? {
            continue;
        }
        lookup(applications.len(), meter)?;
        if applications.get(&callable) != Some(&exact_owner) {
            return Err(Error::MissingSource(callable));
        }
        lookup(callables.entries().len(), meter)?;
        let binding = callables
            .get(StrongCallableDefinitionOwner::GeneratedCallable(callable))
            .ok_or(Error::MissingCallable(callable))?;
        let boolean = boolean_type(source, dependencies, meter)?;
        validate_binding(definition, binding, callable, exact_owner, boolean, meter)?;
        lookup(required.len(), meter)?;
        meter.check_table_entries(required.len() as u64 + 1, &WirePath::root())?;
        meter.charge_collection_slots(1, &WirePath::root())?;
        required.insert(callable);
    }
    for binding in callables.entries() {
        meter.charge_work(1, &WirePath::root())?;
        if let mir::MirCallableOriginV1::Generated {
            callable,
            role: GeneratedCallableKey::DerivedEquality { .. },
        } = binding.origin()
        {
            lookup(required.len(), meter)?;
            if !required.contains(callable) {
                return Err(Error::UnexpectedCallable(*callable));
            }
        }
    }
    Ok(())
}

fn exported_owner(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    exact: PersistentExactTypeId,
    meter: &mut BudgetMeter,
) -> Result<bool, Error> {
    let metadata = source.metadata();
    lookup(metadata.identities.identity_count(), meter)?;
    let key = metadata
        .identities
        .canonical_key::<_, ExactTypeKey>(exact)?;
    let ExactTypeKey::Nominal(nominal) = *key else {
        return Ok(false);
    };
    let unit = CoreBuiltinNominal::Unit.identity_record();
    if nominal == unit.id() {
        return Ok(metadata.provider == unit.key().origin());
    }
    lookup(source.representations().table().records().len(), meter)?;
    let Some(representation) = source.representations().get(nominal) else {
        return Ok(false);
    };
    if !matches!(
        representation.shape(),
        hir::NominalRepresentationShapeV1::Struct { .. }
            | hir::NominalRepresentationShapeV1::Enum { .. }
    ) {
        return Err(Error::Owner(exact));
    }
    Ok(true)
}

fn validate_binding(
    definition: &mir::StrongCallableBridgeV1,
    binding: &mir::ParamFreeMirCallableBindingV1,
    callable: PersistentGeneratedCallableId,
    owner: PersistentExactTypeId,
    boolean: PersistentExactTypeId,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    meter.charge_collection_slots(1, &WirePath::root())?;
    let expected = mir::MirBridgeCallableSignatureV1::new(
        ExactCallableSignature::new(Effect::Ordinary, Some(owner), vec![owner], boolean),
        mir::GcEffect::Managed,
    );
    meter.charge_work(24, &WirePath::root())?;
    if definition.signature() != expected.exact()
        || definition.role() != mir::CallableRole::Ordinary
    {
        return Err(Error::Definition(callable));
    }
    if binding.semantic_signature() != &expected || binding.lowered_signature() != &expected {
        return Err(Error::Signature(callable));
    }
    let origin = mir::MirCallableOriginV1::Generated {
        callable,
        role: GeneratedCallableKey::DerivedEquality { exact_owner: owner },
    };
    let role = mir::MirCallableLoweringRoleV1::DerivedEquality { owner };
    if binding.origin() != &origin || binding.lowering_role() != &role {
        return Err(Error::Role(callable));
    }
    Ok(())
}

fn boolean_type(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[hir::CheckedSharedTypeFoundationV1<'_>],
    meter: &mut BudgetMeter,
) -> Result<PersistentExactTypeId, Error> {
    let mut found = None;
    for provider in std::iter::once(source).chain(dependencies.iter().copied()) {
        let metadata = provider.metadata();
        for nominal in metadata.public.nominal_interfaces().all_records() {
            meter.charge_work(1, &WirePath::root())?;
            if !matches!(
                nominal.source_shape(),
                hir::NominalSourceShapeV1::Intrinsic(intrinsic)
                    if intrinsic.family() == hir::IntrinsicTypeKind::Boolean
            ) {
                continue;
            }
            let hir::SourceNominalId::Concrete(owner) = nominal.declaration() else {
                return Err(Error::BooleanSource);
            };
            let exact = metadata.signature_exact_type(&SignatureTypeKey::Nominal(owner), meter)?;
            if found.replace(exact).is_some() {
                return Err(Error::BooleanSource);
            }
        }
    }
    found.ok_or(Error::BooleanSource)
}

fn lookup(count: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(u64::from(count.max(1).ilog2()) + 1, &WirePath::root())?)
}
