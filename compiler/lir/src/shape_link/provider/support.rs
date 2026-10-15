use super::*;

impl ShapeLinkProviderV1<'_> {
    pub(super) fn support_contract(
        &self,
        subject: ExternalStrongShapeSubjectV1,
    ) -> Result<ShapeLinkContractV1, ShapeLinkError> {
        use ExternalStrongShapeSubjectV1 as Subject;
        match subject {
            Subject::InitializationCell(id) | Subject::InitializationRegistration(id) => {
                let unit = self
                    .parts
                    .initialization
                    .registrations()
                    .iter()
                    .find(|registration| registration.semantic().unit() == id)
                    .ok_or(ShapeLinkError::SupportRelation(subject))?;
                Ok(ShapeLinkContractV1::Initialization {
                    unit_projection: unit.semantic().clone(),
                })
            }
            Subject::StaticStorage(id) | Subject::StaticStorageRegistration(id) => {
                let storage = self
                    .parts
                    .static_storages
                    .registrations()
                    .iter()
                    .find(|registration| registration.semantic().storage() == id)
                    .ok_or(ShapeLinkError::SupportRelation(subject))?;
                Ok(ShapeLinkContractV1::StaticStorage {
                    storage_projection: storage.semantic().clone(),
                })
            }
            _ => Err(ShapeLinkError::SupportRelation(subject)),
        }
    }
}
