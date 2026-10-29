use super::*;
use crate::{
    CompletedNode, PreparedNodeRepresentation, ProductionSingleConeCompilerRunner,
    ResolvedBuildGraph, SingleConeCompilerRunner,
};
use scoop_protocol::{RequestCorrelationId, ScoopcResponseEnvelopeV1};

pub(super) fn graph(
    graph: ResolvedBuildGraph,
    request: &DocumentRequest,
    sources: &BTreeMap<ConeIdentity, plan::Source>,
    selected: &BTreeSet<ConeIdentity>,
    compiled: &mut Vec<ConeIdentity>,
) -> BuildResult<()> {
    let mut needed = BTreeSet::new();
    for id in graph.dependency_first() {
        if graph.source_manifest(*id).is_some()
            && selected.contains(id)
            && !cache::Entry::new(&request.cache, &sources[id].key).ready()?
        {
            needed.insert(*id);
        }
    }
    if needed.is_empty() {
        return Ok(());
    }
    let mut artifacts = BTreeSet::new();
    let mut pending = needed.iter().copied().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        for edge in graph.edges().filter(|edge| edge.dependent() == id) {
            if artifacts.insert(edge.dependency()) {
                pending.push(edge.dependency());
            }
        }
    }
    let mut prepared = graph
        .prepare_frontend()
        .map_err(BuildFailure::preparation)?;
    let mut completed = BTreeMap::<ConeIdentity, CompletedNode>::new();
    let runner = ProductionSingleConeCompilerRunner;
    for (position, identity) in prepared.dependency_first().to_vec().into_iter().enumerate() {
        if !needed.contains(&identity) && !artifacts.contains(&identity) {
            continue;
        }
        let inputs = completed.values().collect::<Vec<_>>();
        let request_id = RequestCorrelationId::from_array((position as u128 + 1).to_be_bytes());
        let result = (|| {
            if prepared.node_representation(identity)
                == Some(PreparedNodeRepresentation::PrebuiltArtifact)
            {
                let node = prepared
                    .complete_prebuilt_node(identity, &inputs)
                    .map_err(error)?;
                return Ok(Some(node));
            }
            let entry = cache::Entry::new(&request.cache, &sources[&identity].key);
            let _lock = needed
                .contains(&identity)
                .then(|| entry.lock())
                .transpose()?;
            let documentation =
                (needed.contains(&identity) && !entry.ready()?).then(|| entry.input());
            let (node, invoked, documented) = if artifacts.contains(&identity) {
                prepared
                    .prepare_machine_source(identity)
                    .map_err(BuildFailure::preparation)?;
                let (node, invoked, result) = prepared
                    .execute_source_output(
                        identity,
                        &inputs,
                        &runner,
                        request_id,
                        documentation.as_deref(),
                    )
                    .map_err(|cause| {
                        let mut failure = error(&cause);
                        if let crate::OrdinarySourceExecutionError::ChildFailure(diagnostics) =
                            &cause
                        {
                            failure.diagnostics = diagnostics.clone();
                        }
                        failure
                    })?;
                (Some(node), invoked, result)
            } else if let Some(path) = &documentation {
                let invocation = prepared
                    .child_documentation_plan(identity, request_id, &inputs, path, false)
                    .map_err(error)?;
                let result = match runner
                    .invoke(prepared.compiler(), invocation.request(), invocation.io())
                    .map_err(error)?
                {
                    ScoopcResponseEnvelopeV1::Success { result, .. } => result,
                    ScoopcResponseEnvelopeV1::Failure { diagnostics, .. } => {
                        let mut failure = error("documentation compilation failed");
                        failure.diagnostics = diagnostics;
                        return Err(failure);
                    }
                    _ => return Err(error("compiler returned an unexpected observation result")),
                };
                (None, true, Some(result))
            } else {
                (None, false, None)
            };
            if let Some(result) = documented {
                let warnings = inputs
                    .iter()
                    .flat_map(|input| input.warnings().iter().cloned())
                    .collect::<Vec<_>>();
                let compiler_inputs = inputs
                    .iter()
                    .flat_map(|input| input.compiler_inputs().iter().cloned())
                    .collect::<Vec<_>>();
                entry.save(&result, &warnings, &compiler_inputs)?;
            }
            if invoked {
                compiled.push(identity);
            }
            Ok(node)
        })();
        let node = result.map_err(|failure: Box<BuildFailure>| {
            let warnings = inputs
                .iter()
                .flat_map(|node| node.warnings().iter().cloned())
                .collect::<Vec<_>>();
            let sources = sources
                .values()
                .flat_map(|source| source.sources.iter().cloned())
                .collect::<Vec<_>>();
            failure.with_context(&warnings, &sources)
        })?;
        if let Some(node) = node {
            completed.insert(identity, node);
        }
    }
    Ok(())
}
