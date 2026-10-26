use super::*;

pub(super) fn select(function: &mir::Function, addressed: &HashSet<mir::LocalId>) -> Vec<Source> {
    let body = &function.body;
    let mut queries = Vec::new();
    let mut definitions = HashMap::new();
    for (id, block) in body.blocks.iter() {
        let mut visit = |expression: &mir::Expr| {
            if let mir::ExprKind::IsInstance { operand, check_ty } = &expression.kind
                && let mir::Type::Interface(interface) = **check_ty
                && let Some(local) = interface_tables::source_local(operand)
                && !addressed.contains(&local)
            {
                queries.push((id, (local, interface)));
            }
        };
        for statement in &block.statements {
            mir::visit_statement_exprs(statement, &mut visit);
            if let Some(local) = interface_tables::written_local(statement) {
                definitions
                    .entry(local)
                    .and_modify(|block| *block = None)
                    .or_insert(Some(id));
            }
        }
        mir::visit_terminator_exprs(&block.terminator, &mut visit);
    }
    if queries.is_empty() {
        return Vec::new();
    }
    let cyclic = cyclic_blocks(body);
    let parameters: HashSet<_> = function.params.iter().map(|param| param.local).collect();
    let mut seen = HashSet::new();
    queries
        .into_iter()
        .filter_map(|(block, source)| {
            let stable = match definitions.get(&source.0) {
                Some(Some(definition)) => !cyclic.contains(definition),
                None => parameters.contains(&source.0),
                Some(None) => false,
            };
            (stable && cyclic.contains(&block) && seen.insert(source)).then_some(source)
        })
        .collect()
}

/// Find exactly the blocks in CFG cycles, including exceptional retry paths.
/// This does not infer or modify the separate MIR loop-poll plan.
fn cyclic_blocks(body: &mir::Body) -> HashSet<mir::BlockId> {
    let mut successors = HashMap::new();
    let mut predecessors = HashMap::<_, Vec<_>>::new();
    for (id, block) in body.blocks.iter() {
        let mut edges = Vec::new();
        match block.terminator {
            mir::Terminator::Goto(target) => edges.push(target),
            mir::Terminator::Branch {
                then_block,
                else_block,
                ..
            } => {
                edges.extend([then_block, else_block]);
            }
            mir::Terminator::Throw { unwind, .. } | mir::Terminator::Rethrow { unwind } => {
                edges.extend(unwind);
            }
            _ => {}
        }
        edges.extend(block.unwind);
        for target in &edges {
            predecessors.entry(*target).or_default().push(id);
        }
        successors.insert(id, edges);
    }

    // Iterative Kosaraju traversal avoids a host stack limit for large bodies.
    let mut visited = HashSet::new();
    let mut order = Vec::new();
    let mut stack = vec![(body.entry, false)];
    while let Some((block, finished)) = stack.pop() {
        if finished {
            order.push(block);
        } else if visited.insert(block) {
            stack.push((block, true));
            stack.extend(
                successors[&block]
                    .iter()
                    .rev()
                    .map(|target| (*target, false)),
            );
        }
    }
    let reachable = visited;
    let mut assigned = HashSet::new();
    let mut cyclic = HashSet::new();
    for block in order.into_iter().rev() {
        if assigned.contains(&block) {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![block];
        while let Some(block) = stack.pop() {
            if reachable.contains(&block) && assigned.insert(block) {
                component.push(block);
                stack.extend(predecessors.get(&block).into_iter().flatten());
            }
        }
        if component.len() > 1 || successors[&block].contains(&block) {
            cyclic.extend(component);
        }
    }
    cyclic
}
