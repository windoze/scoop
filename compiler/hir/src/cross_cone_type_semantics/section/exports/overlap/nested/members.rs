use super::*;

pub(super) fn subsets<E>(
    source: &ProtectedNestedSourceInterfaceV1,
    old: &NominalInterfaceRecordV1,
    public: &CrossConeHirInterfaceSectionV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    // Public lists are a visibility-filtered subset of complete source support.
    // The reverse inclusion is checked when each support record is visited.
    sequence(old.constructors().values().len(), meter, path)?;
    for id in old.constructors().values() {
        lookup(source.constructors().values().len(), meter, path)?;
        require(source.constructors().values().binary_search(id).is_ok())?;
    }
    sequence(old.members().members().len(), meter, path)?;
    for member in old.members().members() {
        sequence(source.members().values().len(), meter, path)?;
        require(
            source
                .members()
                .values()
                .iter()
                .any(|candidate| match (member, candidate) {
                    (
                        PublicMemberRefV1::Callable(CallableTemplateOrigin::Function(a)),
                        NestedSourceMemberRefV1::Function(b),
                    ) => a == b,
                    (
                        PublicMemberRefV1::Callable(CallableTemplateOrigin::GenericFunction(a)),
                        NestedSourceMemberRefV1::GenericFunction(b),
                    ) => a == b,
                    (
                        PublicMemberRefV1::Property(PropertyDeclarationId::Property(a)),
                        NestedSourceMemberRefV1::Property(b),
                    ) => a == b,
                    _ => false,
                }),
        )?;
    }
    sequence(old.nested_bindings().values().len(), meter, path)?;
    for binding in old.nested_bindings().values() {
        lookup(public.public_bindings().records().len(), meter, path)?;
        let binding = public
            .public_bindings()
            .get(*binding)
            .ok_or(TypeSectionExportValidationError::PublicOverlap)?;
        let id = match binding.source() {
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: BindableEntity::Type(id),
            } => SourceNominalId::Concrete(*id),
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: BindableEntity::GenericType(id),
            } => SourceNominalId::GenericTemplate(*id),
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: BindableEntity::ObjectValue(value),
            } => {
                sequence(source.source_support().records().len(), meter, path)?;
                source
                    .source_support()
                    .records()
                    .iter()
                    .find_map(|record| match record {
                        NestedSourceSupportV1::NestedNominal(record) => {
                            match record.payload().source_interface().source_shape() {
                                NominalSourceShapeV1::Object(object)
                                    if object.value() == *value =>
                                {
                                    Some(record.declaration())
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    })
                    .ok_or(TypeSectionExportValidationError::PublicOverlap)?
            }
            _ => return Err(TypeSectionExportValidationError::PublicOverlap),
        };
        sequence(source.children().values().len(), meter, path)?;
        require(source.children().values().contains(&id))?;
    }
    Ok(())
}

pub(super) fn public_entry<E>(
    record: &NestedSourceSupportV1,
    source: &ProtectedNestedSourceInterfaceV1,
    old: &NominalInterfaceRecordV1,
    public: &CrossConeHirInterfaceSectionV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    match record {
        NestedSourceSupportV1::Constructor(record) => {
            lookup(old.constructors().values().len(), meter, path)?;
            require(
                old.constructors()
                    .values()
                    .binary_search(&record.declaration())
                    .is_ok(),
            )
        }
        NestedSourceSupportV1::Callable(record) => {
            if matches!(record.declaration(), CallableTemplateOrigin::Accessor(_)) {
                return Ok(());
            }
            sequence(old.members().members().len(), meter, path)?;
            require(
                old.members()
                    .members()
                    .contains(&PublicMemberRefV1::Callable(record.declaration())),
            )
        }
        NestedSourceSupportV1::Property(record) => {
            sequence(old.members().members().len(), meter, path)?;
            require(
                old.members()
                    .members()
                    .contains(&PublicMemberRefV1::Property(
                        PropertyDeclarationId::Property(record.declaration()),
                    )),
            )
        }
        NestedSourceSupportV1::NestedNominal(record) => {
            sequence(old.nested_bindings().values().len(), meter, path)?;
            let mut found = false;
            for binding in old.nested_bindings().values() {
                lookup(public.public_bindings().records().len(), meter, path)?;
                let binding = public
                    .public_bindings()
                    .get(*binding)
                    .ok_or(TypeSectionExportValidationError::PublicOverlap)?;
                found |= match (binding.source(), record.declaration()) {
                    (
                        ExportBindingSourceV1::DeclaredCurrent {
                            declaration: BindableEntity::Type(left),
                        },
                        SourceNominalId::Concrete(right),
                    ) => *left == right,
                    (
                        ExportBindingSourceV1::DeclaredCurrent {
                            declaration: BindableEntity::GenericType(left),
                        },
                        SourceNominalId::GenericTemplate(right),
                    ) => *left == right,
                    _ => false,
                };
            }
            sequence(source.children().values().len(), meter, path)?;
            require(found && source.children().values().contains(&record.declaration()))
        }
    }
}
