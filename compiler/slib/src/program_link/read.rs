use super::*;
use crate::production_dependencies::{self, DefinitionProvider};
use crate::{DecodedMachineLinkSections, ValidatedGraphArtifact};

pub fn read_program_link_closure(
    root: &[u8],
    dependencies: &[&[u8]],
    selection: lir::ValidatedLirTargetSelection,
    profile: &lir::CBridgeToolchainProfileV1,
) -> Result<ProgramLinkClosure, ProgramLinkReadError> {
    let graph = super::graph::read(root, dependencies, selection)?;
    let mut artifacts = Vec::with_capacity(graph.dependency_first.len());
    let mut symbols = Vec::with_capacity(graph.dependency_first.len());
    for (position, input) in graph.dependency_first.into_iter().enumerate() {
        let identity = input.identity();
        let coordinate = input.coordinate().clone();
        let reachable = crate::dependency_reachability::transitive_positions(
            position,
            &graph.dependency_positions,
        )
        .map_err(error)?;
        let (artifact, symbol) =
            read_artifact(input, &artifacts, &symbols, &reachable, selection, profile)
                .map_err(|err| err.context(identity, &coordinate))?;
        artifacts.push(artifact);
        symbols.push(symbol);
    }
    let odr = crate::merge_cross_cone_odr_definitions(
        artifacts
            .iter()
            .map(|artifact| &artifact.identities)
            .zip(&symbols),
    )
    .map_err(error)?;
    Ok(ProgramLinkClosure {
        root: graph.current,
        artifacts,
        symbols,
        odr,
    })
}

fn read_artifact(
    graph: ValidatedGraphArtifact<'_>,
    previous: &[ProgramLinkArtifact],
    previous_symbols: &[ReplayedLayoutLinkSymbolUsesV1],
    reachable: &[usize],
    selection: lir::ValidatedLirTargetSelection,
    profile: &lir::CBridgeToolchainProfileV1,
) -> Result<(ProgramLinkArtifact, ReplayedLayoutLinkSymbolUsesV1), ProgramLinkReadError> {
    let DecodedMachineLinkSections {
        mut graph,
        hir_keys,
        mir_keys,
        foundation,
        production,
        ordinary,
        layout,
        link,
    } = graph
        .decode_machine_link_sections()
        .map_err(super::error::section)?;
    let dependencies = reachable
        .iter()
        .map(|&index| &previous[index])
        .collect::<Vec<_>>();
    let mut identities =
        crate::compile_decode::validate_foundation_identity_graph_with_authorities(
            &mut graph,
            &hir_keys,
            &mir_keys,
            &foundation,
            dependencies.iter().map(|dependency| &dependency.identities),
        )
        .map_err(error)?;
    let foundation = lir::ConeLirFoundation::from_validated(
        foundation
            .validate(graph.identity(), &mut identities)
            .map_err(error)?,
    );
    let exports = dependencies
        .iter()
        .map(|dependency| dependency.layout.exports())
        .collect::<Vec<_>>();
    let layout = layout
        .read_link_layouts(
            selection.target(),
            &foundation,
            &mut identities,
            &exports
                .iter()
                .map(|exports| exports.layouts())
                .collect::<Vec<_>>(),
        )
        .map_err(error)?;
    let inputs = dependencies
        .iter()
        .map(|dependency| DefinitionProvider {
            identities: &dependency.identities,
            foundation: &dependency.foundation,
            strong: &dependency.production,
            layouts: dependency.layout.exports().layouts(),
        })
        .collect::<Vec<_>>();
    let (types, units) =
        production_dependencies::definitions(graph.identity(), &inputs).map_err(error)?;
    let shape_sources = production
        .link_shape_sources(&mut identities)
        .map_err(error)?;
    let direct = graph
        .direct_dependencies()
        .iter()
        .map(crate::DependencyRecord::identity)
        .collect::<Vec<_>>();
    let production = production
        .read_link(
            graph.coordinate().clone(),
            &direct,
            selection.target(),
            &foundation,
            &mut identities,
            &shape_sources,
            &types,
            &units,
        )
        .map_err(error)?;
    let ordinary = ordinary
        .validate(&mut identities, &foundation)
        .map_err(error)?;
    let coordinates = std::iter::once(graph.coordinate().clone())
        .chain(
            dependencies
                .iter()
                .map(|dependency| dependency.manifest.cone().coordinate().clone()),
        )
        .collect::<Vec<_>>();
    let layout = layout
        .read_link_exports(
            selection.target(),
            &foundation,
            &mut identities,
            &production,
            &ordinary,
            &shape_sources,
            &coordinates,
            &exports,
        )
        .map_err(error)?
        .resolve_dependencies(&mut identities)
        .map_err(error)?;
    let providers = dependencies
        .iter()
        .map(|dependency| {
            let exports = dependency.layout.exports();
            lir::ShapeLinkProviderV1::try_new(lir::ShapeLinkProviderPartsV1 {
                foundation: &dependency.foundation,
                production: &dependency.production,
                ordinary: &dependency.ordinary,
                layouts: exports.layouts(),
                callables: exports.callables(),
                descriptors: exports.descriptors(),
                dispatch: exports.dispatch(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    let layout = layout
        .replay_physical_imports(&providers, &mut identities)
        .map_err(error)?;
    production
        .validate_layout_selection(&layout)
        .map_err(error)?;
    let objects =
        crate::layout_link_objects::replay(&link, &mut graph, &foundation, &production, profile)
            .map_err(|source| error(format!("{source:?}")))?;
    let code_strong =
        crate::link_decode::layout_code_strong_input(&mut graph).map_err(super::error::section)?;
    let symbols = crate::layout_link_symbols::replay(
        objects,
        crate::layout_link_symbols::ReplayInputs {
            link: &link,
            foundation: &foundation,
            strong: &production,
            ordinary: &ordinary,
            layout: &layout,
            selection,
            profile,
            manifest: graph.envelope.manifest(),
            code_strong: &code_strong,
        },
        previous_symbols,
        previous.iter().map(|artifact| &artifact.layout),
        reachable,
    )
    .map_err(super::error::symbols)?;
    Ok((
        ProgramLinkArtifact {
            manifest: graph.envelope.into_manifest(),
            identities,
            foundation,
            production,
            ordinary,
            layout,
        },
        symbols,
    ))
}
