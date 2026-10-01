use super::*;

impl<'a> ShapeLinkProviderV1<'a> {
    pub(super) fn support_contract(
        &self,
        subject: ExternalStrongShapeSubjectV1,
        physical: StrongShapeDefinitionRefV1,
    ) -> Result<ShapeLinkContractV1, ShapeLinkError> {
        use ExternalStrongShapeSubjectV1 as Subject;
        let registration =
            self.parts
                .production
                .initialization_registrations()
                .registrations()
                .iter()
                .find(|registration| {
                    let unit = registration.semantic();
                    match subject {
                        Subject::InitializationCell(id)
                        | Subject::InitializationRegistration(id) => unit.unit() == id,
                        Subject::StaticStorage(id) | Subject::StaticStorageRegistration(id) => {
                            unit.storage() == id || unit.failure_root() == id
                        }
                        _ => false,
                    }
                })
                .ok_or(ShapeLinkError::SupportRelation(subject))?;
        let unit = registration.semantic();
        match subject {
            Subject::InitializationCell(id) | Subject::InitializationRegistration(id) => {
                let (definition, symbol, atom) =
                    if matches!(subject, Subject::InitializationCell(_)) {
                        (
                            registration.cell_definition_plan(),
                            registration.cell_symbol(),
                            registration.cell_primary_atom(),
                        )
                    } else {
                        (
                            registration.registration_definition_plan(),
                            registration.registration_symbol(),
                            registration.registration_primary_atom(),
                        )
                    };
                if id != unit.unit()
                    || definition != physical.definition()
                    || symbol != physical.symbol()
                    || atom != physical.primary()
                {
                    return Err(ShapeLinkError::DefinitionRelation(subject));
                }
                Ok(ShapeLinkContractV1::Initialization {
                    unit_projection: registration.semantic().clone(),
                })
            }
            Subject::StaticStorage(id) | Subject::StaticStorageRegistration(id) => {
                let registration = self
                    .parts
                    .production
                    .static_storage_registrations()
                    .registrations()
                    .iter()
                    .find(|registration| registration.semantic().storage() == id)
                    .ok_or(ShapeLinkError::SupportRelation(subject))?;
                let (definition, symbol, atom) = if matches!(subject, Subject::StaticStorage(_)) {
                    (
                        registration.storage_definition_plan(),
                        registration.semantic().symbol(),
                        registration.storage_primary_atom(),
                    )
                } else {
                    (
                        registration.registration_definition_plan(),
                        registration.registration_symbol(),
                        registration.registration_primary_atom(),
                    )
                };

                if definition != physical.definition()
                    || symbol != physical.symbol()
                    || atom != physical.primary()
                {
                    return Err(ShapeLinkError::DefinitionRelation(subject));
                }
                Ok(ShapeLinkContractV1::StaticStorage {
                    storage_projection: registration.semantic().clone(),
                })
            }
            _ => Err(ShapeLinkError::SupportRelation(subject)),
        }
    }
}
