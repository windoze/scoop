use scoop_identity::{NominalDeclarationOwner, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

/// Iteratively visits every nominal leaf in one signature tree while charging
/// the artifact-wide semantic resource meter.
pub(crate) struct SignatureNominalWalker<'signature> {
    pending: Vec<(&'signature SignatureTypeKey, u64)>,
}

impl<'signature> SignatureNominalWalker<'signature> {
    pub(crate) fn new(
        signature: &'signature SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, WireError> {
        let mut pending = Vec::new();
        meter.try_reserve_collection_slots(&mut pending, 1, path)?;
        pending.push((signature, 1));
        Ok(Self { pending })
    }

    pub(crate) fn next(
        &mut self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<NominalDeclarationOwner>, WireError> {
        while let Some((signature, depth)) = self.pending.pop() {
            meter.check_semantic_depth(depth, path)?;
            meter.charge_nodes(1, path)?;
            meter.charge_work(1, path)?;

            match signature {
                SignatureTypeKey::Nominal(declaration) => {
                    return Ok(Some(NominalDeclarationOwner::Concrete(*declaration)));
                }
                SignatureTypeKey::NominalApplication { origin, arguments } => {
                    push_children(&mut self.pending, arguments.as_slice(), depth, meter, path)?;
                    return Ok(Some(NominalDeclarationOwner::GenericTemplate(*origin)));
                }
                SignatureTypeKey::Tuple(elements) => {
                    push_children(&mut self.pending, elements.as_slice(), depth, meter, path)?
                }
                SignatureTypeKey::Function {
                    parameters, result, ..
                }
                | SignatureTypeKey::NativeFunctionPointer {
                    parameters, result, ..
                } => {
                    let child_count = parameters
                        .len()
                        .checked_add(1)
                        .ok_or_else(|| integer_out_of_range(path))?;
                    let child_depth =
                        reserve_children(&mut self.pending, child_count, depth, meter, path)?;
                    self.pending.push((result.as_ref(), child_depth));
                    for parameter in parameters.iter().rev() {
                        self.pending.push((parameter, child_depth));
                    }
                }
                SignatureTypeKey::RawPointer(pointee) => {
                    let child_depth = reserve_children(&mut self.pending, 1, depth, meter, path)?;
                    self.pending.push((pointee.as_ref(), child_depth));
                }
                SignatureTypeKey::Binder { .. } => {}
            }
        }
        Ok(None)
    }
}

fn push_children<'signature>(
    pending: &mut Vec<(&'signature SignatureTypeKey, u64)>,
    children: &'signature [SignatureTypeKey],
    parent_depth: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    if children.is_empty() {
        return Ok(());
    }
    let child_depth = reserve_children(pending, children.len(), parent_depth, meter, path)?;
    for child in children.iter().rev() {
        pending.push((child, child_depth));
    }
    Ok(())
}

fn reserve_children(
    pending: &mut Vec<(&SignatureTypeKey, u64)>,
    count: usize,
    parent_depth: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<u64, WireError> {
    let child_depth = parent_depth
        .checked_add(1)
        .ok_or_else(|| integer_out_of_range(path))?;
    meter.check_semantic_depth(child_depth, path)?;
    let edge_count = u64::try_from(count).map_err(|_| integer_out_of_range(path))?;
    meter.charge_edges(edge_count, path)?;
    meter.charge_work(edge_count, path)?;
    meter.try_reserve_collection_slots(pending, count, path)?;
    Ok(child_depth)
}

fn integer_out_of_range(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
