use super::*;
use crate::{ProductionSingleConeCompilerRunner, ResolvedBuildGraph, SingleConeCompilerRunner};
use scoop_protocol::{
    HostPathCarrier, RequestCorrelationId, ScoopcProductV1, ScoopcResponseEnvelopeV1,
};

pub(super) fn run(
    graph: ResolvedBuildGraph,
    request: &CheckRequest,
    sources: &[SelectedSource],
) -> BuildResult<(cache::Record, Vec<ConeIdentity>)> {
    let root = graph.root_identity();
    let mut prepared = graph
        .prepare_frontend()
        .map_err(BuildFailure::preparation)?;
    let mut native_inputs = Vec::new();
    let mut machine = false;
    for id in prepared.dependency_first().to_vec() {
        if id != root && prepared.source_input(id).is_some() {
            prepared
                .prepare_machine_source(id)
                .map_err(BuildFailure::preparation)?;
            native_inputs.extend_from_slice(
                prepared
                    .source_input(id)
                    .expect("source dependency")
                    .native_input_metadata(),
            );
            machine = true;
        }
    }
    if machine {
        native_inputs.extend(
            scoop_toolchain::cached_native_tool_inputs(&request.target, request.cache.as_path())
                .map_err(error)?,
        );
    }
    let runner = ProductionSingleConeCompilerRunner;
    let (completed, observations) = prepared
        .execute_dependencies(&runner)
        .map_err(|cause| BuildFailure::execution(cause, sources))?;
    let mut warnings = completed
        .iter()
        .flat_map(|node| node.warnings().iter().cloned())
        .collect::<Vec<_>>();
    let result = (|| {
        let inputs = completed.iter().collect::<Vec<_>>();
        let invocation = prepared
            .child_check_plan(
                RequestCorrelationId::from_array((prepared.node_count() as u128 + 1).to_be_bytes()),
                &inputs,
            )
            .map_err(error)?;
        match runner
            .invoke(prepared.compiler(), invocation.request(), invocation.io())
            .map_err(error)?
        {
            ScoopcResponseEnvelopeV1::Success { result, .. }
                if matches!(result.product(), ScoopcProductV1::Check) =>
            {
                Ok(result)
            }
            ScoopcResponseEnvelopeV1::Failure { diagnostics, .. } => {
                let mut failure = error("root check failed");
                failure.diagnostics = diagnostics;
                Err(failure)
            }
            _ => Err(error("compiler returned an unexpected check result")),
        }
    })()
    .map_err(|failure| failure.with_context(&warnings, sources))?;
    warnings.extend_from_slice(result.warnings());
    let mut used = completed
        .iter()
        .flat_map(|node| node.used_generators().iter().cloned())
        .collect::<Vec<_>>();
    used.extend_from_slice(result.used_generators());
    used.sort_by(|left, right| left.registration().name().cmp(right.registration().name()));
    used.dedup();
    let mut environment_inputs = completed
        .iter()
        .flat_map(|node| node.environment_inputs().iter().cloned())
        .collect::<Vec<_>>();
    environment_inputs.extend_from_slice(result.environment_inputs());
    environment_inputs.sort();
    environment_inputs.dedup();
    let success = cache::Messages {
        warnings,
        used_generators: used,
        environment_inputs,
    };
    Ok((
        cache::Record {
            result: scoop_wire::encode(&success).map_err(error)?,
            native_inputs,
            dependencies: completed
                .iter()
                .map(|node| HostPathCarrier::from_path(node.artifact_locator()).map_err(error))
                .collect::<BuildResult<_>>()?,
        },
        observations.child_invocations().to_vec(),
    ))
}
