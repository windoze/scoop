//! Borrowed physical definitions never discover source or selected roots.

use scoop_identity::{CallableBodyKey, CallableBodyKeyKind, ConeIdentity};
use scoop_lir as lir;
use scoop_wire::WirePath;

use super::{
    LirStrongProductionReplayedCrossConeLayoutSections as Sections,
    SharedLirStrongProductionError as Error,
};

pub(super) fn definitions(
    consumer: ConeIdentity,
    dependencies: &[&Sections<'_>],
) -> Result<
    (
        lir::StrongTypeReferenceDefinitionsV2,
        lir::StrongInitializationDefinitionCatalogV2,
    ),
    Error,
> {
    let path = WirePath::root();
    let mut types = Vec::new();
    let mut units = Vec::new();

    for dependency in dependencies {
        let foundation = dependency.prepared.lir_foundation();
        let strong = dependency.lir_strong_production();
        let descriptors = strong.type_registrations().registrations();
        let callables = strong.callable_registrations().registrations();
        scoop_wire::allocation::try_reserve(&mut types, descriptors.len(), &path)?;
        for descriptor in descriptors {
            types.push(lir::StrongShapeDefinitionRefV1::from_foundation(
                lir::ExternalStrongShapeSubjectV1::TypeDescriptor(descriptor.exact_type()),
                foundation,
            )?);
        }
        scoop_wire::allocation::try_reserve(&mut types, callables.len(), &path)?;
        for callable in callables {
            let key = dependency
                .prepared
                .shared_metadata()
                .identities
                .canonical_key::<_, CallableBodyKey>(callable.body())?;
            let target = match key.kind() {
                CallableBodyKeyKind::Strong(target) => target,
                CallableBodyKeyKind::InitializationStartupGateway(unit) => {
                    let units = strong.initialization_registrations().registrations();

                    let registration = units
                        .iter()
                        .find(|record| record.semantic().unit() == unit)
                        .ok_or(Error::CallableBody(callable.body()))?;
                    if !matches!(
                        registration.schedule(),
                        lir::StrongInitializationRegistrationSchedulePlanV1::EagerStartup { gateway, .. }
                            if gateway.body() == callable.body()
                                && gateway.entry_symbol() == callable.entry_symbol()
                    ) {
                        return Err(Error::CallableBody(callable.body()));
                    }
                    // Gateways are unit-owned entries, not ordinary dispatch targets.
                    continue;
                }
                CallableBodyKeyKind::Odr(_) => continue,
                CallableBodyKeyKind::RootGateway { .. } => {
                    return Err(Error::CallableBody(callable.body()));
                }
            };
            types.push(lir::StrongShapeDefinitionRefV1::from_foundation(
                lir::ExternalStrongShapeSubjectV1::Callable(
                    scoop_identity::CallableDefinitionOwner::Strong(target),
                ),
                foundation,
            )?);
        }
        let registrations = strong.initialization_registrations();
        scoop_wire::allocation::try_reserve(
            &mut units,
            registrations.registrations().len(),
            &path,
        )?;
        for record in registrations.registrations() {
            let unit = record.semantic().unit();
            units.push(
                lir::StrongInitializationUnitDefinitionRefV2::from_registrations(
                    registrations,
                    unit,
                )
                .ok_or(Error::InitializationDefinition(unit))?,
            );
        }
    }
    Ok((
        lir::StrongTypeReferenceDefinitionsV2::new(
            consumer,
            &types,
            &dependencies
                .iter()
                .map(|dependency| dependency.layout.exports().layouts())
                .collect::<Vec<_>>(),
        )?,
        lir::StrongInitializationDefinitionCatalogV2::new(consumer, &units)?,
    ))
}
