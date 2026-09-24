//! Borrowed physical definitions never discover source or selected roots.

use scoop_identity::{CallableBodyKey, CallableBodyKeyKind, ConeIdentity, PersistentSymbolRequest};
use scoop_lir as lir;
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    LirStrongProductionReplayedCrossConeLayoutSections as Sections,
    SharedLirStrongProductionError as Error,
};

pub(super) fn definitions(
    consumer: ConeIdentity,
    dependencies: &[&Sections<'_>],
    external: &lir::StrongExternalLirBridgeSurfaceV1,
    meter: &mut BudgetMeter,
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
    meter.charge_work(dependencies.len() as u64, &path)?;
    for dependency in dependencies {
        let foundation = dependency.prepared.lir_foundation();
        let strong = dependency.lir_strong_production();
        let descriptors = strong.type_registrations().registrations();
        let callables = strong.callable_registrations().registrations();
        meter.try_reserve_collection_slots(&mut types, descriptors.len(), &path)?;
        for descriptor in descriptors {
            if is_service(
                foundation.producer(),
                descriptor.descriptor_symbol(),
                external,
                meter,
            )? {
                continue;
            }
            types.push(lir::StrongShapeDefinitionRefV1::from_foundation(
                lir::ExternalStrongShapeSubjectV1::TypeDescriptor(descriptor.exact_type()),
                foundation,
                meter,
            )?);
        }
        meter.try_reserve_collection_slots(&mut types, callables.len(), &path)?;
        for callable in callables {
            if is_service(
                foundation.producer(),
                callable.entry_symbol(),
                external,
                meter,
            )? {
                continue;
            }
            meter.charge_work(1, &path)?;
            let key = dependency
                .prepared
                .shared_metadata()
                .identities
                .canonical_key::<_, CallableBodyKey>(callable.body())?;
            let CallableBodyKeyKind::Strong(target) = key.kind() else {
                return Err(Error::CallableBody(callable.body()));
            };
            types.push(lir::StrongShapeDefinitionRefV1::from_foundation(
                lir::ExternalStrongShapeSubjectV1::Callable(target),
                foundation,
                meter,
            )?);
        }
        let registrations = strong.initialization_registrations();
        meter.try_reserve_collection_slots(
            &mut units,
            registrations.registrations().len(),
            &path,
        )?;
        for record in registrations.registrations() {
            meter.charge_work(64, &path)?;
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
        lir::StrongTypeReferenceDefinitionsV2::new(consumer, &types, meter)?,
        lir::StrongInitializationDefinitionCatalogV2::new(consumer, &units, meter)?,
    ))
}

pub(super) fn validate_external_bridges(
    external: &lir::StrongExternalLirBridgeSurfaceV1,
    dependencies: &[&Sections<'_>],
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    for reference in external.bridges() {
        meter.charge_work(dependencies.len() as u64, &path)?;
        let provider = reference.provider();
        let dependency = dependencies
            .iter()
            .find(|dependency| dependency.identity() == provider)
            .ok_or(Error::Provider(provider))?;
        let symbol = reference.expected_symbol();
        meter.charge_work(64, &path)?;
        match reference {
            lir::StrongExternalLirBridgeV1::TypeDescriptor(reference) => {
                let protocol = dependency
                    .prepared
                    .hir_production()
                    .compiler_protocol_definitions()
                    .ok_or(Error::ExternalDefinition(symbol.key()))?;
                if protocol.string_capability().exact_type() != reference.target() {
                    return Err(Error::ExternalDefinition(symbol.key()));
                }
                let descriptor = dependency
                    .lir_exports()
                    .descriptors()
                    .get(reference.target())
                    .ok_or(Error::ExternalDefinition(symbol.key()))?;
                let physical = descriptor.physical_definition();
                if physical.provider() != provider
                    || physical.symbol() != symbol
                    || physical.definition() != reference.required_definition()
                {
                    return Err(Error::ExternalDefinition(symbol.key()));
                }
                let registrations = dependency.strong.type_registrations().registrations();
                let position = registrations
                    .binary_search_by_key(&reference.target(), |record| record.exact_type())
                    .map_err(|_| Error::ExternalDefinition(symbol.key()))?;
                let registration = &registrations[position];
                if registration.descriptor_definition_plan() != physical.definition()
                    || registration.descriptor_primary_atom() != physical.primary()
                    || registration.descriptor_symbol() != symbol
                {
                    return Err(Error::ExternalDefinition(symbol.key()));
                }
            }
            lir::StrongExternalLirBridgeV1::Callable(reference) => {
                let expected = dependency
                    .initialization_cycle_abi()
                    .ok_or(Error::ExternalAbi(symbol.key()))?;
                meter.charge_work(
                    expected.abi_signature().signature().parameters().len() as u64,
                    &path,
                )?;
                if reference.bridge().callable_abi() != expected {
                    return Err(Error::ExternalAbi(symbol.key()));
                }
                let physical = lir::StrongShapeDefinitionRefV1::from_foundation(
                    lir::ExternalStrongShapeSubjectV1::Callable(expected.target()),
                    dependency.prepared.lir_foundation(),
                    meter,
                )?;
                if physical.symbol() != symbol
                    || physical.definition() != expected.required_definition()
                {
                    return Err(Error::ExternalDefinition(symbol.key()));
                }
            }
        }
    }
    Ok(())
}

fn is_service(
    provider: ConeIdentity,
    symbol: PersistentSymbolRequest,
    external: &lir::StrongExternalLirBridgeSurfaceV1,
    meter: &mut BudgetMeter,
) -> Result<bool, Error> {
    meter.charge_work(external.bridges().len() as u64, &WirePath::root())?;
    Ok(external
        .bridges()
        .iter()
        .any(|reference| reference.provider() == provider && reference.expected_symbol() == symbol))
}
