use super::*;
use crate::{ReplayedLayoutLinkSymbolUsesV1, layout_link_symbols};

/// Symbol uses and provider owners from the same owned physical replay.
pub struct LinkSymbolsReplayedCrossConeLayoutClosure {
    physical: PhysicalImportsReplayedCrossConeLayoutClosure,
    symbols: Vec<ReplayedLayoutLinkSymbolUsesV1>,
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
                        hir_foundation: parts.hir_foundation,
                    },
                    previous,
                    physical.iter().map(|artifact| &artifact.layout),
                    reachable,
                )?;
                Ok(symbols)
            })?;
        Ok(LinkSymbolsReplayedCrossConeLayoutClosure { physical, symbols })
    }
}

impl LinkSymbolsReplayedCrossConeLayoutClosure {
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
