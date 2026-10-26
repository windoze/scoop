use std::collections::BTreeSet;

use scoop_protocol::ScoopcResponseEnvelopeV1;

use super::ready::ReadyNodes;
use super::*;
use crate::{ChildTransportError, PendingSourceExecution, PreparedSourceExecution};

type CompilerResponse = Result<ScoopcResponseEnvelopeV1, ChildTransportError>;

enum PreparedNode {
    Complete(Option<ConeCompileCacheKeyV1>, Box<CompletedNode>),
    Pending(ConeCompileCacheKeyV1, Box<PendingSourceExecution>),
    WaitingForLock(ConeCompileCacheKeyV1),
}

struct Execution {
    order: Vec<ConeIdentity>,
    ready: ReadyNodes,
    completed: BTreeMap<ConeIdentity, CompletedNode>,
    observations: BTreeMap<usize, BuildNodeObservation>,
    child_invocations: BTreeSet<usize>,
    pending: BTreeMap<usize, (ConeCompileCacheKeyV1, Box<PendingSourceExecution>)>,
    waiting_keys: BTreeMap<usize, ConeCompileCacheKeyV1>,
    failures: BTreeMap<usize, BuildGraphExecutionError>,
}

impl Execution {
    fn new(graph: &PreparedBuildGraph, include_root: bool) -> Self {
        let order = graph
            .dependency_first()
            .iter()
            .copied()
            .filter(|id| include_root || *id != graph.root_identity())
            .collect::<Vec<_>>();
        Self {
            ready: ReadyNodes::new(
                &order,
                graph
                    .dependency_edges()
                    .filter(|(dependent, _)| include_root || *dependent != graph.root_identity()),
            ),
            order,
            completed: BTreeMap::new(),
            observations: BTreeMap::new(),
            child_invocations: BTreeSet::new(),
            pending: BTreeMap::new(),
            waiting_keys: BTreeMap::new(),
            failures: BTreeMap::new(),
        }
    }

    fn completed_refs(&self) -> Vec<&CompletedNode> {
        self.order
            .iter()
            .filter_map(|id| self.completed.get(id))
            .collect()
    }

    fn prepare(
        &self,
        graph: &PreparedBuildGraph,
        position: usize,
    ) -> Result<PreparedNode, BuildGraphExecutionError> {
        let identity = self.order[position];
        let completed = self.completed_refs();
        match graph
            .node_representation(identity)
            .ok_or(BuildGraphExecutionError::MissingPreparedNode(identity))?
        {
            PreparedNodeRepresentation::PrebuiltArtifact => graph
                .complete_prebuilt_node(identity, &completed)
                .map(|node| PreparedNode::Complete(None, Box::new(node)))
                .map_err(|source| BuildGraphExecutionError::Prebuilt(identity, Box::new(source))),
            PreparedNodeRepresentation::ManifestSource | PreparedNodeRepresentation::SingleFile => {
                let key = match self.waiting_keys.get(&position) {
                    Some(&key) => key,
                    None => graph
                        .compile_cache_key(identity, &completed)
                        .map_err(|source| {
                            BuildGraphExecutionError::CacheKey(identity, Box::new(source))
                        })?,
                };
                match graph
                    .prepare_ordinary_source(
                        identity,
                        &completed,
                        key,
                        request_id(position)?,
                        self.pending.is_empty(),
                    )
                    .map_err(|source| {
                        BuildGraphExecutionError::Ordinary(identity, Box::new(source))
                    })? {
                    PreparedSourceExecution::Complete(node) => {
                        Ok(PreparedNode::Complete(Some(key), node))
                    }
                    PreparedSourceExecution::Pending(pending) => {
                        Ok(PreparedNode::Pending(key, pending))
                    }
                    PreparedSourceExecution::WaitingForLock => {
                        Ok(PreparedNode::WaitingForLock(key))
                    }
                }
            }
        }
    }

    fn complete(
        &mut self,
        position: usize,
        cache_key: Option<ConeCompileCacheKeyV1>,
        node: CompletedNode,
        invoked_child: bool,
    ) {
        let identity = self.order[position];
        self.waiting_keys.remove(&position);
        if invoked_child {
            self.child_invocations.insert(position);
        }
        self.observations.insert(
            position,
            BuildNodeObservation {
                identity,
                origin: node.origin(),
                cache_key,
            },
        );
        self.completed.insert(identity, node);
        self.ready.complete(position);
    }

    fn receive(
        &mut self,
        graph: &PreparedBuildGraph,
        (position, response): (usize, CompilerResponse),
    ) {
        let (key, pending) = self
            .pending
            .remove(&position)
            .expect("a compiler response belongs to an active node");
        let result = graph.finish_ordinary_source(*pending, &self.completed_refs(), response);
        match result {
            Ok(node) => self.complete(position, Some(key), node, true),
            Err(source) => {
                self.failures.insert(
                    position,
                    BuildGraphExecutionError::Ordinary(self.order[position], Box::new(source)),
                );
            }
        }
    }
}

impl PreparedBuildGraph {
    pub fn execute(self) -> Result<ExecutedBuildGraph, BuildGraphExecutionFailure> {
        self.execute_with_runner(&ProductionSingleConeCompilerRunner)
    }

    pub(crate) fn execute_with_runner(
        self,
        runner: &impl SingleConeCompilerRunner,
    ) -> Result<ExecutedBuildGraph, BuildGraphExecutionFailure> {
        let execution = self.execute_nodes(runner, true)?;
        let (order, completed, observations) = execution.into_parts();
        Ok(ExecutedBuildGraph {
            root: self.root_identity(),
            _prepared: self,
            dependency_first: order,
            completed,
            observations,
        })
    }

    pub(crate) fn execute_dependencies(
        &self,
        runner: &impl SingleConeCompilerRunner,
    ) -> Result<(Vec<CompletedNode>, BuildObservations), BuildGraphExecutionFailure> {
        let execution = self.execute_nodes(runner, false)?;
        let (order, mut completed, observations) = execution.into_parts();
        Ok((
            order
                .into_iter()
                .map(|id| {
                    completed
                        .remove(&id)
                        .expect("scheduled dependency completed")
                })
                .collect(),
            observations,
        ))
    }

    fn execute_nodes(
        &self,
        runner: &impl SingleConeCompilerRunner,
        include_root: bool,
    ) -> Result<Execution, BuildGraphExecutionFailure> {
        let mut execution = Execution::new(self, include_root);
        let compiler = self.compiler().clone();
        let jobs = self.jobs().get();
        std::thread::scope(|scope| {
            let (sender, receiver) = std::sync::mpsc::channel();
            loop {
                while let Ok(response) = receiver.try_recv() {
                    execution.receive(self, response);
                }
                let mut waiting = Vec::new();
                while execution.failures.is_empty() && execution.pending.len() < jobs {
                    let Some(position) = execution.ready.pop() else {
                        break;
                    };
                    match execution.prepare(self, position) {
                        Ok(PreparedNode::WaitingForLock(key)) => {
                            execution.waiting_keys.insert(position, key);
                            waiting.push(position);
                        }
                        Ok(PreparedNode::Complete(key, node)) => {
                            execution.complete(position, key, *node, false)
                        }
                        Ok(PreparedNode::Pending(key, pending)) => {
                            let request = pending.invocation.request().clone();
                            let io = pending.invocation.io().clone();
                            let sender = sender.clone();
                            let compiler = &compiler;
                            let worker = std::thread::Builder::new()
                                .name(format!("scoop-compile-{position}"))
                                .spawn_scoped(scope, move || {
                                    let response =
                                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                                            || runner.invoke(compiler, &request, &io),
                                        ))
                                        .unwrap_or(Err(ChildTransportError::WorkerPanicked));
                                    let _ = sender.send((position, response));
                                });
                            match worker {
                                Ok(_) => {
                                    execution.pending.insert(position, (key, pending));
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
                                }
                            }
                        }
                        Err(source) => {
                            execution.failures.insert(position, source);
                        }
                    }
                }
                execution.ready.defer(waiting);
                if execution.pending.is_empty() {
                    break;
                }
                // The main thread retains graph state and key locks, including
                // while draining already started children after a failure.
                let response = receiver
                    .recv()
                    .expect("an active compiler worker returns a response");
                execution.receive(self, response);
            }
        });
        if let Some((_, cause)) = execution.failures.pop_first() {
            return Err(BuildGraphExecutionFailure::new(
                cause,
                &execution.completed_refs(),
            ));
        }
        debug_assert_eq!(execution.completed.len(), execution.order.len());
        Ok(execution)
    }
}

impl Execution {
    fn into_parts(
        self,
    ) -> (
        Vec<ConeIdentity>,
        BTreeMap<ConeIdentity, CompletedNode>,
        BuildObservations,
    ) {
        let child_invocations = self
            .child_invocations
            .iter()
            .map(|&position| self.order[position])
            .collect();
        let observations = BuildObservations {
            nodes: self.observations.into_values().collect(),
            child_invocations,
        };
        (self.order, self.completed, observations)
    }
}
