//! Extend shared HIR records only for providers selected by the current output.

use scoop_identity::PendingIdentityValidation;
use scoop_lir as lir;
use scoop_mir as mir;

use super::*;
use crate::HirProductionValidatedCrossConeLayoutSections;
use crate::compile_sections::{decode_compile_stage, decode_stage_section};

impl CompileDependencyClosure<'_> {
    pub fn read_machine_dependencies(
        &self,
        roots: impl IntoIterator<Item = ConeIdentity>,
    ) -> Result<crate::ValidatedCrossConeArtifactClosure, Error> {
        let mut selected = vec![false; self.providers.len()];
        let mut pending = Vec::new();
        for root in roots {
            if root == self.current {
                continue;
            }
            pending.push(*self.positions.get(&root).ok_or_else(|| {
                semantic(
                    crate::CrossConeClosureGraphError::MissingDependencyArtifact {
                        dependent: self.current,
                        dependency: root,
                    },
                )
            })?);
        }
        while let Some(position) = pending.pop() {
            if !std::mem::replace(&mut selected[position], true) {
                pending.extend(&self.dependencies[position]);
            }
        }
        let mut remap = BTreeMap::new();
        let mut artifacts = Vec::new();
        let mut dependencies = Vec::new();
        let mut aliases = Vec::new();
        for (position, provider) in self.providers.iter().enumerate() {
            if !selected[position] {
                continue;
            }
            let direct = self.dependencies[position]
                .iter()
                .map(|index| remap[index])
                .collect::<Vec<_>>();
            let artifact = read_machine(provider, position, &artifacts, &direct)?;
            remap.insert(position, artifacts.len());
            artifacts.push(artifact);
            dependencies.push(direct);
            aliases.push(Rc::clone(&provider.aliases));
        }
        let direct = self
            .direct
            .iter()
            .copied()
            .filter(|identity| selected[self.positions[identity]])
            .collect();
        let machine = crate::HirProductionValidatedCrossConeLayoutClosure::from_hir_dependencies(
            self.current,
            self.target,
            direct,
            artifacts,
            dependencies,
            aliases,
        )
        .read_machine_sections()
        .map_err(semantic)?
        .read_definition_data()
        .map_err(|source| {
            layout(LayoutError::Physical {
                source: Box::new(source),
            })
        })?;
        crate::cross_cone_closure::commit_artifact_data(machine, &mut self.session.borrow_mut())
    }
}

fn read_machine<'input>(
    provider: &HirDependency<'input>,
    slot: usize,
    artifacts: &[HirProductionValidatedCrossConeLayoutSections<'input>],
    direct: &[usize],
) -> Result<HirProductionValidatedCrossConeLayoutSections<'input>, Error> {
    let mut graph = provider.graph.clone();
    let profile = crate::ArtifactCapabilityProfile::CROSS_CONE_GENERIC;
    let mir_location = crate::MetadataLocation::Mir;
    let lir_location = crate::MetadataLocation::Lir;
    let decode = || -> Result<_, crate::CompileSectionDecodeError> {
        let mir = decode_compile_stage(&graph, profile, mir_location)?;
        let lir = decode_compile_stage(&graph, profile, lir_location)?;
        Ok((
            decode_stage_section::<mir::DecodedMirFoundation>(
                &mir,
                mir_location,
                crate::mir_identity_foundation_capability(),
            )?,
            decode_stage_section(
                &mir,
                mir_location,
                crate::mir_core_bootstrap_bridge_capability(),
            )?,
            decode_stage_section(
                &mir,
                mir_location,
                crate::mir_cross_cone_param_free_bridge_capability(),
            )?,
            decode_stage_section(
                &mir,
                mir_location,
                crate::mir_cross_cone_type_bridge_capability(),
            )?,
            decode_stage_section::<lir::DecodedLirFoundation>(
                &lir,
                lir_location,
                crate::lir_identity_foundation_capability(),
            )?,
            decode_stage_section(&lir, lir_location, crate::lir_cone_production_capability())?,
            decode_stage_section(
                &lir,
                lir_location,
                crate::lir_cross_cone_param_free_bridge_capability(),
            )?,
            decode_stage_section(
                &lir,
                lir_location,
                crate::lir_cross_cone_layout_abi_capability(),
            )?,
            lir,
        ))
    };
    let (
        mir_foundation,
        mir_core_production,
        mir_cross_cone_bridge,
        mir_type_bridge,
        lir_foundation,
        lir_strong_production,
        lir_cross_cone_bridge,
        lir_layout_abi,
        lir,
    ) = decode().map_err(|source| sections(slot, source))?;
    let link = graph
        .decode_layout_link_only_sections(&lir)
        .map_err(|source| {
            layout(LayoutError::LinkSections {
                slot: Slot::Dependency(slot),
                source: Box::new(source),
            })
        })?;
    let identity = graph.identity();
    let identities = || -> Result<_, scoop_identity::IdentityValidationError> {
        let mut pending = PendingIdentityValidation::from_graph(provider.identities.clone());
        mir_foundation.register_identities(&mut pending)?;
        lir_foundation.register_identities(&mut pending)?;
        for &index in direct {
            pending.register_external_graph_authorities(&artifacts[index].identities)?;
        }
        mir_foundation.resolve_identities(&mut pending)?;
        lir_foundation.resolve_identities(&mut pending)?;
        pending.finish()
    };
    let mut identities = identities().map_err(|source| {
        semantic(crate::CrossConeClosureIdentityError::Artifact { identity, source })
    })?;
    let mir_foundation = mir_foundation
        .validate(&mut identities)
        .map_err(|source| {
            semantic(crate::CrossConeClosureFoundationError::Artifact {
                identity,
                source: Box::new(crate::StrongProfileFoundationError::MirStructure(source)),
            })
        })?
        .into_canonical();
    let lir_foundation = lir_foundation
        .validate(identity, &mut identities)
        .map_err(|source| {
            semantic(crate::CrossConeClosureFoundationError::Artifact {
                identity,
                source: Box::new(crate::StrongProfileFoundationError::LirStructure(source)),
            })
        })?;
    Ok(HirProductionValidatedCrossConeLayoutSections {
        graph,
        view: crate::link_decode::DecodedLayoutView::Link(Box::new(link)),
        identities,
        foundations: crate::strong_compile_decode::CanonicalFoundationSet {
            hir: Rc::clone(&provider.foundation),
            mir: Rc::new(mir_foundation),
            lir: lir::ConeLirFoundation::from_validated(lir_foundation),
        },
        hir_core_production: Rc::clone(&provider.core),
        hir_interface: Rc::clone(&provider.interface),
        hir_type_semantics: Rc::clone(&provider.types),
        mir_core_production,
        mir_cross_cone_bridge,
        mir_type_bridge,
        lir_strong_production,
        lir_cross_cone_bridge,
        lir_layout_abi,
    })
}
