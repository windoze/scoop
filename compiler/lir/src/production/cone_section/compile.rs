//! Saved production inputs used while compiling a dependent Cone.
use super::*;
use crate::{
    CanonicalExactLayoutExportsV1, LinkDataError, StrongInitializationUnitDefinitionRefV2,
    StrongInitializationUnitSemanticPlanV2, StrongStaticStorageSemanticPlanV1,
};
use scoop_identity::PersistentInitializationUnitId;

pub struct CompileInitializationUnit {
    pub(in crate::production) semantic: StrongInitializationUnitSemanticPlanV2,
    pub(in crate::production) definition: StrongInitializationUnitDefinitionRefV2,
}

impl CompileInitializationUnit {
    pub fn semantic(&self) -> &StrongInitializationUnitSemanticPlanV2 {
        &self.semantic
    }
    pub fn definition(&self) -> StrongInitializationUnitDefinitionRefV2 {
        self.definition
    }
}

pub struct CompileProduction {
    provider: ConeIdentity,
    definitions: ObjectSymbolSurfaceV1,
    static_storages: Vec<StrongStaticStorageSemanticPlanV1>,
    initialization: Vec<CompileInitializationUnit>,
}

impl CompileProduction {
    pub fn provider(&self) -> ConeIdentity {
        self.provider
    }
    pub fn canonical_definitions(&self) -> &ObjectSymbolSurfaceV1 {
        &self.definitions
    }
    pub fn static_storages(&self) -> &[StrongStaticStorageSemanticPlanV1] {
        &self.static_storages
    }
    pub fn initialization_units(&self) -> &[CompileInitializationUnit] {
        &self.initialization
    }
    pub fn initialization_definition(
        &self,
        unit: PersistentInitializationUnitId,
    ) -> Option<StrongInitializationUnitDefinitionRefV2> {
        self.initialization
            .iter()
            .find(|record| record.semantic.unit() == unit)
            .map(|record| record.definition)
    }
}

impl DecodedConeProductionSectionV2 {
    pub fn read_compile(
        self,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
        layouts: &[&CanonicalExactLayoutExportsV1],
        dependencies: &[&CompileProduction],
    ) -> Result<CompileProduction, LinkDataError> {
        let definitions = ObjectSymbolSurfaceV1::read_foundation(foundation)?;
        let (static_storages, initialization) = self.registration_production.read_compile(
            foundation,
            identities,
            layouts,
            dependencies,
        )?;
        Ok(CompileProduction {
            provider: foundation.producer(),
            definitions,
            static_storages,
            initialization,
        })
    }
}
