//! One worker limit for independent closures. Cache-key locks share actual work.

use super::execution::{CompilerResponse, Execution, PreparedNode};
use super::*;
use crate::ChildTransportError;

pub(super) fn execute(
    graphs: &[&PreparedBuildGraph],
    runner: &impl SingleConeCompilerRunner,
    include_root: bool,
) -> Result<Vec<Execution>, BuildGraphExecutionFailure> {
    let mut executions = graphs
        .iter()
        .map(|graph| Execution::new(graph, include_root))
        .collect::<Vec<_>>();
    let jobs = graphs
        .iter()
        .map(|graph| graph.jobs().get())
        .min()
        .unwrap_or(1);
    let mut next_ordinal = 0usize;
    let offsets = executions
        .iter()
        .map(|execution| {
            let offset = next_ordinal;
            next_ordinal = next_ordinal
                .checked_add(execution.order.len())
                .ok_or_else(|| {
                    BuildGraphExecutionFailure::new(
                        BuildGraphExecutionError::RequestIdOverflow,
                        &[],
                    )
                })?;
            Ok(offset)
        })
        .collect::<Result<Vec<_>, BuildGraphExecutionFailure>>()?;
    std::thread::scope(|scope| {
        let (sender, receiver) = std::sync::mpsc::channel::<(usize, usize, CompilerResponse)>();
        let mut active = 0;
        loop {
            while let Ok((graph, position, response)) = receiver.try_recv() {
                executions[graph].receive(graphs[graph], (position, response));
                active -= 1;
            }
            let mut failed = executions
                .iter()
                .any(|execution| !execution.failures.is_empty());
            for (index, (graph, execution)) in graphs.iter().zip(&mut executions).enumerate() {
                let mut waiting = Vec::new();
                while !failed && active < jobs {
                    let Some(position) = execution.ready.pop() else {
                        break;
                    };
                    match execution.prepare(graph, position, offsets[index] + position, active == 0)
                    {
                        Ok(PreparedNode::WaitingForLock(key)) => {
                            execution.waiting_keys.insert(position, key);
                            waiting.push(position);
                        }
                        Ok(PreparedNode::Complete(key, node)) => {
                            execution.complete(position, key, *node, false);
                        }
                        Ok(PreparedNode::Pending(key, pending)) => {
                            let request = pending.invocation.request().clone();
                            let io = pending.invocation.io().clone();
                            let sender = sender.clone();
                            let compiler = graph.compiler();
                            let worker = std::thread::Builder::new()
                                .name(format!("scoop-compile-{index}-{position}"))
                                .spawn_scoped(scope, move || {
                                    let response =
                                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                                            || runner.invoke(compiler, &request, &io),
                                        ))
                                        .unwrap_or(Err(ChildTransportError::WorkerPanicked));
                                    let _ = sender.send((index, position, response));
                                });
                            match worker {
                                Ok(_) => {
                                    execution.pending.insert(position, (key, pending));
                                    active += 1;
                                }
                                Err(source) => {
                                    execution.failures.insert(
                                        position,
                                        BuildGraphExecutionError::Ordinary(
                                            execution.order[position],
                                            Box::new(OrdinarySourceExecutionError::ChildTransport(
                                                ChildTransportError::WorkerSpawn(source),
                                            )),
                                        ),
                                    );
                                    failed = true;
                                }
                            }
                        }
                        Err(source) => {
                            execution.failures.insert(position, source);
                            failed = true;
                        }
                    }
                }
                execution.ready.defer(waiting);
            }
            if active == 0 {
                break;
            }
            // All locks remain on the parent thread. Drain started workers even
            // after failure, so another root cannot wait on an undelivered result.
            let (graph, position, response) = receiver
                .recv()
                .expect("an active compiler worker returns a response");
            executions[graph].receive(graphs[graph], (position, response));
            active -= 1;
        }
    });
    if let Some((_, cause)) = executions
        .iter_mut()
        .find_map(|execution| execution.failures.pop_first())
    {
        return Err(BuildGraphExecutionFailure::new(
            cause,
            &executions
                .iter()
                .flat_map(Execution::completed_refs)
                .collect::<Vec<_>>(),
        ));
    }
    debug_assert!(
        executions
            .iter()
            .all(|execution| execution.completed_refs().len() == execution.order.len())
    );
    Ok(executions)
}
