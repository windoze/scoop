use std::collections::BTreeSet;

use scoop_protocol::ScoopcResponseEnvelopeV1;

use super::ready::ReadyNodes;
use super::*;
use crate::{ChildTransportError, PendingSourceExecution, PreparedSourceExecution};

pub(super) type CompilerResponse = Result<ScoopcResponseEnvelopeV1, ChildTransportError>;

pub(super) enum PreparedNode {
    Complete(Option<ConeCompileCacheKeyV1>, Box<CompletedNode>),
    Pending(ConeCompileCacheKeyV1, Box<PendingSourceExecution>),
    WaitingForLock(ConeCompileCacheKeyV1),
}

pub(super) struct Execution {
    pub(super) order: Vec<ConeIdentity>,
    pub(super) ready: ReadyNodes,
    completed: BTreeMap<ConeIdentity, CompletedNode>,
    observations: BTreeMap<usize, BuildNodeObservation>,
    child_invocations: BTreeSet<usize>,
    pub(super) pending: BTreeMap<usize, (ConeCompileCacheKeyV1, Box<PendingSourceExecution>)>,
    pub(super) waiting_keys: BTreeMap<usize, ConeCompileCacheKeyV1>,
    pub(super) failures: BTreeMap<usize, BuildGraphExecutionError>,
}

impl Execution {
    pub(super) fn new(graph: &PreparedBuildGraph, include_root: bool) -> Self {
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

    pub(super) fn completed_refs(&self) -> Vec<&CompletedNode> {
        self.order
            .iter()
            .filter_map(|id| self.completed.get(id))
            .collect()
    }

    pub(super) fn prepare(
        &self,
        graph: &PreparedBuildGraph,
        position: usize,
        ordinal: usize,
        wait_for_lock: bool,
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
                        request_id(ordinal)?,
                        wait_for_lock,
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

    pub(super) fn complete(
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

    pub(super) fn receive(
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

impl Execution {
    pub(super) fn into_parts(
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

impl PreparedBuildGraph {
    pub fn execute(self) -> Result<ExecutedBuildGraph, BuildGraphExecutionFailure> {
        self.execute_with_runner(&ProductionSingleConeCompilerRunner)
    }

    pub fn execute_all(
        graphs: Vec<Self>,
    ) -> Result<Vec<ExecutedBuildGraph>, BuildGraphExecutionFailure> {
        Self::execute_all_with_runner(graphs, &ProductionSingleConeCompilerRunner)
    }

    pub(crate) fn execute_with_runner(
        self,
        runner: &impl SingleConeCompilerRunner,
    ) -> Result<ExecutedBuildGraph, BuildGraphExecutionFailure> {
        Ok(Self::execute_all_with_runner(vec![self], runner)?.remove(0))
    }

    pub(crate) fn execute_all_with_runner(
        graphs: Vec<Self>,
        runner: &impl SingleConeCompilerRunner,
    ) -> Result<Vec<ExecutedBuildGraph>, BuildGraphExecutionFailure> {
        let executions = super::batch::execute(&graphs.iter().collect::<Vec<_>>(), runner, true)?;
        Ok(graphs
            .into_iter()
            .zip(executions)
            .map(|(graph, execution)| {
                let (order, completed, observations) = execution.into_parts();
                ExecutedBuildGraph {
                    root: graph.root_identity(),
                    _prepared: graph,
                    dependency_first: order,
                    completed,
                    observations,
                }
            })
            .collect())
    }

    pub(crate) fn execute_dependencies(
        &self,
        runner: &impl SingleConeCompilerRunner,
    ) -> Result<(Vec<CompletedNode>, BuildObservations), BuildGraphExecutionFailure> {
        let execution = super::batch::execute(&[self], runner, false)?.remove(0);
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
}
