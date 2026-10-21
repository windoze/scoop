use super::{OrdinarySourceExecutionError as Error, PreparedBuildGraph, PreparedSourceExecution};
use crate::{CompletedNode, SingleConeCompilerRunner};
use scoop_identity::ConeIdentity;
use scoop_protocol::{RequestCorrelationId, ScoopcResponseEnvelopeV1, ScoopcSuccessV1};
use std::path::Path;

impl PreparedBuildGraph {
    /// Shares normal cache preparation and publication with concurrent builds.
    pub(crate) fn execute_source_output(
        &self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
        runner: &impl SingleConeCompilerRunner,
        request_id: RequestCorrelationId,
        documentation: Option<&Path>,
    ) -> Result<(CompletedNode, bool, Option<Box<ScoopcSuccessV1>>), Error> {
        let key = self
            .compile_cache_key(identity, completed)
            .map_err(|source| Error::CacheKey(Box::new(source)))?;
        let prepared =
            self.prepare_source_output(identity, completed, key, request_id, true, documentation)?;
        let (node, invoked, documented) = match prepared {
            PreparedSourceExecution::WaitingForLock => {
                unreachable!("blocking acquisition completes")
            }
            PreparedSourceExecution::Complete(node) => (*node, false, None),
            PreparedSourceExecution::Pending(pending) => {
                let expects_documentation = pending
                    .invocation
                    .request()
                    .build()
                    .output()
                    .documentation()
                    .is_some();
                let response = runner.invoke(
                    self.compiler(),
                    pending.invocation.request(),
                    pending.invocation.io(),
                );
                let documented = match &response {
                    Ok(ScoopcResponseEnvelopeV1::Success { result, .. })
                        if expects_documentation =>
                    {
                        if result.documentation().is_none() {
                            return Err(Error::UnexpectedResponse);
                        }
                        Some(result.clone())
                    }
                    _ => None,
                };
                let node = self.finish_ordinary_source(*pending, completed, response)?;
                (node, true, documented)
            }
        };
        if let Some(path) = documentation
            && documented.is_none()
        {
            let invocation = self
                .child_documentation_plan(identity, request_id, completed, path, false)
                .map_err(Error::RequestPlan)?;
            let documented = match runner
                .invoke(self.compiler(), invocation.request(), invocation.io())
                .map_err(Error::ChildTransport)?
            {
                ScoopcResponseEnvelopeV1::Success { result, .. }
                    if result.documentation().is_some() =>
                {
                    result
                }
                ScoopcResponseEnvelopeV1::Failure { diagnostics, .. } => {
                    return Err(Error::ChildFailure(diagnostics));
                }
                _ => return Err(Error::UnexpectedResponse),
            };
            return Ok((node, true, Some(documented)));
        }
        Ok((node, invoked, documented))
    }
}
