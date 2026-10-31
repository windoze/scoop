use super::*;
use crate::{DecodedMachineLinkSections, ValidatedGraphArtifact};

pub fn read_program_link_closure(
    root: &crate::ArtifactFile,
    dependencies: &[&crate::ArtifactFile],
    selection: lir::ValidatedLirTargetSelection,
) -> Result<ProgramLinkClosure, ProgramLinkReadError> {
    let graph = super::graph::read(root, dependencies, selection)?;
    let mut files = std::collections::BTreeMap::new();
    for file in std::iter::once(root).chain(dependencies.iter().copied()) {
        let identity = file.directory().map_err(error)?.manifest.cone().identity();
        files.entry(identity).or_insert(file);
    }
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
        let (artifact, symbol) = read_artifact(
            input,
            (*files[&identity]).clone(),
            &artifacts,
            &reachable,
            selection,
        )
        .map_err(|err| err.context(identity, &coordinate))?;
        artifacts.push(artifact);
        symbols.push(symbol);
    }
    let odr = crate::merge_cross_cone_odr_definitions(artifacts.iter().zip(&symbols).map(
        |(artifact, symbols)| {
            (
                &artifact.identities,
                symbols.defined_symbols(),
                symbols.odr_members(),
                artifact.manifest.direct_dependencies(),
            )
        },
    ))
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
    source: crate::ArtifactFile,
    previous: &[ProgramLinkArtifact],
    reachable: &[usize],
    selection: lir::ValidatedLirTargetSelection,
) -> Result<(ProgramLinkArtifact, ProgramLinkData), ProgramLinkReadError> {
    let DecodedMachineLinkSections {
        mut graph,
        hir_keys,
        mir_keys,
        foundation,
        production,
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
    let production = production
        .read_link(&foundation, &mut identities)
        .map_err(error)?;
    let native_values = layout.read_link_values(&mut identities).map_err(error)?;
    let symbols = ProgramLinkData::read(
        &link,
        graph.envelope.manifest(),
        &mut identities,
        &foundation,
        selection,
    )
    .map_err(error)?;
    let native_objects = super::native::read(&mut graph, selection.target())?;
    Ok((
        ProgramLinkArtifact {
            manifest: graph.envelope.into_manifest(),
            source,
            identities,
            native_cxx: foundation.native_cxx(),
            production,
            native_values,
            native_objects,
        },
        symbols,
    ))
}
