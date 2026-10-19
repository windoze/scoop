//! Dominance and loop-carried aliases for value replacement, without SSA.

use std::collections::BTreeSet;

use super::{
    graph, index, mir,
    origins::{AllocationId, Site, Sources, defined_local},
};

pub(super) struct ControlFlow {
    edges: Vec<Vec<usize>>,
    order: Vec<usize>,
    dominators: Vec<Option<usize>>,
    back_edges: Vec<(usize, usize)>,
}

impl ControlFlow {
    pub fn new(body: &mir::Body) -> Self {
        let edges = graph::block_edges(body);
        let entry = index(body.entry);
        let (order, back_edges) = traversal(&edges, entry);
        let mut predecessors = vec![Vec::new(); edges.len()];
        let mut rank = vec![usize::MAX; edges.len()];
        for (position, &block) in order.iter().enumerate() {
            rank[block] = position;
            for &target in &edges[block] {
                predecessors[target].push(block);
            }
        }
        let mut dominators = vec![None; edges.len()];
        dominators[entry] = Some(entry);
        loop {
            let mut changed = false;
            for &block in order.iter().skip(1) {
                let mut incoming = predecessors[block]
                    .iter()
                    .copied()
                    .filter(|&predecessor| dominators[predecessor].is_some());
                let mut common = incoming
                    .next()
                    .expect("reachable block has a visited predecessor");
                for mut other in incoming {
                    while common != other {
                        if rank[common] > rank[other] {
                            common = dominators[common].expect("visited dominator");
                        } else {
                            other = dominators[other].expect("visited dominator");
                        }
                    }
                }
                changed |= dominators[block] != Some(common);
                dominators[block] = Some(common);
            }
            if !changed {
                break;
            }
        }
        Self {
            edges,
            order,
            dominators,
            back_edges,
        }
    }

    pub fn reachable(&self, block: mir::BlockId) -> bool {
        self.dominators[index(block)].is_some()
    }

    pub fn dominates(&self, creation: Site, usage: Site) -> bool {
        if creation.block == usage.block {
            return creation.statement < usage.statement;
        }
        self.block_dominates(index(creation.block), index(usage.block))
    }

    fn block_dominates(&self, creation: usize, mut block: usize) -> bool {
        if creation == block {
            return true;
        }
        while let Some(parent) = self.dominators[block] {
            if parent == creation {
                return true;
            }
            if parent == block {
                break;
            }
            block = parent;
        }
        false
    }

    pub fn loop_carried(&self, body: &mir::Body, sources: &Sources) -> BTreeSet<AllocationId> {
        if self.back_edges.is_empty() {
            return BTreeSet::new();
        }
        let mut uses = vec![BTreeSet::new(); self.edges.len()];
        let mut definitions = uses.clone();
        for (id, block) in body.blocks.iter() {
            if !self.reachable(id) {
                continue;
            }
            let used = &mut uses[index(id)];
            let defined = &mut definitions[index(id)];
            for statement in &block.statements {
                mir::visit_statement_exprs(statement, &mut |expr| {
                    read_before_definition(expr, sources, defined, used);
                });
                if let Some(local) = defined_local(statement) {
                    defined.insert(local);
                }
            }
            mir::visit_terminator_exprs(&block.terminator, &mut |expr| {
                read_before_definition(expr, sources, defined, used);
            });
        }
        let mut live = vec![BTreeSet::new(); self.edges.len()];
        loop {
            let mut changed = false;
            for &block in self.order.iter().rev() {
                let mut incoming = uses[block].clone();
                for &successor in &self.edges[block] {
                    incoming.extend(live[successor].difference(&definitions[block]).copied());
                }
                changed |= incoming != live[block];
                live[block] = incoming;
            }
            if !changed {
                break;
            }
        }
        let cyclic = graph::cyclic_nodes(&self.edges);
        self.back_edges
            .iter()
            .flat_map(|&(from, target)| {
                let natural = self.block_dominates(target, from);
                let cyclic = &cyclic;
                live[target]
                    .iter()
                    .flat_map(|&local| &sources.locals[index(local)].candidates)
                    .copied()
                    .filter(move |&id| {
                        let creation = index(sources.allocations[id].site.block);
                        // An outer iteration may reuse one instance throughout
                        // an inner loop. Only its creation loop needs isolation.
                        let outside =
                            natural && creation != target && self.block_dominates(creation, target);
                        cyclic[creation] && !outside
                    })
            })
            .collect()
    }
}

fn read_before_definition(
    expr: &mir::Expr,
    sources: &Sources,
    defined: &BTreeSet<mir::LocalId>,
    used: &mut BTreeSet<mir::LocalId>,
) {
    if let mir::ExprKind::Local(local) = expr.kind
        && !sources.locals[index(local)].candidates.is_empty()
        && !defined.contains(&local)
    {
        used.insert(local);
    }
}

/// DFS back edges cut every cycle, including irreducible control flow.
fn traversal(edges: &[Vec<usize>], entry: usize) -> (Vec<usize>, Vec<(usize, usize)>) {
    let mut color = vec![0_u8; edges.len()];
    let mut order = Vec::new();
    let mut back_edges = Vec::new();
    let mut stack = vec![(entry, 0)];
    color[entry] = 1;
    while let Some((block, next)) = stack.last_mut() {
        if let Some(&target) = edges[*block].get(*next) {
            *next += 1;
            match color[target] {
                0 => {
                    color[target] = 1;
                    stack.push((target, 0));
                }
                1 => back_edges.push((*block, target)),
                _ => {}
            }
        } else {
            color[*block] = 2;
            order.push(*block);
            stack.pop();
        }
    }
    order.reverse();
    (order, back_edges)
}
