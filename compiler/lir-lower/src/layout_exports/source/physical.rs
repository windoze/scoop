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

pub(super) fn project(input: LayoutAbiExportInputV1<'_>) -> Result<Vec<RequiredImport>, Error> {
    let module = input.lir.module();

    let mut imports = Vec::new();

    for (_, descriptor) in module.meta.external_type_descriptors.iter() {
        push(
            &mut imports,
            RequiredImport {
                provider: descriptor.provider(),
                subject: Subject::TypeDescriptor(descriptor.target()),
                symbol: descriptor.expected_symbol(),
                definition: descriptor.required_definition(),
            },
        )?;
    }

    for registration in input.registration.immortal_registrations().registrations() {
        if let lir::ImmortalObjectTypeRegistrationRefV1::DependencyExternal { provider, exact } =
            registration.semantic().type_registration_ref()
        {
            push(
                &mut imports,
                RequiredImport {
                    provider,
                    subject: Subject::TypeRegistration(exact),
                    symbol: registration.type_registration_symbol(),
                    definition: ObjectDefinitionPlanId::from_key(
                        &Subject::TypeRegistration(exact)
                            .expected_definition(provider)
                            .map_err(|_| Error::PhysicalDefinition {
                                provider,
                                subject: Subject::TypeRegistration(exact),
                            })?
                            .0,
                    )
                    .map_err(|_| Error::PhysicalDefinition {
                        provider,
                        subject: Subject::TypeRegistration(exact),
                    })?,
                },
            )?;
        }
    }

    for (_, global) in module.globals.iter() {
        if let lir::GlobalInit::ImportedStorage { definition, .. } = &global.init {
            push(
                &mut imports,
                RequiredImport {
                    provider: definition.provider(),
                    subject: definition.subject(),
                    symbol: definition.expected_symbol(),
                    definition: definition.required_definition(),
                },
            )?;
        }
        if let lir::GlobalInit::Storage {
            layout: lir::StaticStorageLayout::External(value),
            ..
        } = &global.init
        {
            let scan = value.scan_definition();
            push(
                &mut imports,
                RequiredImport {
                    provider: scan.provider(),
                    subject: scan.subject(),
                    symbol: scan.symbol(),
                    definition: scan.definition(),
                },
            )?;
        }
    }

    for (_, callable) in module.meta.external_callables.iter() {
        if callable.origin() == lir::ExternalCallableOrigin::LayoutV1 {
            push(
                &mut imports,
                RequiredImport {
                    provider: callable.provider(),
                    subject: Subject::Callable(scoop_identity::CallableDefinitionOwner::Strong(
                        callable.target(),
                    )),
                    symbol: callable.expected_symbol(),
                    definition: callable.required_definition(),
                },
            )?;
        }
    }
    for (_, descriptor) in module.meta.type_descriptors.iter() {
        for slot in descriptor
            .vtable
            .slots()
            .iter()
            .chain(descriptor.itables.iter().flat_map(|table| table.slots()))
        {
            if let lir::CallableRef::External(id) = slot.callable {
                let callable = &module.meta.external_callables[id];
                push(
                    &mut imports,
                    RequiredImport {
                        provider: callable.provider(),
                        subject: Subject::Callable(
                            scoop_identity::CallableDefinitionOwner::Strong(callable.target()),
                        ),
                        symbol: callable.expected_symbol(),
                        definition: callable.required_definition(),
                    },
                )?;
            }
        }
    }
    for unit in input
        .registration
        .registration_production()
        .initialization_units()
        .registrations()
    {
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
                )?;
            }
        }
    }

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
    actual: &[lir::ExternalShapeLinkImportV1],
) -> Result<(), Error> {
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
