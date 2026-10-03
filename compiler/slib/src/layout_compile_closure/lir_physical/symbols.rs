use super::*;
use crate::{ReplayedLayoutLinkSymbolUsesV1, layout_link_symbols};

/// Symbol uses and provider owners from the same owned physical replay.
pub struct LinkSymbolsReplayedCrossConeLayoutClosure {
    physical: PhysicalImportsReplayedCrossConeLayoutClosure,
    symbols: Vec<ReplayedLayoutLinkSymbolUsesV1>,
    odr_definitions: MergedOdrDefinitions,
}

impl<'input> LirDependencyGraphReplayedCrossConeLayoutClosure<'input> {
    pub fn replay_link_symbol_uses(
        self,
        profile: &lir::CBridgeToolchainProfileV1,
    ) -> Result<LinkSymbolsReplayedCrossConeLayoutClosure, CrossConeLayoutLirPhysicalError> {
        let selection = self.target;
        let (physical, symbols) =
            self.replay_physical(|artifact, reachable, previous, physical| {
                let objects = artifact
                    .prepared
                    .replay_link_object_contents(&artifact.strong, profile)?;
                let code_strong = artifact.prepared.code_strong_input()?;
                let parts = artifact.prepared.semantic_parts();
                let link = parts
                    .link_sections
                    .ok_or(crate::LayoutLinkObjectContentsError::CompileView)?;
                let symbols = layout_link_symbols::replay(
                    objects,
                    layout_link_symbols::ReplayInputs {
                        link,
                        foundation: parts.lir_foundation,
                        strong: &artifact.strong,
                        ordinary: &artifact.ordinary,
                        layout: &artifact.layout,
                        selection,
                        profile,
                        manifest: parts.manifest,
                        code_strong: &code_strong,
                    },
                    previous,
                    physical.iter().map(|artifact| &artifact.layout),
                    reachable,
                )?;
                Ok(symbols)
            })?;
        let odr_definitions = merge_cross_cone_odr_definitions(
            physical
                .dependency_first()
                .map(|artifact| artifact.identity_graph())
                .zip(&symbols),
        )
        .map_err(|source| CrossConeLayoutLirPhysicalError {
            provider: source.provider(),
            source: Box::new(SharedLirPhysicalError::OdrDefinitions(Box::new(source))),
        })?;
        Ok(LinkSymbolsReplayedCrossConeLayoutClosure {
            physical,
            symbols,
            odr_definitions,
        })
    }
}

impl LinkSymbolsReplayedCrossConeLayoutClosure {
    pub(crate) fn into_parts(
        self,
    ) -> (
        ConeIdentity,
        lir::ValidatedLirTargetSelection,
        Vec<ConeIdentity>,
        Vec<(
            PhysicalImportsReplayedCrossConeLayoutSections,
            ReplayedLayoutLinkSymbolUsesV1,
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
            &ReplayedLayoutLinkSymbolUsesV1,
        ),
    > {
        self.physical.dependency_first().zip(&self.symbols)
    }

    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<(
        &PhysicalImportsReplayedCrossConeLayoutSections,
        &ReplayedLayoutLinkSymbolUsesV1,
    )> {
        self.physical
            .positions
            .get(&provider)
            .map(|&index| (&self.physical.artifacts[index], &self.symbols[index]))
    }
}
