//! Decode and resolve the HIR portion of an existing dependency artifact.

use scoop_identity::PendingIdentityValidation;
use scoop_wire::WirePath;

use super::*;
use crate::compile_sections::{
    decode_compile_stage, decode_stage_section, validate_compile_profile,
};

pub(super) fn read_hir<'input>(
    graph: ValidatedGraphArtifact<'input>,
    slot: usize,
    providers: &[HirDependency<'input>],
    direct: &[usize],
    reachable: &[usize],
) -> Result<HirDependency<'input>, Error> {
    let profile = crate::ArtifactCapabilityProfile::CROSS_CONE_GENERIC;
    let location = crate::MetadataLocation::Hir;
    let decode = || -> Result<_, crate::CompileSectionDecodeError> {
        validate_compile_profile(&graph, profile)?;
        let metadata = decode_compile_stage(&graph, profile, location)?;
        Ok((
            decode_stage_section::<hir::DecodedHirFoundation>(
                &metadata,
                location,
                crate::hir_identity_foundation_capability(),
            )?,
            decode_stage_section::<hir::DecodedCoreBootstrapInterfaceSectionV1>(
                &metadata,
                location,
                crate::hir_core_bootstrap_interface_capability(),
            )?,
            decode_stage_section::<hir::DecodedCrossConeHirInterfaceSectionV1>(
                &metadata,
                location,
                crate::hir_cross_cone_interface_capability(),
            )?,
            decode_stage_section::<hir::DecodedCrossConeTypeSemanticsSectionV1>(
                &metadata,
                location,
                crate::hir_cross_cone_type_semantics_capability(),
            )?,
        ))
    };
    let (foundation, core, interface, types) = decode().map_err(|source| sections(slot, source))?;
    let provider = graph.identity();
    let identities = || -> Result<_, scoop_identity::IdentityValidationError> {
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(provider)?;
        for dependency in graph.direct_dependencies() {
            pending.register_authority(dependency.identity())?;
        }
        foundation.register_identities(&mut pending)?;
        for &dependency in direct {
            pending.register_external_graph_authorities(&providers[dependency].identities)?;
        }
        foundation.resolve_identities(&mut pending)?;
        pending.finish()
    };
    let mut identities = identities().map_err(|source| {
        semantic(crate::CrossConeClosureIdentityError::Artifact {
            identity: provider,
            source,
        })
    })?;
    let foundation = foundation
        .validate_with_dependency_sources(
            graph.coordinate(),
            &mut identities,
            &reachable
                .iter()
                .map(|&index| providers[index].foundation.as_ref())
                .collect::<Vec<_>>(),
        )
        .map_err(|source| {
            semantic(crate::CrossConeClosureFoundationError::Artifact {
                identity: provider,
                source: Box::new(crate::StrongProfileFoundationError::HirStructure(source)),
            })
        })?
        .into_canonical();
    let interface = interface.resolve(&mut identities).map_err(|source| {
        semantic(crate::CrossConeLayoutClosureHirResolutionError::Artifact {
            identity: provider,
            source: Box::new(crate::CrossConeLayoutHirResolutionError::Public(Box::new(
                source,
            ))),
        })
    })?;
    let types = types
        .resolve(&mut identities, &WirePath::root())
        .map_err(|source| {
            semantic(crate::CrossConeLayoutClosureHirResolutionError::Artifact {
                identity: provider,
                source: Box::new(crate::CrossConeLayoutHirResolutionError::TypeSemantics(
                    Box::new(source),
                )),
            })
        })?;
    let core = core
        .validate_against(provider, &foundation)
        .map_err(|source| {
            semantic(crate::CrossConeClosureHirProductionError::Artifact {
                identity: provider,
                source,
            })
        })?;
    let aliases = || -> Result<_, crate::CrossConeHirDeclarationValidationError> {
        crate::cross_cone_closure::validate_alias_targets(provider, &interface, &identities)
            .map_err(crate::CrossConeHirDeclarationValidationError::AliasReferences)?;
        interface
            .type_aliases()
            .expand_alias_closure(
                &reachable
                    .iter()
                    .map(|&index| providers[index].aliases.as_ref())
                    .collect::<Vec<_>>(),
                &WirePath::root().field(5),
            )
            .map_err(crate::CrossConeHirDeclarationValidationError::AliasExpansion)
    };
    let aliases = aliases().map_err(|source| {
        semantic(crate::CrossConeLayoutHirDeclarationError {
            provider,
            source: Box::new(source),
        })
    })?;
    Ok(HirDependency {
        graph,
        identities,
        foundation: Rc::new(foundation),
        core: Rc::new(core),
        interface: Rc::new(interface),
        types: Rc::new(types),
        aliases: Rc::new(aliases),
    })
}
