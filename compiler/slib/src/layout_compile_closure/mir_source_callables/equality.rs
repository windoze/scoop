use scoop_hir as hir;
use scoop_identity::{
    CallableDefinitionOwner, CallableOwner, Effect, ExactCallableSignature, GeneratedCallableKey,
    OdrMemberDiscriminator, OdrMemberKey, PersistentExactTypeId, PersistentGeneratedCallableId,
    SignatureTypeKey, StrongCallableDefinitionOwner,
};
use scoop_mir as mir;
use std::collections::BTreeSet;

mod errors;
use SharedMirEqualityValidationError as Error;
pub use errors::SharedMirEqualityValidationError;

/// Joins source applications to the existing complete MIR definition surface.
/// A source-only application never creates a callable export by itself.
pub fn validate_shared_mir_equality(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[hir::CheckedSharedTypeFoundationV1<'_>],
    foundation: &mir::CanonicalMirFoundation,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    callables: &mir::CanonicalMirCallableBindingsV1,
) -> Result<(), Error> {
    let metadata = source.metadata();
    let applications = metadata.derived_equality_applications()?;
    let mut required = BTreeSet::new();
    let mut boolean_cache = None;
    for definition in foundation.callable_signatures() {
        let (implementation, callable) = match definition.subject() {
            mir::CallableSignatureSubject::Strong(CallableOwner::Generated(callable)) => (
                CallableDefinitionOwner::Strong(StrongCallableDefinitionOwner::GeneratedCallable(
                    callable,
                )),
                callable,
            ),
            mir::CallableSignatureSubject::Odr(member) => {
                let key = metadata
                    .identities
                    .canonical_key::<_, OdrMemberKey>(member.member())?;
                let OdrMemberDiscriminator::GeneratedCallable(callable) = *key.discriminator()
                else {
                    continue;
                };
                (CallableDefinitionOwner::Odr(member), callable)
            }
            _ => continue,
        };

        let key = metadata
            .identities
            .canonical_key::<_, GeneratedCallableKey>(callable)?;
        let GeneratedCallableKey::DerivedEquality { exact_owner } = *key else {
            continue;
        };
        if types.get(exact_owner).is_none() {
            continue;
        }

        if applications.get(&callable) != Some(&exact_owner) {
            return Err(Error::MissingSource(callable));
        }

        let binding = callables
            .get(implementation)
            .ok_or(Error::MissingCallable(callable))?;
        let boolean = match boolean_cache {
            Some(exact) => exact,
            None => *boolean_cache.insert(boolean_type(source, dependencies)?),
        };
        validate_binding(definition, binding, callable, exact_owner, boolean)?;

        required.insert(callable);
    }
    for binding in callables.entries() {
        if let mir::MirCallableOriginV1::Generated {
            callable,
            role: GeneratedCallableKey::DerivedEquality { .. },
        } = binding.origin()
        {
            if !required.contains(callable) {
                return Err(Error::UnexpectedCallable(*callable));
            }
        }
    }
    Ok(())
}

fn validate_binding(
    definition: &mir::CallableSignatureRecord,
    binding: &mir::ParamFreeMirCallableBindingV1,
    callable: PersistentGeneratedCallableId,
    owner: PersistentExactTypeId,
    boolean: PersistentExactTypeId,
) -> Result<(), Error> {
    let expected = mir::MirBridgeCallableSignatureV1::new(
        ExactCallableSignature::new(Effect::Ordinary, Some(owner), vec![owner], boolean),
        mir::GcEffect::Managed,
    );

    if definition.signature() != expected.exact() {
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
) -> Result<PersistentExactTypeId, Error> {
    let mut found = None;
    for provider in std::iter::once(source).chain(dependencies.iter().copied()) {
        let metadata = provider.metadata();
        for nominal in metadata.public.nominal_interfaces().all_records() {
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
            let exact = metadata.signature_exact_type(&SignatureTypeKey::Nominal(owner))?;
            if found.replace(exact).is_some() {
                return Err(Error::BooleanSource);
            }
        }
    }
    found.ok_or(Error::BooleanSource)
}
