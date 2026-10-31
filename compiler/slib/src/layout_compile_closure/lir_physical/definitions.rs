use super::*;
use crate::ArtifactDefinitionData;

/// Shared semantic sections with saved definition metadata.
pub struct CrossConeLayoutArtifactData {
    physical: PhysicalImportsReplayedCrossConeLayoutClosure,
    symbols: Vec<ArtifactDefinitionData>,
    odr_definitions: MergedOdrDefinitions,
}

impl<'input> LirDependenciesResolvedCrossConeLayoutClosure<'input> {
    pub fn read_definition_data(
        self,
    ) -> Result<CrossConeLayoutArtifactData, CrossConeLayoutLirPhysicalError> {
        let selection = self.target;
        let (physical, symbols) = self.read_physical(|artifact| {
            let parts = artifact.prepared.semantic_parts();
            let link = parts.link_sections.ok_or_else(|| {
                SharedLirPhysicalError::DefinitionData("missing definition metadata".to_owned())
            })?;
            ArtifactDefinitionData::read(
                link,
                parts.manifest,
                parts.identities,
                selection,
                parts.lir_foundation,
            )
            .map_err(SharedLirPhysicalError::DefinitionData)
        })?;
        let odr_definitions = merge_cross_cone_odr_definitions(
            physical
                .dependency_first()
                .map(|artifact| artifact.identity_graph())
                .zip(&symbols)
                .map(|(identities, data)| (identities, data.defined_symbols(), data.odr_members())),
        )
        .map_err(|source| CrossConeLayoutLirPhysicalError {
            provider: source.provider(),
            source: Box::new(SharedLirPhysicalError::OdrDefinitions(Box::new(source))),
        })?;
        Ok(CrossConeLayoutArtifactData {
            physical,
            symbols,
            odr_definitions,
        })
    }
}

impl CrossConeLayoutArtifactData {
    pub(crate) fn into_parts(
        self,
    ) -> (
        ConeIdentity,
        lir::ValidatedLirTargetSelection,
        Vec<ConeIdentity>,
        Vec<(
            PhysicalImportsReplayedCrossConeLayoutSections,
            ArtifactDefinitionData,
        )>,
        MergedOdrDefinitions,
    ) {
        let PhysicalImportsReplayedCrossConeLayoutClosure {
            current,
            target,
            direct,
            artifacts,
            ..
        } = self.physical;
        (
            current,
            target,
            direct,
            artifacts.into_iter().zip(self.symbols).collect(),
            self.odr_definitions,
        )
    }
    pub const fn odr_definitions(&self) -> &MergedOdrDefinitions {
        &self.odr_definitions
    }

    pub const fn physical_imports(&self) -> &PhysicalImportsReplayedCrossConeLayoutClosure {
        &self.physical
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &PhysicalImportsReplayedCrossConeLayoutSections,
            &ArtifactDefinitionData,
        ),
    > {
        self.physical.dependency_first().zip(&self.symbols)
    }

    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<(
        &PhysicalImportsReplayedCrossConeLayoutSections,
        &ArtifactDefinitionData,
    )> {
        self.physical
            .positions
            .get(&provider)
            .map(|&index| (&self.physical.artifacts[index], &self.symbols[index]))
    }
}
