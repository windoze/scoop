use super::*;
use crate::{ReplayedLayoutLinkSymbolUsesV1, layout_link_symbols};

/// Symbol uses and provider owners from the same owned physical replay.
pub struct LinkSymbolsReplayedCrossConeLayoutClosure<'checked, 'input> {
    physical: PhysicalImportsReplayedCrossConeLayoutClosure<'checked, 'input>,
    symbols: Vec<ReplayedLayoutLinkSymbolUsesV1<'input>>,
}

impl<'input> LirDependencyGraphReplayedCrossConeLayoutClosure<'input> {
    pub fn with_replayed_link_symbol_uses<R>(
        self,
        profile: &lir::CBridgeToolchainProfileV1,
        use_checked: impl for<'checked> FnOnce(
            LinkSymbolsReplayedCrossConeLayoutClosure<'checked, 'input>,
        ) -> R,
    ) -> Result<R, CrossConeLayoutLirPhysicalError> {
        let selection = self.target;
        self.with_replayed_physical(
            |artifact, reachable, previous, physical| {
                let objects = artifact
                    .prepared
                    .replay_link_object_contents(&artifact.strong, profile)?;
                let code_strong = artifact.prepared.code_strong_input()?;
                let parts = artifact.prepared.semantic_parts();
                let link = parts
                    .link_sections
                    .ok_or(crate::LayoutLinkObjectContentsError::CompileView)?;
                Ok(layout_link_symbols::replay(
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
                    parts.meter,
                )?)
            },
            |physical, symbols| {
                use_checked(LinkSymbolsReplayedCrossConeLayoutClosure { physical, symbols })
            },
        )
    }
}

impl<'checked, 'input> LinkSymbolsReplayedCrossConeLayoutClosure<'checked, 'input> {
    pub const fn physical_imports(
        &self,
    ) -> &PhysicalImportsReplayedCrossConeLayoutClosure<'checked, 'input> {
        &self.physical
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'checked PhysicalImportsReplayedCrossConeLayoutSections<'input, 'checked>,
            &ReplayedLayoutLinkSymbolUsesV1<'input>,
        ),
    > {
        self.physical.dependency_first().zip(&self.symbols)
    }

    pub fn artifact(
        &self,
        provider: ConeIdentity,
    ) -> Option<(
        &'checked PhysicalImportsReplayedCrossConeLayoutSections<'input, 'checked>,
        &ReplayedLayoutLinkSymbolUsesV1<'input>,
    )> {
        self.physical
            .positions
            .get(&provider)
            .map(|&index| (self.physical.artifacts[index], &self.symbols[index]))
    }
}
