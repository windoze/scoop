use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Knowledge {
    /// Test results whose operand identity has not been redefined.
    tests: HashSet<scoop_lir::TempId>,
    /// Variants established by a matching edge or an unchanged construction.
    facts: HashSet<VariantFact>,
}

impl Knowledge {
    fn intersect_with(&mut self, other: &Self) {
        self.tests.retain(|test| other.tests.contains(test));
        self.facts.retain(|fact| other.facts.contains(fact));
    }

    fn kill_value(
        &mut self,
        value: scoop_lir::Value,
        tests: &HashMap<scoop_lir::TempId, VariantFact>,
    ) {
        self.tests
            .retain(|test| tests.get(test).is_none_or(|fact| fact.operand != value));
        self.facts.retain(|fact| fact.operand != value);
    }
}

#[derive(Debug, Clone, Copy)]
struct FlowEdge {
    from: scoop_lir::BlockId,
    to: scoop_lir::BlockId,
    /// Present only on a `CondBr` then edge.
    true_condition: Option<scoop_lir::TempId>,
}

pub(super) fn validate(
    function: &Function,
    tests: &HashMap<scoop_lir::TempId, VariantFact>,
) -> Result<(), CodegenError> {
    if function.blocks.is_empty() {
        return Ok(());
    }
    let count = function.blocks.len();
    let index = |id: scoop_lir::BlockId| id.into_raw().into_u32() as usize;
    let checked_index = |id: scoop_lir::BlockId| -> Result<usize, CodegenError> {
        let value = index(id);
        if value >= count {
            return Err(CodegenError(format!(
                "variant control-flow validation in @{} reached invalid block {}",
                function.symbol(),
                id.into_raw()
            )));
        }
        Ok(value)
    };

    let mut outgoing = vec![Vec::new(); count];
    let mut predecessors = vec![Vec::new(); count];
    for (id, block) in function.blocks.iter() {
        let mut add = |to: scoop_lir::BlockId,
                       true_condition: Option<scoop_lir::TempId>|
         -> Result<(), CodegenError> {
            checked_index(to)?;
            let edge = FlowEdge {
                from: id,
                to,
                true_condition,
            };
            outgoing[index(id)].push(edge);
            predecessors[index(to)].push(edge);
            Ok(())
        };
        match block.terminator {
            Terminator::Br(target) => add(target, None)?,
            Terminator::CondBr {
                cond,
                then_block,
                else_block,
            } => {
                let condition = match cond {
                    Value::Temp(temp) => Some(temp),
                    _ => None,
                };
                add(then_block, condition)?;
                add(else_block, None)?;
            }
            Terminator::Return { .. } | Terminator::Resume { .. } | Terminator::Unreachable => {}
        }
        if let Some(Instruction::Invoke { site }) = block.instructions.last() {
            add(site.unwind(), None)?;
        }
    }

    let entry = checked_index(function.entry)?;
    let mut reachable = vec![false; count];
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        let block_index = index(block);
        if std::mem::replace(&mut reachable[block_index], true) {
            continue;
        }
        pending.extend(outgoing[block_index].iter().map(|edge| edge.to));
    }

    // This is a forward must-analysis. Starting reachable non-entry blocks at
    // the universe and intersecting predecessor edge states computes exactly
    // the facts established on every path, i.e. true-edge dominance.
    let universe = universe(function, tests);
    let escaped = escaped_locals(function);
    let mut incoming = vec![universe; count];
    for (block, state) in incoming.iter_mut().enumerate() {
        if block == entry || !reachable[block] {
            *state = Knowledge::default();
        }
    }
    loop {
        let mut next = incoming.clone();
        let mut changed = false;
        for block in 0..count {
            if block == entry || !reachable[block] {
                next[block] = Knowledge::default();
                continue;
            }
            let mut edges = predecessors[block]
                .iter()
                .copied()
                .filter(|edge| reachable[index(edge.from)]);
            let Some(first) = edges.next() else {
                next[block] = Knowledge::default();
                continue;
            };
            let mut state = edge_knowledge(
                function,
                &incoming[index(first.from)],
                first,
                tests,
                &escaped,
            );
            for edge in edges {
                let other =
                    edge_knowledge(function, &incoming[index(edge.from)], edge, tests, &escaped);
                state.intersect_with(&other);
            }
            if state != incoming[block] {
                next[block] = state;
                changed = true;
            }
        }
        incoming = next;
        if !changed {
            break;
        }
    }

    for (id, block) in function.blocks.iter() {
        let mut knowledge = incoming[index(id)].clone();
        for instruction in &block.instructions {
            if let Instruction::VariantPayloadProject { operand, field, .. } = instruction {
                let required = VariantFact {
                    operand: *operand,
                    variant: field.variant(),
                };
                if !knowledge.facts.contains(&required) {
                    return Err(CodegenError(format!(
                        "variant_payload_project @{} for enum{} variant {} is not dominated by a matching VariantTest true edge or known construction for the same operand",
                        function.symbol(),
                        field.definition().into_raw(),
                        field.variant().index()
                    )));
                }
            }
            transfer(&mut knowledge, instruction, tests, &escaped);
        }
    }
    Ok(())
}

fn edge_knowledge(
    function: &Function,
    incoming: &Knowledge,
    edge: FlowEdge,
    tests: &HashMap<scoop_lir::TempId, VariantFact>,
    escaped: &HashSet<scoop_lir::LocalId>,
) -> Knowledge {
    let mut knowledge = incoming.clone();
    for instruction in &function.blocks[edge.from].instructions {
        transfer(&mut knowledge, instruction, tests, escaped);
    }
    if let Some(condition) = edge.true_condition
        && knowledge.tests.contains(&condition)
        && let Some(fact) = tests.get(&condition)
    {
        knowledge.facts.insert(*fact);
    }
    knowledge
}

fn transfer(
    knowledge: &mut Knowledge,
    instruction: &Instruction,
    tests: &HashMap<scoop_lir::TempId, VariantFact>,
    escaped: &HashSet<scoop_lir::LocalId>,
) {
    let copy = match instruction {
        Instruction::Store { local, value } => Some((
            *local,
            knowledge
                .facts
                .iter()
                .filter(|fact| fact.operand == *value)
                .map(|fact| fact.variant)
                .collect::<Vec<_>>(),
        )),
        _ => None,
    };
    for definition in crate::dataflow::instruction_defs(instruction) {
        let value = match definition {
            crate::dataflow::LiveValue::Temp(id) => {
                knowledge.tests.remove(&id);
                Value::Temp(id)
            }
            crate::dataflow::LiveValue::Local(id) => Value::Local(id),
            crate::dataflow::LiveValue::Param(index) => Value::Param(index),
        };
        knowledge.kill_value(value, tests);
    }
    if instruction.safepoint().is_some()
        || matches!(
            instruction,
            Instruction::RawStore { .. } | Instruction::Call { .. } | Instruction::Invoke { .. }
        )
    {
        for local in escaped {
            knowledge.kill_value(Value::Local(*local), tests);
        }
    }
    if let Some((local, variants)) = copy {
        knowledge
            .facts
            .extend(variants.into_iter().map(|variant| VariantFact {
                operand: Value::Local(local),
                variant,
            }));
    }
    match instruction {
        Instruction::EnumWrap { out, variant, .. } => {
            knowledge.facts.insert(VariantFact {
                operand: Value::Temp(*out),
                variant: *variant,
            });
        }
        Instruction::VariantTest { out, .. } => {
            knowledge.tests.insert(*out);
        }
        _ => {}
    }
}

fn universe(function: &Function, tests: &HashMap<scoop_lir::TempId, VariantFact>) -> Knowledge {
    let mut facts = tests.values().copied().collect::<HashSet<_>>();
    let mut copies = HashMap::<Value, Vec<Value>>::new();
    for instruction in function
        .blocks
        .values()
        .flat_map(|block| &block.instructions)
    {
        match instruction {
            Instruction::EnumWrap { out, variant, .. } => {
                facts.insert(VariantFact {
                    operand: Value::Temp(*out),
                    variant: *variant,
                });
            }
            Instruction::Store { local, value } => {
                copies.entry(*value).or_default().push(Value::Local(*local));
            }
            _ => {}
        }
    }
    let mut pending = facts.iter().copied().collect::<Vec<_>>();
    while let Some(fact) = pending.pop() {
        for operand in copies.get(&fact.operand).into_iter().flatten() {
            let copied = VariantFact {
                operand: *operand,
                variant: fact.variant,
            };
            if facts.insert(copied) {
                pending.push(copied);
            }
        }
    }
    Knowledge {
        tests: tests.keys().copied().collect(),
        facts,
    }
}

fn escaped_locals(function: &Function) -> HashSet<scoop_lir::LocalId> {
    let mut escaped = HashSet::new();
    for instruction in function
        .blocks
        .values()
        .flat_map(|block| &block.instructions)
    {
        if let Instruction::LocalAddress { local, .. } = instruction {
            escaped.insert(*local);
        }
        for value in crate::dataflow::instruction_uses(instruction, function) {
            if let Value::CArgumentStorage(storage) = value {
                escaped.insert(storage.local());
            }
        }
    }
    escaped
}
