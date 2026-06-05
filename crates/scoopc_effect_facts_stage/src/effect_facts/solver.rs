use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use crate::mir::{BasicBlockId, InstanceKey};
use crate::opt::OptLevel;

use super::{
    BlockEffectFacts, BodyEffectFacts, CallSiteEffectFacts, CallSiteTarget, CallableAbiKind,
    CallableEffectFacts, CaseSet, CaseTag, ConcreteOpKey, EffectPrecision, HandleArmEffectFacts,
    HandleSiteEffectFacts, HandleSiteSolverFacts, ImplPlan, MaterializedEffectFacts,
    SiteEffectFacts, StepSchema, StepSchemaId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EffectFactsSolverBudget {
    max_scc_nodes: usize,
    max_scc_edges: usize,
    max_scc_iterations: usize,
    max_candidate_union_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EffectFactsSolverConfig {
    opt_level: OptLevel,
    budget: EffectFactsSolverBudget,
}

impl EffectFactsSolverConfig {
    fn for_opt_level(opt_level: OptLevel) -> Self {
        let budget = match opt_level {
            OptLevel::O0 => EffectFactsSolverBudget {
                max_scc_nodes: 32,
                max_scc_edges: 64,
                max_scc_iterations: 4,
                max_candidate_union_size: 4,
            },
            OptLevel::O1 | OptLevel::O2 | OptLevel::O3 | OptLevel::Os | OptLevel::Oz => {
                EffectFactsSolverBudget {
                    max_scc_nodes: 256,
                    max_scc_edges: 1024,
                    max_scc_iterations: 16,
                    max_candidate_union_size: 16,
                }
            }
        };
        Self { opt_level, budget }
    }

    #[cfg(test)]
    #[cfg_attr(feature = "standalone-stage-crate", allow(dead_code))]
    fn with_budget(opt_level: OptLevel, budget: EffectFactsSolverBudget) -> Self {
        Self { opt_level, budget }
    }
}

#[derive(Debug, Clone)]
struct CallableState {
    resolved_outward_cases: CaseSet,
    widened: bool,
}

#[derive(Debug)]
struct SchemaProjectionIndex {
    full_case_sets: HashMap<StepSchemaId, CaseSet>,
    concrete_op_by_tag: HashMap<StepSchemaId, HashMap<CaseTag, ConcreteOpKey>>,
    tag_by_concrete_op: HashMap<StepSchemaId, HashMap<ConcreteOpKey, CaseTag>>,
}

impl SchemaProjectionIndex {
    fn new(step_schemas: &BTreeMap<StepSchemaId, StepSchema>) -> Self {
        let mut full_case_sets = HashMap::with_capacity(step_schemas.len());
        let mut concrete_op_by_tag = HashMap::with_capacity(step_schemas.len());
        let mut tag_by_concrete_op = HashMap::with_capacity(step_schemas.len());
        for (schema_id, schema) in step_schemas {
            full_case_sets.insert(
                *schema_id,
                CaseSet::new(
                    *schema_id,
                    schema.cases().iter().map(|case| case.case_tag()).collect(),
                ),
            );
            concrete_op_by_tag.insert(
                *schema_id,
                schema
                    .cases()
                    .iter()
                    .map(|case| (case.case_tag(), case.concrete_op_key().clone()))
                    .collect(),
            );
            tag_by_concrete_op.insert(
                *schema_id,
                schema
                    .cases()
                    .iter()
                    .map(|case| (case.concrete_op_key().clone(), case.case_tag()))
                    .collect(),
            );
        }
        Self {
            full_case_sets,
            concrete_op_by_tag,
            tag_by_concrete_op,
        }
    }

    fn empty_case_set(&self, schema: StepSchemaId) -> CaseSet {
        CaseSet::new(schema, Vec::new())
    }

    fn full_case_set(&self, schema: StepSchemaId) -> CaseSet {
        self.full_case_sets
            .get(&schema)
            .cloned()
            .unwrap_or_else(|| self.empty_case_set(schema))
    }

    fn singleton(&self, schema: StepSchemaId, tag: CaseTag) -> CaseSet {
        CaseSet::new(schema, vec![tag])
    }

    fn project_case_set(&self, source: &CaseSet, target_schema: StepSchemaId) -> CaseSet {
        let Some(source_index) = self.concrete_op_by_tag.get(&source.schema()) else {
            return self.empty_case_set(target_schema);
        };
        let Some(target_index) = self.tag_by_concrete_op.get(&target_schema) else {
            return self.empty_case_set(target_schema);
        };
        let mut projected = Vec::new();
        for tag in source.tags() {
            let Some(concrete_op) = source_index.get(tag) else {
                continue;
            };
            if let Some(target_tag) = target_index.get(concrete_op) {
                projected.push(*target_tag);
            }
        }
        CaseSet::new(target_schema, projected)
    }
}

#[derive(Debug, Clone)]
struct CallResolution {
    callee_abi_kind: CallableAbiKind,
    callee_schema: Option<StepSchemaId>,
    resolved_cases: CaseSet,
    precision: EffectPrecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterializedEffectFactsSolver {
    config: EffectFactsSolverConfig,
}

impl Default for MaterializedEffectFactsSolver {
    fn default() -> Self {
        Self::for_opt_level(OptLevel::O2)
    }
}

impl MaterializedEffectFactsSolver {
    pub fn for_opt_level(opt_level: OptLevel) -> Self {
        Self {
            config: EffectFactsSolverConfig::for_opt_level(opt_level),
        }
    }

    #[cfg(test)]
    #[cfg_attr(feature = "standalone-stage-crate", allow(dead_code))]
    fn with_config(config: EffectFactsSolverConfig) -> Self {
        Self { config }
    }

    pub fn solve(self, facts: MaterializedEffectFacts) -> MaterializedEffectFacts {
        let (
            type_context,
            snapshot_binding,
            step_schemas,
            continuation_schemas,
            callable_facts,
            bodies,
        ) = facts.into_parts();
        let schema_index = SchemaProjectionIndex::new(&step_schemas);
        let sorted_keys = sorted_callable_keys(&callable_facts);
        let key_to_index = sorted_keys
            .iter()
            .enumerate()
            .map(|(index, key)| (key.clone(), index))
            .collect::<HashMap<_, _>>();
        let adjacency = build_call_graph(&sorted_keys, &key_to_index, &bodies);
        let (components, component_by_node) = tarjan_scc(&adjacency);
        let component_order =
            reverse_topological_component_order(&components, &component_by_node, &adjacency);
        let callable_step_schemas = callable_facts
            .iter()
            .map(|(key, callable)| (key.clone(), callable.step_schema()))
            .collect::<HashMap<_, _>>();
        let local_cases = sorted_keys
            .iter()
            .map(|key| {
                let callable = callable_facts
                    .get(key)
                    .expect("solver node should have callable shell facts");
                compute_local_cases(key, callable, &bodies, &schema_index)
            })
            .collect::<Vec<_>>();

        let mut states = local_cases
            .iter()
            .cloned()
            .map(|resolved_outward_cases| CallableState {
                resolved_outward_cases,
                widened: false,
            })
            .collect::<Vec<_>>();

        for component_id in component_order {
            let component = &components[component_id];
            if component_exceeds_scc_budget(
                component,
                component_id,
                &component_by_node,
                &adjacency,
                self.config,
            ) {
                widen_component(
                    component,
                    &sorted_keys,
                    &callable_facts,
                    &schema_index,
                    &mut states,
                );
                continue;
            }

            let mut converged = false;
            for _ in 0..self.config.budget.max_scc_iterations {
                let snapshot = states.clone();
                let mut changed = false;
                for &node_index in component {
                    let key = &sorted_keys[node_index];
                    let callable = callable_facts
                        .get(key)
                        .expect("solver node should have callable shell facts");
                    let Some(body) = bodies.get(key) else {
                        continue;
                    };
                    let next_cases = compute_callable_resolved_cases(
                        callable,
                        body,
                        &local_cases[node_index],
                        &snapshot,
                        &key_to_index,
                        &schema_index,
                        self.config,
                    );
                    let next_state = if next_cases.force_full {
                        CallableState {
                            resolved_outward_cases: schema_index
                                .full_case_set(callable.step_schema()),
                            widened: true,
                        }
                    } else {
                        CallableState {
                            resolved_outward_cases: next_cases.resolved_outward_cases,
                            widened: snapshot[node_index].widened,
                        }
                    };
                    if next_state.widened != states[node_index].widened
                        || next_state.resolved_outward_cases
                            != states[node_index].resolved_outward_cases
                    {
                        states[node_index] = next_state;
                        changed = true;
                    }
                }
                if !changed {
                    converged = true;
                    break;
                }
            }

            if !converged {
                widen_component(
                    component,
                    &sorted_keys,
                    &callable_facts,
                    &schema_index,
                    &mut states,
                );
            }
        }

        let solved_callable_facts: HashMap<InstanceKey, CallableEffectFacts> = callable_facts
            .into_iter()
            .map(|(key, callable)| {
                let node_index = *key_to_index
                    .get(&key)
                    .expect("solved callable should still have a node index");
                let resolved_outward_cases = states[node_index].resolved_outward_cases.clone();
                let needs_reentry = !resolved_outward_cases.is_empty();
                let impl_plan = derive_impl_plan(self.config.opt_level, &resolved_outward_cases);
                let call_abi_kind = derive_callable_abi_kind(&resolved_outward_cases);
                let (invoke_args_tuple_ty, step_schema) = match call_abi_kind {
                    CallableAbiKind::Plain => (None, None),
                    CallableAbiKind::EffectStep => (
                        Some(callable.invoke_args_tuple_ty()),
                        Some(callable.step_schema()),
                    ),
                };
                (
                    key,
                    CallableEffectFacts::new(
                        callable.declared_row().clone(),
                        call_abi_kind,
                        invoke_args_tuple_ty,
                        step_schema,
                        resolved_outward_cases,
                        needs_reentry,
                        impl_plan,
                    ),
                )
            })
            .collect();

        let solved_bodies: HashMap<InstanceKey, BodyEffectFacts> = bodies
            .into_iter()
            .map(|(key, body)| {
                let callable = solved_callable_facts
                    .get(&key)
                    .expect("every solved body should still have callable shell facts");
                let current_schema = *callable_step_schemas
                    .get(&key)
                    .expect("every solved body should retain its analysis step schema");
                let finalized_sites = finalize_body_sites(
                    current_schema,
                    &body,
                    &states,
                    &key_to_index,
                    &schema_index,
                    self.config,
                );
                let finalized_blocks = finalize_body_blocks(
                    current_schema,
                    callable.resolved_outward_cases(),
                    &body,
                    &finalized_sites,
                    &schema_index,
                );
                let local_control_step_schema =
                    if matches!(callable.call_abi_kind(), CallableAbiKind::Plain)
                        && body_needs_plain_local_control(&finalized_sites)
                    {
                        Some(current_schema)
                    } else {
                        None
                    };
                (
                    key,
                    BodyEffectFacts::with_solver_facts(
                        finalized_blocks,
                        finalized_sites,
                        local_control_step_schema,
                        body.solver_facts().clone(),
                    ),
                )
            })
            .collect();

        let mut step_schemas = step_schemas;
        for (key, callable) in &solved_callable_facts {
            if !matches!(callable.call_abi_kind(), CallableAbiKind::Plain) {
                continue;
            }
            let Some(schema_id) = callable_step_schemas.get(key).copied() else {
                continue;
            };
            let retained_for_local_control = solved_bodies
                .get(key)
                .and_then(BodyEffectFacts::local_control_step_schema)
                == Some(schema_id);
            if retained_for_local_control {
                continue;
            }
            if step_schemas
                .get(&schema_id)
                .is_some_and(|schema| schema.cases().is_empty())
            {
                step_schemas.remove(&schema_id);
            }
        }

        MaterializedEffectFacts::new(
            type_context,
            snapshot_binding,
            step_schemas,
            continuation_schemas,
            solved_callable_facts,
            solved_bodies,
        )
    }
}

#[derive(Debug)]
struct CallableResolution {
    resolved_outward_cases: CaseSet,
    force_full: bool,
}

#[derive(Debug, Clone, Default)]
struct RegionCaseContribution {
    non_cleanup: BTreeSet<CaseTag>,
    cleanup: BTreeSet<CaseTag>,
}

impl RegionCaseContribution {
    fn add_case_set(&mut self, is_cleanup: bool, cases: &CaseSet) {
        let target = if is_cleanup {
            &mut self.cleanup
        } else {
            &mut self.non_cleanup
        };
        target.extend(cases.tags().iter().copied());
    }

    fn extend(&mut self, other: Self) {
        self.non_cleanup.extend(other.non_cleanup);
        self.cleanup.extend(other.cleanup);
    }

    fn total_tags(&self) -> BTreeSet<CaseTag> {
        let mut total = self.non_cleanup.clone();
        total.extend(self.cleanup.iter().copied());
        total
    }
}

fn sorted_callable_keys(
    callable_facts: &HashMap<InstanceKey, CallableEffectFacts>,
) -> Vec<InstanceKey> {
    let mut keys = callable_facts.keys().cloned().collect::<Vec<_>>();
    keys.sort_by_key(|key| format!("{key:?}"));
    keys
}

fn compute_local_cases(
    key: &InstanceKey,
    callable: &CallableEffectFacts,
    bodies: &HashMap<InstanceKey, BodyEffectFacts>,
    schema_index: &SchemaProjectionIndex,
) -> CaseSet {
    let current_schema = callable.step_schema();
    let Some(body) = bodies.get(key) else {
        return schema_index.empty_case_set(current_schema);
    };
    let mut local_tags = BTreeSet::new();
    for (block_id, site_ids) in body.solver_facts().block_sites() {
        let handled_cases = body
            .solver_facts()
            .handled_cases_for_block(*block_id)
            .cloned()
            .unwrap_or_else(|| schema_index.empty_case_set(current_schema));
        for site_id in site_ids {
            let Some(site) = body.site(*site_id) else {
                continue;
            };
            let Some(local_site_cases) = local_site_cases(current_schema, site, schema_index)
            else {
                continue;
            };
            let site_cases = subtract_case_set(&local_site_cases, &handled_cases);
            local_tags.extend(site_cases.tags().iter().copied());
        }
    }
    CaseSet::new(current_schema, local_tags.into_iter().collect())
}

fn local_site_cases(
    current_schema: StepSchemaId,
    site: &SiteEffectFacts,
    schema_index: &SchemaProjectionIndex,
) -> Option<CaseSet> {
    match site {
        SiteEffectFacts::Call(_) => None,
        SiteEffectFacts::ClassCtor(facts) => Some(facts.emitted_cases().clone()),
        SiteEffectFacts::Perform(facts) => {
            Some(schema_index.singleton(current_schema, facts.emitted_case()))
        }
        SiteEffectFacts::Resume(facts) => {
            Some(schema_index.project_case_set(facts.resolved_cases(), current_schema))
        }
        SiteEffectFacts::Handle(_) => None,
    }
}

fn handle_total_outward_cases(
    current_schema: StepSchemaId,
    facts: &HandleSiteEffectFacts,
) -> CaseSet {
    let mut tags = BTreeSet::new();
    tags.extend(facts.body_outward_cases().tags().iter().copied());
    for arm in facts.arm_facts() {
        tags.extend(arm.arm_outward_cases().tags().iter().copied());
    }
    tags.extend(facts.finally_outward_cases().tags().iter().copied());
    CaseSet::new(current_schema, tags.into_iter().collect())
}

fn build_call_graph(
    sorted_keys: &[InstanceKey],
    key_to_index: &HashMap<InstanceKey, usize>,
    bodies: &HashMap<InstanceKey, BodyEffectFacts>,
) -> Vec<Vec<usize>> {
    sorted_keys
        .iter()
        .map(|key| {
            let Some(body) = bodies.get(key) else {
                return Vec::new();
            };
            let mut successors = HashSet::new();
            for site_ids in body.solver_facts().block_sites().values() {
                for site_id in site_ids {
                    let Some(SiteEffectFacts::Call(call_facts)) = body.site(*site_id) else {
                        continue;
                    };
                    match call_facts.target() {
                        CallSiteTarget::KnownInstance(target) => {
                            if let Some(index) = key_to_index.get(target) {
                                successors.insert(*index);
                            }
                        }
                        CallSiteTarget::CandidateSet(targets) => {
                            for target in targets {
                                if let Some(index) = key_to_index.get(target) {
                                    successors.insert(*index);
                                }
                            }
                        }
                        CallSiteTarget::BodylessDirect { .. } => {}
                        CallSiteTarget::DynamicFallback => {}
                    }
                }
            }
            let mut ordered = successors.into_iter().collect::<Vec<_>>();
            ordered.sort_unstable();
            ordered
        })
        .collect()
}

fn tarjan_scc(adjacency: &[Vec<usize>]) -> (Vec<Vec<usize>>, Vec<usize>) {
    struct Tarjan<'a> {
        adjacency: &'a [Vec<usize>],
        next_index: usize,
        stack: Vec<usize>,
        on_stack: Vec<bool>,
        indices: Vec<Option<usize>>,
        lowlinks: Vec<usize>,
        components: Vec<Vec<usize>>,
        component_by_node: Vec<usize>,
    }

    impl Tarjan<'_> {
        fn strong_connect(&mut self, node: usize) {
            self.indices[node] = Some(self.next_index);
            self.lowlinks[node] = self.next_index;
            self.next_index += 1;
            self.stack.push(node);
            self.on_stack[node] = true;

            for &target in &self.adjacency[node] {
                if self.indices[target].is_none() {
                    self.strong_connect(target);
                    self.lowlinks[node] = self.lowlinks[node].min(self.lowlinks[target]);
                } else if self.on_stack[target] {
                    self.lowlinks[node] = self.lowlinks[node].min(
                        self.indices[target].expect("stacked node should already have an index"),
                    );
                }
            }

            if self.lowlinks[node] == self.indices[node].expect("current node should have an index")
            {
                let mut component = Vec::new();
                let component_id = self.components.len();
                loop {
                    let member = self.stack.pop().expect("SCC stack should not underflow");
                    self.on_stack[member] = false;
                    self.component_by_node[member] = component_id;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                self.components.push(component);
            }
        }
    }

    let mut tarjan = Tarjan {
        adjacency,
        next_index: 0,
        stack: Vec::new(),
        on_stack: vec![false; adjacency.len()],
        indices: vec![None; adjacency.len()],
        lowlinks: vec![0; adjacency.len()],
        components: Vec::new(),
        component_by_node: vec![usize::MAX; adjacency.len()],
    };
    for node in 0..adjacency.len() {
        if tarjan.indices[node].is_none() {
            tarjan.strong_connect(node);
        }
    }
    (tarjan.components, tarjan.component_by_node)
}

fn reverse_topological_component_order(
    components: &[Vec<usize>],
    component_by_node: &[usize],
    adjacency: &[Vec<usize>],
) -> Vec<usize> {
    let mut component_edges = vec![BTreeSet::new(); components.len()];
    let mut indegree = vec![0usize; components.len()];
    for (node, targets) in adjacency.iter().enumerate() {
        let source_component = component_by_node[node];
        for &target in targets {
            let target_component = component_by_node[target];
            if source_component == target_component
                || !component_edges[source_component].insert(target_component)
            {
                continue;
            }
            indegree[target_component] += 1;
        }
    }

    let mut queue = indegree
        .iter()
        .enumerate()
        .filter_map(|(component, degree)| (*degree == 0).then_some(component))
        .collect::<VecDeque<_>>();
    let mut topo = Vec::with_capacity(components.len());
    while let Some(component) = queue.pop_front() {
        topo.push(component);
        for &target in &component_edges[component] {
            indegree[target] -= 1;
            if indegree[target] == 0 {
                queue.push_back(target);
            }
        }
    }
    topo.reverse();
    topo
}

fn component_exceeds_scc_budget(
    component: &[usize],
    component_id: usize,
    component_by_node: &[usize],
    adjacency: &[Vec<usize>],
    config: EffectFactsSolverConfig,
) -> bool {
    component.len() > config.budget.max_scc_nodes
        || component_edge_count(component_id, component_by_node, adjacency)
            > config.budget.max_scc_edges
}

fn component_edge_count(
    component_id: usize,
    component_by_node: &[usize],
    adjacency: &[Vec<usize>],
) -> usize {
    let mut edges = HashSet::new();
    for (node, targets) in adjacency.iter().enumerate() {
        if component_by_node[node] != component_id {
            continue;
        }
        for &target in targets {
            if component_by_node[target] == component_id {
                edges.insert((node, target));
            }
        }
    }
    edges.len()
}

fn widen_component(
    component: &[usize],
    sorted_keys: &[InstanceKey],
    callable_facts: &HashMap<InstanceKey, CallableEffectFacts>,
    schema_index: &SchemaProjectionIndex,
    states: &mut [CallableState],
) {
    for &node_index in component {
        let callable = callable_facts
            .get(&sorted_keys[node_index])
            .expect("widened SCC node should still have callable shell facts");
        states[node_index] = CallableState {
            resolved_outward_cases: schema_index.full_case_set(callable.step_schema()),
            widened: true,
        };
    }
}

fn compute_callable_resolved_cases(
    callable: &CallableEffectFacts,
    body: &BodyEffectFacts,
    local_cases: &CaseSet,
    states: &[CallableState],
    key_to_index: &HashMap<InstanceKey, usize>,
    schema_index: &SchemaProjectionIndex,
    config: EffectFactsSolverConfig,
) -> CallableResolution {
    let current_schema = callable.step_schema();
    let mut resolved_tags = local_cases.tags().iter().copied().collect::<BTreeSet<_>>();
    for (block_id, site_ids) in body.solver_facts().block_sites() {
        let handled_cases = body
            .solver_facts()
            .handled_cases_for_block(*block_id)
            .cloned()
            .unwrap_or_else(|| schema_index.empty_case_set(current_schema));
        for site_id in site_ids {
            let Some(SiteEffectFacts::Call(call_facts)) = body.site(*site_id) else {
                continue;
            };
            if matches!(call_facts.callee_abi_kind(), CallableAbiKind::EffectStep)
                && matches!(call_facts.target(), CallSiteTarget::CandidateSet(targets) if targets.len() > config.budget.max_candidate_union_size)
            {
                return CallableResolution {
                    resolved_outward_cases: schema_index.full_case_set(current_schema),
                    force_full: true,
                };
            }
            let site_resolution = finalize_call_site_resolution(
                call_facts,
                states,
                key_to_index,
                schema_index,
                config,
            );
            let projected =
                schema_index.project_case_set(&site_resolution.resolved_cases, current_schema);
            let contributed = subtract_case_set(&projected, &handled_cases);
            resolved_tags.extend(contributed.tags().iter().copied());
        }
    }
    CallableResolution {
        resolved_outward_cases: CaseSet::new(current_schema, resolved_tags.into_iter().collect()),
        force_full: false,
    }
}

fn finalize_body_sites(
    current_schema: StepSchemaId,
    body: &BodyEffectFacts,
    states: &[CallableState],
    key_to_index: &HashMap<InstanceKey, usize>,
    schema_index: &SchemaProjectionIndex,
    config: EffectFactsSolverConfig,
) -> BTreeMap<crate::mir::SiteId, SiteEffectFacts> {
    let mut finalized = body
        .sites()
        .iter()
        .map(|(site_id, site)| {
            let finalized = match site {
                SiteEffectFacts::Call(call_facts) => {
                    let resolution = finalize_call_site_resolution(
                        call_facts,
                        states,
                        key_to_index,
                        schema_index,
                        config,
                    );
                    SiteEffectFacts::Call(CallSiteEffectFacts::new_with_abi(
                        call_facts.kind(),
                        call_facts.target().clone(),
                        resolution.callee_abi_kind,
                        call_facts.invoke_args_tuple_ty(),
                        resolution.callee_schema,
                        resolution.resolved_cases,
                        resolution.precision,
                    ))
                }
                SiteEffectFacts::ClassCtor(facts) => SiteEffectFacts::ClassCtor(facts.clone()),
                SiteEffectFacts::Perform(facts) => SiteEffectFacts::Perform(facts.clone()),
                SiteEffectFacts::Resume(facts) => SiteEffectFacts::Resume(facts.clone()),
                SiteEffectFacts::Handle(facts) => SiteEffectFacts::Handle(facts.clone()),
            };
            (*site_id, finalized)
        })
        .collect::<BTreeMap<_, _>>();
    finalize_handle_sites(current_schema, body, &mut finalized, schema_index);
    finalized
}

fn finalize_handle_sites(
    current_schema: StepSchemaId,
    body: &BodyEffectFacts,
    finalized_sites: &mut BTreeMap<crate::mir::SiteId, SiteEffectFacts>,
    schema_index: &SchemaProjectionIndex,
) {
    loop {
        let snapshot = finalized_sites.clone();
        let mut changed = false;
        for (site_id, site) in body.sites() {
            let SiteEffectFacts::Handle(handle_facts) = site else {
                continue;
            };
            let Some(region) = body.solver_facts().handle_site(*site_id) else {
                continue;
            };
            let next = recompute_handle_site_facts(
                current_schema,
                body,
                &snapshot,
                handle_facts,
                region,
                schema_index,
            );
            let next_site = SiteEffectFacts::Handle(next);
            if snapshot.get(site_id) != Some(&next_site) {
                finalized_sites.insert(*site_id, next_site);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

fn recompute_handle_site_facts(
    current_schema: StepSchemaId,
    body: &BodyEffectFacts,
    finalized_sites: &BTreeMap<crate::mir::SiteId, SiteEffectFacts>,
    original: &HandleSiteEffectFacts,
    region: &HandleSiteSolverFacts,
    schema_index: &SchemaProjectionIndex,
) -> HandleSiteEffectFacts {
    let mut body_stops = BTreeSet::from([region.exit_target()]);
    if let Some(finally_target) = region.finally_target() {
        body_stops.insert(finally_target);
    }
    let body_cases = collect_region_cases_from_finalized_sites(
        current_schema,
        body,
        finalized_sites,
        region.body_target(),
        &body_stops,
        schema_index,
        &mut BTreeSet::new(),
    );

    let handled_tags = original
        .handled_cases()
        .tags()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut arm_facts = Vec::with_capacity(original.arm_facts().len());
    let mut arm_non_cleanup = BTreeSet::new();
    let mut cleanup_outward = body_cases.cleanup.clone();

    for (arm, arm_target) in original.arm_facts().iter().zip(region.arm_targets()) {
        let arm_cases = collect_region_cases_from_finalized_sites(
            current_schema,
            body,
            finalized_sites,
            *arm_target,
            &body_stops,
            schema_index,
            &mut BTreeSet::new(),
        );
        cleanup_outward.extend(arm_cases.cleanup.iter().copied());
        arm_non_cleanup.extend(arm_cases.non_cleanup.iter().copied());
        arm_facts.push(HandleArmEffectFacts::new(
            arm.handled_case(),
            arm.payload_tuple_ty(),
            arm.continuation_schema(),
            CaseSet::new(current_schema, arm_cases.non_cleanup.into_iter().collect()),
        ));
    }

    let finally_cases = if let Some(finally_target) = region.finally_target() {
        collect_region_cases_from_finalized_sites(
            current_schema,
            body,
            finalized_sites,
            finally_target,
            &BTreeSet::from([region.exit_target()]),
            schema_index,
            &mut BTreeSet::new(),
        )
    } else {
        RegionCaseContribution::default()
    };

    let body_outward = body_cases
        .non_cleanup
        .difference(&handled_tags)
        .copied()
        .collect::<BTreeSet<_>>();
    cleanup_outward.extend(finally_cases.total_tags());
    let classification =
        if body_outward.is_empty() && arm_non_cleanup.is_empty() && cleanup_outward.is_empty() {
            crate::effect_facts::NestedHandleClassification::SelfContained
        } else {
            crate::effect_facts::NestedHandleClassification::MaySuspendOutward
        };

    HandleSiteEffectFacts::new(
        original.result_ty(),
        original.handled_cases().clone(),
        CaseSet::new(current_schema, body_outward.into_iter().collect()),
        arm_facts,
        CaseSet::new(current_schema, cleanup_outward.into_iter().collect()),
        classification,
    )
}

fn collect_region_cases_from_finalized_sites(
    current_schema: StepSchemaId,
    body: &BodyEffectFacts,
    finalized_sites: &BTreeMap<crate::mir::SiteId, SiteEffectFacts>,
    entry: BasicBlockId,
    stops: &BTreeSet<BasicBlockId>,
    schema_index: &SchemaProjectionIndex,
    visited: &mut BTreeSet<BasicBlockId>,
) -> RegionCaseContribution {
    if stops.contains(&entry) || !visited.insert(entry) {
        return RegionCaseContribution::default();
    }

    let mut acc = RegionCaseContribution::default();
    let is_cleanup = body.solver_facts().is_cleanup_block(entry);
    if let Some(site_ids) = body.solver_facts().block_sites().get(&entry) {
        for site_id in site_ids {
            let Some(site) = finalized_sites.get(site_id) else {
                continue;
            };
            let Some(site_cases) = site_cases_for_region(current_schema, site, schema_index) else {
                continue;
            };
            acc.add_case_set(is_cleanup, &site_cases);
        }
    }
    if let Some(successors) = body.solver_facts().block_successors().get(&entry) {
        for successor in successors {
            acc.extend(collect_region_cases_from_finalized_sites(
                current_schema,
                body,
                finalized_sites,
                *successor,
                stops,
                schema_index,
                visited,
            ));
        }
    }
    acc
}

fn site_cases_for_region(
    current_schema: StepSchemaId,
    site: &SiteEffectFacts,
    schema_index: &SchemaProjectionIndex,
) -> Option<CaseSet> {
    match site {
        SiteEffectFacts::Call(facts) => {
            Some(schema_index.project_case_set(facts.resolved_cases(), current_schema))
        }
        SiteEffectFacts::ClassCtor(facts) => Some(facts.emitted_cases().clone()),
        SiteEffectFacts::Perform(facts) => {
            Some(schema_index.singleton(current_schema, facts.emitted_case()))
        }
        SiteEffectFacts::Resume(facts) => {
            Some(schema_index.project_case_set(facts.resolved_cases(), current_schema))
        }
        SiteEffectFacts::Handle(facts) => Some(handle_total_outward_cases(current_schema, facts)),
    }
}

fn finalize_call_site_resolution(
    call_facts: &CallSiteEffectFacts,
    states: &[CallableState],
    key_to_index: &HashMap<InstanceKey, usize>,
    schema_index: &SchemaProjectionIndex,
    config: EffectFactsSolverConfig,
) -> CallResolution {
    match call_facts.target() {
        CallSiteTarget::KnownInstance(target) => {
            let Some(index) = key_to_index.get(target) else {
                panic!(
                    "effect facts solver received unpublished known-instance call target `{}`",
                    target.template.fqn
                );
            };
            let precision = if states[*index].widened {
                EffectPrecision::Widened
            } else {
                EffectPrecision::Precise
            };
            if states[*index].resolved_outward_cases.is_empty() {
                return plain_call_resolution(call_facts, precision);
            }
            let resolved_cases = schema_index.project_case_set(
                &states[*index].resolved_outward_cases,
                call_facts.callee_schema(),
            );
            CallResolution {
                callee_abi_kind: CallableAbiKind::EffectStep,
                callee_schema: Some(call_facts.callee_schema()),
                resolved_cases,
                precision,
            }
        }
        CallSiteTarget::BodylessDirect { .. } => {
            plain_call_resolution(call_facts, call_facts.precision())
        }
        CallSiteTarget::CandidateSet(targets) => {
            if matches!(call_facts.callee_abi_kind(), CallableAbiKind::Plain) {
                return plain_call_resolution(call_facts, call_facts.precision());
            }
            if targets.len() > config.budget.max_candidate_union_size {
                return CallResolution {
                    callee_abi_kind: CallableAbiKind::EffectStep,
                    callee_schema: Some(call_facts.callee_schema()),
                    resolved_cases: schema_index.full_case_set(call_facts.callee_schema()),
                    precision: EffectPrecision::Widened,
                };
            }

            let mut precise = true;
            let mut resolved_tags = BTreeSet::new();
            for target in targets {
                let Some(index) = key_to_index.get(target) else {
                    panic!(
                        "effect facts solver received unpublished candidate call target `{}`",
                        target.template.fqn
                    );
                };
                let projected = schema_index.project_case_set(
                    &states[*index].resolved_outward_cases,
                    call_facts.callee_schema(),
                );
                resolved_tags.extend(projected.tags().iter().copied());
                precise &= !states[*index].widened;
            }
            if resolved_tags.is_empty() {
                return plain_call_resolution(
                    call_facts,
                    if precise {
                        EffectPrecision::Precise
                    } else {
                        EffectPrecision::Widened
                    },
                );
            }

            CallResolution {
                callee_abi_kind: CallableAbiKind::EffectStep,
                callee_schema: Some(call_facts.callee_schema()),
                resolved_cases: CaseSet::new(
                    call_facts.callee_schema(),
                    resolved_tags.into_iter().collect(),
                ),
                precision: if precise {
                    EffectPrecision::Precise
                } else {
                    EffectPrecision::Widened
                },
            }
        }
        CallSiteTarget::DynamicFallback => match call_facts.callee_abi_kind() {
            CallableAbiKind::Plain => plain_call_resolution(call_facts, call_facts.precision()),
            CallableAbiKind::EffectStep => CallResolution {
                callee_abi_kind: CallableAbiKind::EffectStep,
                callee_schema: Some(call_facts.callee_schema()),
                resolved_cases: schema_index.full_case_set(call_facts.callee_schema()),
                precision: call_facts.precision(),
            },
        },
    }
}

fn plain_call_resolution(
    call_facts: &CallSiteEffectFacts,
    precision: EffectPrecision,
) -> CallResolution {
    let resolved_cases = if let Some(schema) = call_facts.callee_step_schema() {
        CaseSet::new(schema, Vec::new())
    } else {
        call_facts.resolved_cases().clone()
    };
    CallResolution {
        callee_abi_kind: CallableAbiKind::Plain,
        callee_schema: None,
        resolved_cases,
        precision,
    }
}

fn finalize_body_blocks(
    current_schema: StepSchemaId,
    callable_resolved_cases: &CaseSet,
    body: &BodyEffectFacts,
    finalized_sites: &BTreeMap<crate::mir::SiteId, SiteEffectFacts>,
    schema_index: &SchemaProjectionIndex,
) -> BTreeMap<BasicBlockId, BlockEffectFacts> {
    let mut ambient_cases = BTreeMap::new();
    let mut local_cases = BTreeMap::new();
    for &block_id in body.blocks().keys() {
        let handled_cases = body
            .solver_facts()
            .handled_cases_for_block(block_id)
            .cloned()
            .unwrap_or_else(|| schema_index.empty_case_set(current_schema));
        ambient_cases.insert(
            block_id,
            subtract_case_set(callable_resolved_cases, &handled_cases),
        );

        let mut site_tags = BTreeSet::new();
        if let Some(site_ids) = body.solver_facts().block_sites().get(&block_id) {
            for site_id in site_ids {
                let Some(site) = finalized_sites.get(site_id) else {
                    continue;
                };
                let Some(site_cases) = site_cases_for_block(current_schema, site, schema_index)
                else {
                    continue;
                };
                site_tags.extend(site_cases.tags().iter().copied());
            }
        }
        let raw_local_cases = CaseSet::new(current_schema, site_tags.into_iter().collect());
        local_cases.insert(
            block_id,
            subtract_case_set(&raw_local_cases, &handled_cases),
        );
    }

    let mut outward_cases = local_cases.clone();
    loop {
        let mut changed = false;
        for &block_id in body.blocks().keys() {
            let mut tags = outward_cases
                .get(&block_id)
                .map(|cases| cases.tags().iter().copied().collect::<BTreeSet<_>>())
                .unwrap_or_default();
            if let Some(successors) = body.solver_facts().block_successors().get(&block_id) {
                for successor in successors {
                    if let Some(cases) = outward_cases.get(successor) {
                        tags.extend(cases.tags().iter().copied());
                    }
                }
            }
            let next = CaseSet::new(current_schema, tags.into_iter().collect());
            if outward_cases.get(&block_id) != Some(&next) {
                outward_cases.insert(block_id, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    body.blocks()
        .iter()
        .map(|(block_id, block_facts)| {
            (
                *block_id,
                BlockEffectFacts::new(
                    ambient_cases
                        .get(block_id)
                        .cloned()
                        .unwrap_or_else(|| schema_index.empty_case_set(current_schema)),
                    outward_cases
                        .get(block_id)
                        .cloned()
                        .unwrap_or_else(|| schema_index.empty_case_set(current_schema)),
                    block_facts.has_suspend_boundary(),
                    block_facts.has_handle_boundary(),
                ),
            )
        })
        .collect()
}

fn site_cases_for_block(
    current_schema: StepSchemaId,
    site: &SiteEffectFacts,
    schema_index: &SchemaProjectionIndex,
) -> Option<CaseSet> {
    match site {
        SiteEffectFacts::Call(facts) => {
            Some(schema_index.project_case_set(facts.resolved_cases(), current_schema))
        }
        SiteEffectFacts::ClassCtor(facts) => Some(facts.emitted_cases().clone()),
        SiteEffectFacts::Perform(facts) => {
            Some(schema_index.singleton(current_schema, facts.emitted_case()))
        }
        SiteEffectFacts::Resume(facts) => {
            Some(schema_index.project_case_set(facts.resolved_cases(), current_schema))
        }
        SiteEffectFacts::Handle(_) => None,
    }
}

fn subtract_case_set(source: &CaseSet, removed: &CaseSet) -> CaseSet {
    debug_assert_eq!(source.schema(), removed.schema());
    let removed = removed.tags().iter().copied().collect::<HashSet<_>>();
    CaseSet::new(
        source.schema(),
        source
            .tags()
            .iter()
            .copied()
            .filter(|tag| !removed.contains(tag))
            .collect(),
    )
}

fn body_needs_plain_local_control(sites: &BTreeMap<crate::mir::SiteId, SiteEffectFacts>) -> bool {
    sites.values().any(|site| match site {
        SiteEffectFacts::Call(call) => {
            matches!(call.callee_abi_kind(), CallableAbiKind::EffectStep)
                && !call.resolved_cases().is_empty()
        }
        SiteEffectFacts::ClassCtor(class_ctor) => !class_ctor.emitted_cases().is_empty(),
        SiteEffectFacts::Perform(_) | SiteEffectFacts::Resume(_) | SiteEffectFacts::Handle(_) => {
            true
        }
    })
}

fn derive_impl_plan(opt_level: OptLevel, resolved_outward_cases: &CaseSet) -> ImplPlan {
    match resolved_outward_cases.tags() {
        [] => ImplPlan::NoOutward,
        [single] if !matches!(opt_level, OptLevel::O0) => ImplPlan::SingleCase(*single),
        [_] | [_, _, ..] => ImplPlan::CanonicalFull,
    }
}

fn derive_callable_abi_kind(resolved_outward_cases: &CaseSet) -> CallableAbiKind {
    if resolved_outward_cases.is_empty() {
        CallableAbiKind::Plain
    } else {
        CallableAbiKind::EffectStep
    }
}

#[cfg(all(test, not(feature = "standalone-stage-crate")))]
mod tests {
    use std::collections::BTreeSet;

    use super::{EffectFactsSolverBudget, EffectFactsSolverConfig, MaterializedEffectFactsSolver};
    use crate::effect_facts::{
        CallSiteKind, CallableAbiKind, EffectPrecision, ImplPlan, SiteEffectFacts,
    };
    use crate::mir::materialize_for_dump_with_opt_level;
    use crate::mir::{
        BasicBlockId, CallKind, InstanceKey, MaterializedMirPassView, Rvalue, StatementKind,
        TerminatorKind,
    };
    use crate::opt::OptLevel;
    use crate::pipeline::{
        build_effect_facts_stage_output, load_direct_style_mir_stage_output_for_dump,
    };
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;

    fn session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    struct EffectFactsFixtureOutput {
        mir_stage_output: crate::pipeline::MirStageOutput,
        effect_facts_stage_output: crate::pipeline::EffectFactsStageOutput,
    }

    impl EffectFactsFixtureOutput {
        fn effect_facts(&self) -> &crate::effect_facts::MaterializedEffectFacts {
            self.effect_facts_stage_output.effect_facts()
        }

        fn materialized_pass_view(&self) -> MaterializedMirPassView<'_> {
            self.mir_stage_output.materialized_pass_view()
        }

        fn stable_dump(&self) -> String {
            self.effect_facts_stage_output.stable_dump()
        }
    }

    fn build_stage_output_for_source(
        source: &SourceFile,
        opt_level: OptLevel,
    ) -> EffectFactsFixtureOutput {
        let session = session();
        let mir_stage_output =
            load_direct_style_mir_stage_output_for_dump(&session, source).unwrap();
        let materialized =
            materialize_for_dump_with_opt_level(&session, source, opt_level).unwrap();
        let mir_stage_output = mir_stage_output.with_materialized_mir(materialized);
        let effect_facts_stage_output =
            build_effect_facts_stage_output(&session, source, &mir_stage_output).unwrap();
        EffectFactsFixtureOutput {
            mir_stage_output,
            effect_facts_stage_output,
        }
    }

    fn solve_with_config(
        source: &SourceFile,
        opt_level: OptLevel,
        config: EffectFactsSolverConfig,
    ) -> crate::effect_facts::MaterializedEffectFacts {
        let session = session();
        let mir_stage_output =
            load_direct_style_mir_stage_output_for_dump(&session, source).unwrap();
        let materialized =
            materialize_for_dump_with_opt_level(&session, source, opt_level).unwrap();
        let mir_stage_output = mir_stage_output.with_materialized_mir(materialized);
        let mut type_context = crate::effect_facts::EffectOwnedTypeContext::from_mir_types(
            &mir_stage_output.materialized_mir().types,
        );
        let seeded =
            crate::effect_facts::MaterializedEffectFactsBuilder::from_materialized_snapshot(
                mir_stage_output
                    .hir_semantic_artifact()
                    .expect("MIR handoff 应携带 HIR semantic artifact"),
                mir_stage_output.materialized_mir(),
                mir_stage_output.mir_facts(),
                &mut type_context,
            )
            .build()
            .unwrap();
        MaterializedEffectFactsSolver::with_config(config).solve(seeded)
    }

    fn callable_facts_for<'a>(
        facts: &'a crate::effect_facts::MaterializedEffectFacts,
        fqn: &str,
    ) -> (
        &'a InstanceKey,
        &'a crate::effect_facts::CallableEffectFacts,
    ) {
        facts
            .callable_facts()
            .iter()
            .find(|(key, _)| key.template.fqn == fqn || key.template.fqn.ends_with(fqn))
            .unwrap_or_else(|| panic!("fixture callable 应在 solved facts 中可见: {fqn}"))
    }

    fn case_fqns(
        facts: &crate::effect_facts::MaterializedEffectFacts,
        case_set: &crate::effect_facts::CaseSet,
    ) -> BTreeSet<String> {
        if case_set.is_empty() {
            return BTreeSet::new();
        }
        let schema = facts
            .step_schemas()
            .get(&case_set.schema())
            .expect("case set 应引用已存在的 step schema");
        case_set
            .tags()
            .iter()
            .map(|tag| {
                schema
                    .cases()
                    .iter()
                    .find(|case| case.case_tag() == *tag)
                    .expect("case tag 应落在对应 schema 中")
                    .concrete_op_key()
                    .instance_key()
                    .template
                    .fqn
                    .clone()
            })
            .collect()
    }

    fn direct_scc_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_solver_direct_scc.scoop",
            r#"
package sample

effect Ping {
    fun hit(): Unit
}

fun leaf(): Unit / Ping {
    Ping.hit()
}

fun loopA(flag: Bool): Unit / Ping {
    if (flag) {
        leaf()
    } else {
        loopB(true)
    }
}

fun loopB(flag: Bool): Unit / Ping {
    if (flag) {
        loopA(false)
    } else {
        Ping.hit()
    }
}
"#,
        )
    }

    fn candidate_union_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_solver_candidate_union.scoop",
            r#"
package sample

effect Alpha {
    fun go(): Unit
}

effect Beta {
    fun go(): Unit
}

open class Base() {
    open fun run(): Unit / (Alpha + Beta) {}
}

class Left() : Base() {
    override fun run(): Unit / (Alpha + Beta) {
        Alpha.go()
    }
}

class Right() : Base() {
    override fun run(): Unit / (Alpha + Beta) {
        Beta.go()
    }
}

fun call(base: Base): Unit / (Alpha + Beta) {
    base.run()
}
"#,
        )
    }

    fn dynamic_fallback_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_solver_dynamic_fallback.scoop",
            r#"
package sample

effect Alpha {
    fun go(): Unit
}

effect Beta {
    fun go(): Unit
}

fun callValue(f: () -> Unit / (Alpha + Beta)): Unit / (Alpha + Beta) {
    f()
}
"#,
        )
    }

    fn higher_order_handled_function_value_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_solver_higher_order_handled_function_value.scoop",
            r#"
package sample

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

enum Mode {
    Pure,
    Effectful(val seed: Int),
}

fun choose(mode: Mode): () -> Int / Ask {
    when (mode) {
        Pure -> {
            val thunk: () -> Int / Ask = { 5 }
            thunk
        }
        Effectful(seed) -> {
            val thunk: () -> Int / Ask = { Ask.ask(seed) }
            thunk
        }
    }
}

fun drive(mode: Mode): Int {
    val result: Int = handle {
        choose(mode)()
    } on {
        Ask.ask(seed), k -> {
            println("caught")
            println(seed.toString())
            k.resume(seed + 1)
        }
    }
    println(result.toString())
    result
}

fun main() {
    drive(Mode.Pure)
    drive(Effectful(9))
}
"#,
        )
    }

    fn mixed_handle_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_solver_block_facts.scoop",
            r#"
package sample

effect Inner {
    fun ping(): Unit
}

effect Outer {
    fun pong(): Unit
}

fun mixed(): Unit / (Inner + Outer) {
    handle {
        Inner.ping()
    } on {
        Inner.ping() -> ()
    }
    Outer.pong()
}
"#,
        )
    }

    fn nested_handle_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_solver_nested_handle.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun nested_self_contained(): Int {
    return handle {
        val inner: Int = handle {
            Inner.go()
            0
        } on {
            Inner.go() -> 1
        }
        inner + 10
    } on {
        Outer.again() -> 99
    }
}

fun nested_may_suspend_outward(): Int {
    return handle {
        val inner: Int = handle {
            Inner.go()
            0
        } on {
            Inner.go() -> 1
        } finally {
            Outer.again()
        }
        inner + 10
    } on {
        Outer.again() -> 99
    }
}
"#,
        )
    }

    fn handle_call_subset_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_solver_handle_call_subset.scoop",
            r#"
package sample

effect Alpha {
    fun go(): Unit
}

effect Beta {
    fun go(): Unit
}

fun emit_alpha(): Unit / (Alpha + Beta) {
    Alpha.go()
}

fun outer(): Unit / Beta {
    handle {
        emit_alpha()
    } on {
        Alpha.go() -> ()
    }
}
"#,
        )
    }

    fn handle_body_call_outward_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_solver_handle_body_call_outward.scoop",
            r#"
package sample

effect Alpha {
    fun go(): Unit
}

effect Beta {
    fun go(): Unit
}

fun emit_beta(): Unit / Beta {
    Beta.go()
}

fun outer(): Unit / (Alpha + Beta) {
    handle {
        emit_beta()
    } on {
        Alpha.go() -> ()
    }
}
"#,
        )
    }

    fn pure_plain_abi_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_solver_pure_plain_abi.scoop",
            r#"
package sample

fun pure(x: Int): Int {
    return x + 1
}

fun caller(): Int {
    return pure(41)
}
"#,
        )
    }

    fn plain_and_effect_dynamic_call_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_solver_plain_and_effect_dynamic_call.scoop",
            r#"
package sample

import scoop.core.Raise

effect Boom {
    fun hit(): Unit
}

fun pure(x: Int): Int {
    return x + 1
}

fun plainHandled(x: Int): Int {
    return handle {
        Raise.raise(x)
        0
    } on {
        Raise.raise(e) -> e
    }
}

fun caller(): Int {
    return plainHandled(41)
}

fun callEffectTyped(f: (Int) -> Int / Boom): Int / Boom {
    return f(1)
}
"#,
        )
    }

    #[test]
    fn callable_effect_facts_no_outward_uses_plain_abi_after_solver() {
        let output =
            build_stage_output_for_source(&plain_and_effect_dynamic_call_source(), OptLevel::O2);
        let facts = output.effect_facts();

        let (_, pure_facts) = callable_facts_for(facts, "sample.pure");
        assert_eq!(pure_facts.call_abi_kind(), CallableAbiKind::Plain);
        assert!(pure_facts.body_step_schema().is_none());
        assert!(pure_facts.invoke_args_tuple_ty_opt().is_none());
        assert!(pure_facts.resolved_outward_cases().is_empty());
        assert!(!pure_facts.needs_reentry());
        assert!(matches!(pure_facts.impl_plan(), ImplPlan::NoOutward));

        let (_, caller_facts) = callable_facts_for(facts, "sample.caller");
        assert_eq!(caller_facts.call_abi_kind(), CallableAbiKind::Plain);
        assert!(caller_facts.body_step_schema().is_none());
        assert!(caller_facts.resolved_outward_cases().is_empty());

        let (_, handled_facts) = callable_facts_for(facts, "sample.plainHandled");
        assert_eq!(handled_facts.call_abi_kind(), CallableAbiKind::Plain);
        assert!(handled_facts.body_step_schema().is_none());
        assert!(handled_facts.resolved_outward_cases().is_empty());

        let (_, dynamic_facts) = callable_facts_for(facts, "sample.callEffectTyped");
        assert_eq!(dynamic_facts.call_abi_kind(), CallableAbiKind::EffectStep);
        assert!(dynamic_facts.body_step_schema().is_some());
        assert_eq!(
            case_fqns(facts, dynamic_facts.resolved_outward_cases()),
            ["sample.Boom.hit".to_string()].into_iter().collect()
        );
    }

    #[test]
    fn step_schema_not_published_for_plain_body_after_solver() {
        let output = build_stage_output_for_source(&pure_plain_abi_source(), OptLevel::O2);
        let facts = output.effect_facts();

        assert!(facts.step_schemas().is_empty());
        for callable in facts.callable_facts().values() {
            assert_eq!(callable.call_abi_kind(), CallableAbiKind::Plain);
            assert!(callable.body_step_schema().is_none());
            assert!(callable.resolved_outward_cases().is_empty());
        }

        let dump = output.stable_dump();
        assert!(dump.contains("step_schemas:\n  <none>"));
        assert!(dump.contains("call_abi_kind: Plain"));
        assert!(dump.contains("step_schema: <none>"));
    }

    #[test]
    fn call_site_facts_distinguish_plain_call_and_effect_adapter_after_solver() {
        let output =
            build_stage_output_for_source(&plain_and_effect_dynamic_call_source(), OptLevel::O2);
        let facts = output.effect_facts();
        let pass_view = output.materialized_pass_view();

        let (caller_key, _) = callable_facts_for(facts, "sample.caller");
        let caller_body = pass_view
            .instance(caller_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("caller 应有 canonical body");
        let caller_site_id = caller_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let Rvalue::Call {
                    site_id,
                    kind: CallKind::Direct { callee_fqn, .. },
                    ..
                } = value
                else {
                    return None;
                };
                (callee_fqn == "sample.plainHandled").then_some(*site_id)
            })
            .expect("caller 应包含 direct plain call site");
        let SiteEffectFacts::Call(plain_call) = facts
            .body(caller_key)
            .and_then(|body| body.site(caller_site_id))
            .expect("plain call site 应有 facts")
        else {
            panic!("plain call site 应产生 CallSiteEffectFacts");
        };
        assert_eq!(plain_call.kind(), CallSiteKind::Direct);
        assert_eq!(plain_call.callee_abi_kind(), CallableAbiKind::Plain);
        assert!(plain_call.callee_step_schema().is_none());
        assert!(plain_call.resolved_cases().is_empty());
        assert_eq!(plain_call.precision(), EffectPrecision::Precise);

        let (dynamic_key, _) = callable_facts_for(facts, "sample.callEffectTyped");
        let dynamic_body = pass_view
            .instance(dynamic_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("callEffectTyped 应有 canonical body");
        let dynamic_site_id = dynamic_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let Rvalue::Call {
                    site_id,
                    kind: CallKind::FunValue { .. },
                    ..
                } = value
                else {
                    return None;
                };
                Some(*site_id)
            })
            .expect("callEffectTyped 应包含 effect-typed dynamic call site");
        let SiteEffectFacts::Call(dynamic_call) = facts
            .body(dynamic_key)
            .and_then(|body| body.site(dynamic_site_id))
            .expect("dynamic call site 应有 facts")
        else {
            panic!("dynamic call site 应产生 CallSiteEffectFacts");
        };
        assert_eq!(dynamic_call.kind(), CallSiteKind::FunValue);
        assert_eq!(dynamic_call.callee_abi_kind(), CallableAbiKind::EffectStep);
        assert!(dynamic_call.callee_step_schema().is_some());
        assert_eq!(
            case_fqns(facts, dynamic_call.resolved_cases()),
            ["sample.Boom.hit".to_string()].into_iter().collect()
        );
        assert_eq!(dynamic_call.precision(), EffectPrecision::Widened);
    }

    #[test]
    fn effect_solver_propagates_direct_scc_and_known_callee_cases() {
        let output = build_stage_output_for_source(&direct_scc_source(), OptLevel::O2);
        let facts = output.effect_facts();

        let (_, leaf_facts) = callable_facts_for(facts, "sample.leaf");
        let (_, loop_a_facts) = callable_facts_for(facts, "sample.loopA");
        let (_, loop_b_facts) = callable_facts_for(facts, "sample.loopB");

        let expected: BTreeSet<String> = ["sample.Ping.hit".to_string()].into_iter().collect();
        assert_eq!(
            case_fqns(facts, leaf_facts.resolved_outward_cases()),
            expected.clone()
        );
        assert_eq!(
            case_fqns(facts, loop_a_facts.resolved_outward_cases()),
            expected.clone()
        );
        assert_eq!(
            case_fqns(facts, loop_b_facts.resolved_outward_cases()),
            expected.clone()
        );

        let (loop_a_key, _) = callable_facts_for(facts, "sample.loopA");
        let pass_view = output.materialized_pass_view();
        let loop_a_body = pass_view
            .instance(loop_a_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("loopA 应有 canonical body");
        let loop_a_body_facts = facts.body(loop_a_key).expect("loopA 应有 body facts");
        let direct_site_id = loop_a_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let Rvalue::Call {
                    site_id,
                    kind: CallKind::Direct { callee_fqn, .. },
                    ..
                } = value
                else {
                    return None;
                };
                (callee_fqn == "sample.leaf").then_some(*site_id)
            })
            .expect("loopA 应包含 direct known-callee call site");
        let SiteEffectFacts::Call(call_facts) = loop_a_body_facts
            .site(direct_site_id)
            .expect("known-callee site 应可通过 SiteId 查询")
        else {
            panic!("known-callee site 应产生 CallSiteEffectFacts");
        };
        assert_eq!(call_facts.precision(), EffectPrecision::Precise);
        assert_eq!(case_fqns(facts, call_facts.resolved_cases()), expected);
    }

    #[test]
    fn effect_solver_unions_candidate_sets_and_dynamic_fallback() {
        let candidate_output =
            build_stage_output_for_source(&candidate_union_source(), OptLevel::O2);
        let candidate_facts = candidate_output.effect_facts();
        let (call_key, call_facts) = callable_facts_for(candidate_facts, "sample.call");
        assert_eq!(
            case_fqns(candidate_facts, call_facts.resolved_outward_cases()),
            ["sample.Alpha.go".to_string(), "sample.Beta.go".to_string()]
                .into_iter()
                .collect()
        );
        let candidate_pass_view = candidate_output.materialized_pass_view();
        let call_body = candidate_pass_view
            .instance(call_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("call 应有 canonical body");
        let call_body_facts = candidate_facts
            .body(call_key)
            .expect("call 应有 body facts");
        let candidate_site_id = call_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let Rvalue::Call {
                    site_id,
                    kind: CallKind::Virtual { .. },
                    ..
                } = value
                else {
                    return None;
                };
                Some(*site_id)
            })
            .expect("call 应包含 virtual candidate-set site");
        let SiteEffectFacts::Call(candidate_site) = call_body_facts
            .site(candidate_site_id)
            .expect("candidate site 应可通过 SiteId 查询")
        else {
            panic!("candidate site 应产生 CallSiteEffectFacts");
        };
        assert_eq!(candidate_site.precision(), EffectPrecision::Precise);
        assert_eq!(
            case_fqns(candidate_facts, candidate_site.resolved_cases()),
            ["sample.Alpha.go".to_string(), "sample.Beta.go".to_string()]
                .into_iter()
                .collect()
        );

        let dynamic_output =
            build_stage_output_for_source(&dynamic_fallback_source(), OptLevel::O2);
        let dynamic_facts = dynamic_output.effect_facts();
        let (call_value_key, call_value_facts) =
            callable_facts_for(dynamic_facts, "sample.callValue");
        assert_eq!(
            case_fqns(dynamic_facts, call_value_facts.resolved_outward_cases()),
            ["sample.Alpha.go".to_string(), "sample.Beta.go".to_string()]
                .into_iter()
                .collect()
        );
        let dynamic_pass_view = dynamic_output.materialized_pass_view();
        let call_value_body = dynamic_pass_view
            .instance(call_value_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("callValue 应有 canonical body");
        let call_value_body_facts = dynamic_facts
            .body(call_value_key)
            .expect("callValue 应有 body facts");
        let dynamic_site_id = call_value_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let Rvalue::Call {
                    site_id,
                    kind: CallKind::FunValue { .. },
                    ..
                } = value
                else {
                    return None;
                };
                Some(*site_id)
            })
            .expect("callValue 应包含 dynamic fallback site");
        let SiteEffectFacts::Call(dynamic_site) = call_value_body_facts
            .site(dynamic_site_id)
            .expect("dynamic site 应可通过 SiteId 查询")
        else {
            panic!("dynamic site 应产生 CallSiteEffectFacts");
        };
        assert_eq!(dynamic_site.precision(), EffectPrecision::Widened);
        assert_eq!(
            case_fqns(dynamic_facts, dynamic_site.resolved_cases()),
            ["sample.Alpha.go".to_string(), "sample.Beta.go".to_string()]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn effect_solver_consumes_higher_order_function_value_call_in_handle() {
        let output = build_stage_output_for_source(
            &higher_order_handled_function_value_source(),
            OptLevel::O0,
        );
        let facts = output.effect_facts();

        let (_, drive_facts) = callable_facts_for(facts, "sample.drive");
        let drive_outward = case_fqns(facts, drive_facts.resolved_outward_cases());
        assert!(
            !drive_outward.contains("sample.Ask.ask"),
            "drive 的 handle 应消费 choose(mode)() 的 Ask.ask，不应向 main 泄漏: {drive_outward:?}"
        );

        let (_, main_facts) = callable_facts_for(facts, "sample.main");
        let main_outward = case_fqns(facts, main_facts.resolved_outward_cases());
        assert!(
            !main_outward.contains("sample.Ask.ask"),
            "main 调用 drive 时不应看到 handled Ask.ask: {main_outward:?}"
        );
    }

    #[test]
    fn effect_solver_budget_exhaustion_widens_affected_callable() {
        let source = candidate_union_source();
        let facts = solve_with_config(
            &source,
            OptLevel::O2,
            EffectFactsSolverConfig::with_budget(
                OptLevel::O2,
                EffectFactsSolverBudget {
                    max_scc_nodes: 256,
                    max_scc_edges: 1024,
                    max_scc_iterations: 16,
                    max_candidate_union_size: 1,
                },
            ),
        );
        let (call_key, call_facts) = callable_facts_for(&facts, "sample.call");
        let full_cases = facts
            .step_schemas()
            .get(&call_facts.step_schema())
            .expect("call 应有 step schema")
            .cases()
            .iter()
            .map(|case| case.concrete_op_key().instance_key().template.fqn.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            case_fqns(&facts, call_facts.resolved_outward_cases()),
            full_cases
        );

        let body_facts = facts.body(call_key).expect("call 应有 body facts");
        let candidate_site = body_facts
            .sites()
            .values()
            .find_map(|site| match site {
                SiteEffectFacts::Call(call_facts) => Some(call_facts),
                SiteEffectFacts::ClassCtor(_)
                | SiteEffectFacts::Perform(_)
                | SiteEffectFacts::Resume(_)
                | SiteEffectFacts::Handle(_) => None,
            })
            .expect("call 应包含 virtual candidate-set site");
        assert_eq!(candidate_site.precision(), EffectPrecision::Widened);
    }

    #[test]
    fn impl_plan_tracks_needs_reentry_and_opt_level_policy() {
        let source = direct_scc_source();
        let o2 = build_stage_output_for_source(&source, OptLevel::O2);
        let (_, o2_leaf) = callable_facts_for(o2.effect_facts(), "sample.leaf");
        assert!(o2_leaf.needs_reentry());
        assert!(matches!(o2_leaf.impl_plan(), ImplPlan::SingleCase(_)));

        let o0 = build_stage_output_for_source(&source, OptLevel::O0);
        let (_, o0_leaf) = callable_facts_for(o0.effect_facts(), "sample.leaf");
        assert!(o0_leaf.needs_reentry());
        assert!(matches!(o0_leaf.impl_plan(), ImplPlan::CanonicalFull));
    }

    #[test]
    fn block_effect_facts_finalize_ambient_and_outward_cases() {
        let output = build_stage_output_for_source(&mixed_handle_source(), OptLevel::O2);
        let facts = output.effect_facts();
        let (mixed_key, mixed_facts) = callable_facts_for(facts, "sample.mixed");
        assert_eq!(
            case_fqns(facts, mixed_facts.resolved_outward_cases()),
            ["sample.Outer.pong".to_string()].into_iter().collect()
        );

        let pass_view = output.materialized_pass_view();
        let body = pass_view
            .instance(mixed_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("mixed 应有 canonical body");
        let body_facts = facts.body(mixed_key).expect("mixed 应有 body facts");

        let mut handle_block_id = None;
        let mut handle_body_target = None;
        let mut outer_perform_block_id = None;
        for (index, block) in body.blocks.iter().enumerate() {
            let block_id = BasicBlockId::from_raw(index as u32);
            match &block.terminator.kind {
                TerminatorKind::Handle { body_target, .. } => {
                    handle_block_id = Some(block_id);
                    handle_body_target = Some(*body_target);
                }
                TerminatorKind::Perform { op_fqn, .. } if op_fqn == "sample.Outer.pong" => {
                    outer_perform_block_id = Some(block_id);
                }
                TerminatorKind::Perform { .. }
                | TerminatorKind::Return { .. }
                | TerminatorKind::ResumeUnwind
                | TerminatorKind::Goto { .. }
                | TerminatorKind::CondBr { .. }
                | TerminatorKind::Unreachable
                | TerminatorKind::Todo(_) => {}
            }
        }

        let handle_body_block = body_facts
            .block(handle_body_target.expect("mixed 应有 handle body block"))
            .expect("handle body block 应有 final block facts");
        assert_eq!(
            case_fqns(facts, handle_body_block.ambient_cases()),
            ["sample.Outer.pong".to_string()].into_iter().collect()
        );
        assert_eq!(
            case_fqns(facts, handle_body_block.outward_cases()),
            ["sample.Outer.pong".to_string()].into_iter().collect()
        );

        let handle_block = body_facts
            .block(handle_block_id.expect("mixed 应有 handle site block"))
            .expect("handle site block 应有 final block facts");
        assert!(handle_block.has_handle_boundary());
        assert_eq!(
            case_fqns(facts, handle_block.outward_cases()),
            ["sample.Outer.pong".to_string()].into_iter().collect()
        );

        let outer_perform_block = body_facts
            .block(outer_perform_block_id.expect("mixed 应有 outer perform block"))
            .expect("outer perform block 应有 final block facts");
        assert!(outer_perform_block.has_suspend_boundary());
        assert_eq!(
            case_fqns(facts, outer_perform_block.ambient_cases()),
            ["sample.Outer.pong".to_string()].into_iter().collect()
        );
        assert_eq!(
            case_fqns(facts, outer_perform_block.outward_cases()),
            ["sample.Outer.pong".to_string()].into_iter().collect()
        );
    }

    #[test]
    fn block_effect_facts_preserve_nested_handle_classification_after_solver() {
        let output = build_stage_output_for_source(&nested_handle_source(), OptLevel::O2);
        let facts = output.effect_facts();
        let pass_view = output.materialized_pass_view();

        let (self_key, self_facts) = callable_facts_for(facts, "sample.nested_self_contained");
        assert!(self_facts.resolved_outward_cases().is_empty());
        let self_body = pass_view
            .instance(self_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("nested_self_contained 应有 canonical body");
        let self_body_facts = facts
            .body(self_key)
            .expect("nested_self_contained 应有 body facts");
        let self_inner_handle = self_body
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator.kind {
                TerminatorKind::Handle { site_id, .. } => self_body_facts.site(*site_id),
                _ => None,
            })
            .find_map(|site| match site {
                SiteEffectFacts::Handle(handle_facts)
                    if case_fqns(facts, handle_facts.handled_cases())
                        == ["sample.Inner.go".to_string()].into_iter().collect() =>
                {
                    Some(handle_facts)
                }
                SiteEffectFacts::Call(_)
                | SiteEffectFacts::ClassCtor(_)
                | SiteEffectFacts::Perform(_)
                | SiteEffectFacts::Resume(_)
                | SiteEffectFacts::Handle(_) => None,
            })
            .expect("nested_self_contained 应包含 inner handle site");
        assert_eq!(
            self_inner_handle.nested_handle_classification(),
            crate::effect_facts::NestedHandleClassification::SelfContained
        );

        let (may_key, may_facts) = callable_facts_for(facts, "sample.nested_may_suspend_outward");
        assert_eq!(
            case_fqns(facts, may_facts.resolved_outward_cases()),
            ["sample.Outer.again".to_string()].into_iter().collect()
        );
        let may_body = pass_view
            .instance(may_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("nested_may_suspend_outward 应有 canonical body");
        let may_body_facts = facts
            .body(may_key)
            .expect("nested_may_suspend_outward 应有 body facts");
        let may_inner_handle = may_body
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator.kind {
                TerminatorKind::Handle { site_id, .. } => may_body_facts.site(*site_id),
                _ => None,
            })
            .find_map(|site| match site {
                SiteEffectFacts::Handle(handle_facts)
                    if case_fqns(facts, handle_facts.handled_cases())
                        == ["sample.Inner.go".to_string()].into_iter().collect() =>
                {
                    Some(handle_facts)
                }
                SiteEffectFacts::Call(_)
                | SiteEffectFacts::ClassCtor(_)
                | SiteEffectFacts::Perform(_)
                | SiteEffectFacts::Resume(_)
                | SiteEffectFacts::Handle(_) => None,
            })
            .expect("nested_may_suspend_outward 应包含 inner handle site");
        assert_eq!(
            may_inner_handle.nested_handle_classification(),
            crate::effect_facts::NestedHandleClassification::MaySuspendOutward
        );
    }

    #[test]
    fn effect_solver_recomputes_handle_outward_from_finalized_call_sites() {
        let output = build_stage_output_for_source(&handle_call_subset_source(), OptLevel::O2);
        let facts = output.effect_facts();
        let pass_view = output.materialized_pass_view();

        let (outer_key, outer_facts) = callable_facts_for(facts, "sample.outer");
        assert!(
            outer_facts.resolved_outward_cases().is_empty(),
            "outer 的 final resolved_outward_cases 不应保留 builder seed 的 Beta 上界"
        );

        let outer_body = pass_view
            .instance(outer_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("outer 应有 canonical body");
        let outer_body_facts = facts.body(outer_key).expect("outer 应有 body facts");
        let handle_facts = outer_body
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator.kind {
                TerminatorKind::Handle { site_id, .. } => outer_body_facts.site(*site_id),
                _ => None,
            })
            .find_map(|site| match site {
                SiteEffectFacts::Handle(handle_facts) => Some(handle_facts),
                SiteEffectFacts::Call(_)
                | SiteEffectFacts::ClassCtor(_)
                | SiteEffectFacts::Perform(_)
                | SiteEffectFacts::Resume(_) => None,
            })
            .expect("outer 应包含 handle site");
        assert!(
            handle_facts.body_outward_cases().is_empty(),
            "handle body_outward_cases 应按 finalized call site 重算，而不是保留 seed 的 Beta 上界"
        );
        assert_eq!(
            handle_facts.nested_handle_classification(),
            crate::effect_facts::NestedHandleClassification::SelfContained
        );
    }

    #[test]
    fn effect_solver_keeps_handle_body_outward_for_plain_call_effects() {
        let output =
            build_stage_output_for_source(&handle_body_call_outward_source(), OptLevel::O2);
        let facts = output.effect_facts();
        let pass_view = output.materialized_pass_view();

        let (outer_key, outer_facts) = callable_facts_for(facts, "sample.outer");
        assert_eq!(
            case_fqns(facts, outer_facts.resolved_outward_cases()),
            ["sample.Beta.go".to_string()].into_iter().collect(),
            "outer 的 final resolved_outward_cases 应保留 handle body 内 plain call 暴露的 Beta"
        );

        let outer_body = pass_view
            .instance(outer_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("outer 应有 canonical body");
        let outer_body_facts = facts.body(outer_key).expect("outer 应有 body facts");
        let handle_facts = outer_body
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator.kind {
                TerminatorKind::Handle { site_id, .. } => outer_body_facts.site(*site_id),
                _ => None,
            })
            .find_map(|site| match site {
                SiteEffectFacts::Handle(handle_facts) => Some(handle_facts),
                SiteEffectFacts::Call(_)
                | SiteEffectFacts::ClassCtor(_)
                | SiteEffectFacts::Perform(_)
                | SiteEffectFacts::Resume(_) => None,
            })
            .expect("outer 应包含 handle site");
        assert_eq!(
            case_fqns(facts, handle_facts.body_outward_cases()),
            ["sample.Beta.go".to_string()].into_iter().collect(),
            "handle body_outward_cases 不应丢失 body 内 plain call 的 outward effect"
        );
        assert_eq!(
            handle_facts.nested_handle_classification(),
            crate::effect_facts::NestedHandleClassification::MaySuspendOutward
        );
    }
}
