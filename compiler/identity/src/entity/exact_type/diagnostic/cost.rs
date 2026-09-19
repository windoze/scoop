use super::*;

pub(super) fn exact_type_cost(
    id: PersistentExactTypeId,
    graph: &impl ExactTypeDiagnosticGraph,
    costs: &mut BTreeMap<PersistentExactTypeId, usize>,
    active: &mut BTreeSet<PersistentExactTypeId>,
    depth: usize,
    meter: &mut BudgetMeter,
) -> Result<usize, ExactTypeDiagnosticError> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth as u64, &path)?;
    meter.charge_work(1 + u64::from(costs.len().saturating_add(2).ilog2()), &path)?;
    if depth > 1 {
        meter.charge_edges(1, &path)?;
    }
    if depth > MAX_DIAGNOSTIC_RECURSION {
        return Err(ExactTypeDiagnosticError::RecursionLimit);
    }
    if let Some(cost) = costs.get(&id) {
        return Ok(*cost);
    }
    meter.charge_nodes(1, &path)?;
    meter.charge_collection_slots(2, &path)?;
    if !active.insert(id) {
        return Err(ExactTypeDiagnosticError::Cycle(id));
    }
    let key = graph
        .exact_type_key(id)
        .ok_or(ExactTypeDiagnosticError::MissingExactType(id))?;
    let cost = match key {
        ExactTypeKey::Nominal(declaration) => nominal_cost(*declaration, graph, meter)?,
        ExactTypeKey::NominalApplication { origin, arguments } => {
            let declaration = graph
                .source_generic_type_declaration(*origin)
                .ok_or(ExactTypeDiagnosticError::MissingSourceGenericType(*origin))?;
            let mut cost = checked_add(4, nominal_atom_cost(declaration, graph, meter)?)?;
            cost = checked_add(
                cost,
                sequence_cost(arguments.as_slice(), graph, costs, active, depth, meter)?,
            )?;
            checked_add(cost, 2)?
        }
        ExactTypeKey::Tuple(elements) => checked_add(
            5,
            sequence_cost(elements.as_slice(), graph, costs, active, depth, meter)?,
        )?,
        ExactTypeKey::Function {
            parameters, result, ..
        }
        | ExactTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            let mut cost = checked_add(
                5,
                sequence_cost(parameters, graph, costs, active, depth, meter)?,
            )?;
            cost = checked_add(
                cost,
                exact_type_cost(*result, graph, costs, active, depth + 1, meter)?,
            )?;
            checked_add(cost, 4)?
        }
        ExactTypeKey::RawPointer(pointee) => checked_add(
            3,
            exact_type_cost(*pointee, graph, costs, active, depth + 1, meter)?,
        )?,
    };
    active.remove(&id);
    costs.insert(id, cost);
    Ok(cost)
}

fn sequence_cost(
    ids: &[PersistentExactTypeId],
    graph: &impl ExactTypeDiagnosticGraph,
    costs: &mut BTreeMap<PersistentExactTypeId, usize>,
    active: &mut BTreeSet<PersistentExactTypeId>,
    depth: usize,
    meter: &mut BudgetMeter,
) -> Result<usize, ExactTypeDiagnosticError> {
    let mut cost = 0usize;
    for (index, id) in ids.iter().enumerate() {
        if index > 0 {
            cost = checked_add(cost, 1)?;
        }
        cost = checked_add(
            cost,
            exact_type_cost(*id, graph, costs, active, depth + 1, meter)?,
        )?;
    }
    Ok(cost)
}
