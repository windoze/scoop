use super::*;
use crate::shape_link::ShapeLinkSupportSourceV1;

impl<'a> ShapeLinkProviderV1<'a> {
    pub(super) fn support_contract(
        &self,
        subject: ExternalStrongShapeSubjectV1,
        physical: StrongShapeDefinitionRefV1,
        support: &dyn ShapeLinkSupportLookupV1<'a>,
    ) -> Result<ShapeLinkContractV1<'a>, ShapeLinkError> {
        use ExternalStrongShapeSubjectV1 as Subject;
        let source = support
            .support_source(self.provider(), subject)?
            .ok_or(ShapeLinkError::SupportRelation(subject))?;

        let unit = match source {
            ShapeLinkSupportSourceV1::Initialization { unit }
            | ShapeLinkSupportSourceV1::StaticStorage { unit, .. } => unit,
        };

        let registration = self
            .parts
            .production
            .initialization_registrations()
            .registrations()
            .iter()
            .find(|registration| registration.semantic().unit() == unit.unit())
            .ok_or(ShapeLinkError::SupportRelation(subject))?;
        // The hook names a relation, not a replacement payload. The contract
        // always borrows the actual provider's complete V2 semantic plan.

        if unit != registration.semantic() {
            return Err(ShapeLinkError::SupportRelation(subject));
        }
        match (subject, source) {
            (
                Subject::InitializationCell(id) | Subject::InitializationDescriptor(id),
                ShapeLinkSupportSourceV1::Initialization { .. },
            ) => {
                let (definition, symbol, atom) =
                    if matches!(subject, Subject::InitializationCell(_)) {
                        (
                            registration.cell_definition_plan(),
                            registration.cell_symbol(),
                            registration.cell_primary_atom(),
                        )
                    } else {
                        (
                            registration.descriptor_definition_plan(),
                            registration.descriptor_symbol(),
                            registration.descriptor_primary_atom(),
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
                    unit_projection: registration.semantic(),
                })
            }
            (
                Subject::StaticStorage(id) | Subject::StaticStorageRegistration(id),
                ShapeLinkSupportSourceV1::StaticStorage { storage, .. },
            ) => {
                if storage.storage() != id || (unit.storage() != id && unit.failure_root() != id) {
                    return Err(ShapeLinkError::SupportRelation(subject));
                }

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

                if storage != registration.semantic()
                    || definition != physical.definition()
                    || symbol != physical.symbol()
                    || atom != physical.primary()
                {
                    return Err(ShapeLinkError::DefinitionRelation(subject));
                }
                Ok(ShapeLinkContractV1::StaticStorage {
                    storage_projection: registration.semantic(),
                })
            }
            _ => Err(ShapeLinkError::SupportRelation(subject)),
        }
    }
}
