use scoop_hir as hir;
use scoop_identity::{ExactTypeKey, GeneratedNominalKey};
use scoop_mir as mir;
use scoop_mir::MirTypeBridgeTypeLookupV1;
use scoop_wire::WirePath;

use super::{Error, SharedMirTypeComponent as Component, validation::Comparison};

pub(super) fn validate(
    comparison: &mut Comparison<'_, '_>,
    core: &hir::CoreBootstrapInterfaceSectionV1,
    shapes: &mir::CanonicalMirShapeSupportsV1,
) -> Result<(), Error> {
    let metadata = comparison.source.metadata();
    if shapes.provider() != metadata.provider {
        return Err(Error::ShapeProvider {
            expected: metadata.provider,
            actual: shapes.provider(),
        });
    }
    let requirements = hir::PublicNominalShapeRequirementsV1::from_shared_surface(
        metadata.provider,
        core.direct_public_surface(),
        metadata.foundation,
        metadata.public.nominal_interfaces(),
        metadata.public.callable_interfaces(),
    )?;
    let mut sources = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut sources,
        requirements.roots().len(),
        &WirePath::root(),
    )?;

    sources.extend(requirements.roots().iter().map(|root| root.source()));
    shapes.validate_required_sources(&sources)?;
    for (root, shape) in requirements.roots().iter().zip(shapes.records()) {
        mir::ParamFreeMirShapeSupportV1::try_new(
            mir::MirShapeSupportAuthority {
                identities: metadata.identities,
                types: comparison.types,
            },
            shape.source(),
            shape.exact(),
            shape.boxed(),
            shape.coroutine_step(),
            shape.coroutine_slot(),
        )?;
        let source = comparison.require_type(root.exact())?;
        Error::require(
            root.exact(),
            Component::Origin,
            shape.provider() == metadata.provider
                && shape.exact() == root.exact()
                && source.origin() == &mir::MirTypeOriginV1::SourceNominal(root.source()),
        )?;
        if let mir::MirBoxedShapeSupportV1::Available(exact) = shape.boxed() {
            let boxed = comparison.require_type(exact)?;
            Error::require(
                exact,
                Component::Facts,
                boxed.facts().kind() == mir::MirValueKindV1::Reference
                    && boxed.facts().gc() == mir::MirGcKindV1::ContainsManagedReferences,
            )?;
            Error::require(
                exact,
                Component::Base,
                boxed.base_and_interfaces().base == mir::MirBaseClassV1::None,
            )?;

            Error::require(
                exact,
                Component::Interfaces,
                boxed.base_and_interfaces().interfaces == source.base_and_interfaces().interfaces,
            )?;
        }
        for exact in [shape.coroutine_step(), shape.coroutine_slot()] {
            let helper = comparison.require_type(exact)?;
            Error::require(
                exact,
                Component::Facts,
                helper.facts().kind() == mir::MirValueKindV1::NonZeroValue
                    && helper.facts().gc() == source.facts().gc(),
            )?;
            Error::require(
                exact,
                Component::Base,
                helper.base_and_interfaces().base == mir::MirBaseClassV1::None,
            )?;
            Error::require(
                exact,
                Component::Interfaces,
                helper.base_and_interfaces().interfaces.is_empty(),
            )?;
        }
    }
    Ok(())
}

pub(super) fn generated(
    comparison: &mut Comparison<'_, '_>,
    record: &mir::ParamFreeMirTypeExportV1,
    role: &GeneratedNominalKey,
) -> Result<(), Error> {
    let identities = comparison.source.metadata().identities;
    match role {
        GeneratedNominalKey::TaskContext(storage) => {
            Error::require(
                record.exact(),
                Component::Origin,
                storage.core == comparison.source.metadata().provider,
            )?;
        }
        GeneratedNominalKey::BoxedValue { payload } => {
            let key = identities
                .canonical_key::<_, ExactTypeKey>(*payload)
                .map_err(hir::SharedTypeMetadataError::from)?;
            let interfaces = match key.as_ref() {
                ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
                    &comparison
                        .types
                        .get(*payload)
                        .ok_or(Error::MissingType(*payload))?
                        .base_and_interfaces()
                        .interfaces[..]
                }
                ExactTypeKey::Tuple(_)
                | ExactTypeKey::Function { .. }
                | ExactTypeKey::RawPointer(_)
                | ExactTypeKey::NativeFunctionPointer { .. } => &[],
            };
            Error::require(
                record.exact(),
                Component::Interfaces,
                record.base_and_interfaces().interfaces == interfaces,
            )?;
        }
        GeneratedNominalKey::CoroutineStep { result: payload }
        | GeneratedNominalKey::CoroutineSlot { value: payload } => {
            let gc = comparison.types.exact_gc_kind(identities, *payload)?;
            Error::require(record.exact(), Component::Facts, record.facts().gc() == gc)?;
            Error::require(
                record.exact(),
                Component::Interfaces,
                record.base_and_interfaces().interfaces.is_empty(),
            )?;
        }
        _ => return Err(Error::UnexpectedType(record.exact())),
    }
    comparison.require_type(record.exact())?;
    Ok(())
}
