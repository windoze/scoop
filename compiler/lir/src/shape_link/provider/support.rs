use super::*;
use scoop_identity::{PersistentInitializationUnitId, PersistentStaticStorageId};

pub enum ShapeLinkSupportV1<'a> {
    Produced {
        static_storages: &'a StrongStaticStorageRegistrationPlanSetV1,
        initialization: &'a StrongInitializationUnitRegistrationPlanSetV2,
    },
    Saved(&'a CompileProduction),
}

impl ShapeLinkSupportV1<'_> {
    pub(super) fn providers(&self) -> [ConeIdentity; 2] {
        match self {
            Self::Produced {
                static_storages,
                initialization,
            } => [static_storages.producer(), initialization.producer()],
            Self::Saved(data) => [data.provider(); 2],
        }
    }
    fn initialization(
        &self,
        id: PersistentInitializationUnitId,
    ) -> Option<&StrongInitializationUnitSemanticPlanV2> {
        match self {
            Self::Produced { initialization, .. } => initialization
                .registrations()
                .iter()
                .map(|record| record.semantic())
                .find(|unit| unit.unit() == id),
            Self::Saved(data) => data
                .initialization_units()
                .iter()
                .map(|record| record.semantic())
                .find(|unit| unit.unit() == id),
        }
    }
    fn static_storage(
        &self,
        id: PersistentStaticStorageId,
    ) -> Option<&StrongStaticStorageSemanticPlanV1> {
        match self {
            Self::Produced {
                static_storages, ..
            } => static_storages
                .registrations()
                .iter()
                .map(|record| record.semantic())
                .find(|storage| storage.storage() == id),
            Self::Saved(data) => data
                .static_storages()
                .iter()
                .find(|storage| storage.storage() == id),
        }
    }
}

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
                    .support
                    .initialization(id)
                    .ok_or(ShapeLinkError::SupportRelation(subject))?;
                Ok(ShapeLinkContractV1::Initialization {
                    unit_projection: unit.clone(),
                })
            }
            Subject::StaticStorage(id) | Subject::StaticStorageRegistration(id) => {
                let storage = self
                    .parts
                    .support
                    .static_storage(id)
                    .ok_or(ShapeLinkError::SupportRelation(subject))?;
                Ok(ShapeLinkContractV1::StaticStorage {
                    storage_projection: storage.clone(),
                })
            }
            _ => Err(ShapeLinkError::SupportRelation(subject)),
        }
    }
}
