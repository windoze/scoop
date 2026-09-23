use super::*;
use lir::ExternalStrongShapeSubjectV1 as Subject;
use scoop_identity::{ObjectDefinitionPlanId, PersistentSymbolRequest};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RequiredImport {
    provider: ConeIdentity,
    subject: Subject,
    symbol: PersistentSymbolRequest,
    definition: ObjectDefinitionPlanId,
}
impl RequiredImport {
    fn key(&self) -> (ConeIdentity, Subject) {
        (self.provider, self.subject)
    }
    fn error(&self) -> Error {
        Error::PhysicalDefinition {
            provider: self.provider,
            subject: self.subject,
        }
    }
}

pub(super) fn project(
    input: LayoutAbiExportInputV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<Vec<RequiredImport>, Error> {
    let module = input.lir.module();
    let path = WirePath::root();
    let mut imports = Vec::new();
    meter.charge_work(module.meta.external_type_descriptors.len() as u64, &path)?;
    for (id, descriptor) in module.meta.external_type_descriptors.iter() {
        if module.meta.well_known_type_descriptors.string == lir::TypeDescriptorRef::External(id) {
            continue;
        }
        push(
            &mut imports,
            RequiredImport {
                provider: descriptor.provider(),
                subject: Subject::TypeDescriptor(descriptor.target()),
                symbol: descriptor.expected_symbol(),
                definition: descriptor.required_definition(),
            },
            meter,
        )?;
    }
    meter.charge_work(module.meta.external_callables.len() as u64, &path)?;
    for (_, callable) in module.meta.external_callables.iter() {
        if callable.origin() == lir::ExternalCallableOrigin::LayoutV1 {
            push(
                &mut imports,
                RequiredImport {
                    provider: callable.provider(),
                    subject: Subject::Callable(callable.target()),
                    symbol: callable.expected_symbol(),
                    definition: callable.required_definition(),
                },
                meter,
            )?;
        }
    }
    for unit in input
        .registration
        .registration_production()
        .initialization_units()
        .registrations()
    {
        meter.charge_work(unit.semantic().dependencies().len() as u64 + 1, &path)?;
        for dependency in unit.semantic().dependencies() {
            if let lir::StrongInitializationDependencyKindV2::DependencyExternalUnit {
                provider,
                unit_ref,
            } = dependency.kind()
            {
                push(
                    &mut imports,
                    RequiredImport {
                        provider,
                        subject: Subject::InitializationDescriptor(unit_ref.unit()),
                        symbol: unit_ref.descriptor().symbol(),
                        definition: unit_ref.descriptor().plan(),
                    },
                    meter,
                )?;
            }
        }
    }
    sort_cost(imports.len(), meter)?;
    imports.sort_unstable_by_key(RequiredImport::key);
    for pair in imports.windows(2) {
        if pair[0].key() == pair[1].key() && pair[0] != pair[1] {
            return Err(pair[0].error());
        }
    }
    imports.dedup();
    Ok(imports)
}

pub(super) fn validate(
    expected: &[RequiredImport],
    actual: &[lir::ExternalShapeLinkImportV1<'_>],
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    meter.charge_work(
        actual.len() as u64 + expected.len() as u64,
        &WirePath::root(),
    )?;
    if actual.len() != expected.len() {
        return Err(Error::PhysicalInventory);
    }
    for (actual, expected) in actual.iter().zip(expected) {
        if actual.provider() != expected.provider
            || actual.subject() != expected.subject
            || actual.expected_symbol() != expected.symbol
            || actual.required_definition() != expected.definition
        {
            return Err(expected.error());
        }
    }
    Ok(())
}
