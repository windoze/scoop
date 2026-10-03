//! Borrowed physical definitions never discover source or selected roots.

use scoop_identity::{CallableBodyKey, CallableBodyKeyKind, ConeIdentity};
use scoop_lir as lir;
use scoop_wire::WirePath;

use crate::SharedLirStrongProductionError as Error;

pub(crate) struct DefinitionProvider<'a> {
    pub identities: &'a scoop_identity::ValidatedIdentityGraph,
    pub foundation: &'a lir::ConeLirFoundation,
    pub strong: &'a lir::ConeProductionSectionV2,
    pub layouts: &'a lir::CanonicalExactLayoutExportsV1,
}

pub(crate) fn definitions(
    consumer: ConeIdentity,
    dependencies: &[DefinitionProvider<'_>],
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
        let foundation = dependency.foundation;
        let strong = dependency.strong;
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
        // Receiver applications have their own local materializations; only
        // source-owned units participate in external initialization services.
        for record in registrations
            .registrations()
            .iter()
            .filter(|record| record.definition_owner() == lir::RegistrationDefinitionOwner::Strong)
        {
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
                .map(|dependency| dependency.layouts)
                .collect::<Vec<_>>(),
        )?,
        lir::StrongInitializationDefinitionCatalogV2::new(consumer, &units)?,
    ))
}
