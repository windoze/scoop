use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use super::DecodedSignatureTypeKey;

#[cfg(test)]
mod tests;

impl DecodedSignatureTypeKey {
    /// Accounts for the resolved tree before an aggregate resolver allocates
    /// its owned signature nodes. The walk itself uses the same shared meter.
    pub fn charge_resolution(&self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        self.charge_resolution_from_depth(meter, 1)
    }

    /// Continues an aggregate's typed resource walk at its current semantic depth.
    pub fn charge_resolution_from_depth(
        &self,
        meter: &mut BudgetMeter,
        depth: u64,
    ) -> Result<(), WireError> {
        let path = WirePath::root();
        let mut pending = Vec::new();
        meter.try_reserve_collection_slots(&mut pending, 1, &path)?;
        pending.push((self, depth));
        while let Some((node, depth)) = pending.pop() {
            meter.check_semantic_depth(depth, &path)?;
            meter.charge_nodes(1, &path)?;
            meter.charge_work(1, &path)?;
            match node {
                Self::Nominal(_) | Self::Binder { .. } => {}
                Self::NominalApplication { arguments, .. } | Self::Tuple(arguments) => {
                    push(
                        &mut pending,
                        arguments.as_slice(),
                        child_depth(depth)?,
                        meter,
                    )?;
                }
                Self::Function {
                    parameters, result, ..
                }
                | Self::NativeFunctionPointer {
                    parameters, result, ..
                } => {
                    push(&mut pending, parameters, child_depth(depth)?, meter)?;
                    push(
                        &mut pending,
                        std::slice::from_ref(result.as_ref()),
                        child_depth(depth)?,
                        meter,
                    )?;
                }
                Self::RawPointer(pointee) => push(
                    &mut pending,
                    std::slice::from_ref(pointee.as_ref()),
                    child_depth(depth)?,
                    meter,
                )?,
            }
        }
        Ok(())
    }
}

fn child_depth(depth: u64) -> Result<u64, WireError> {
    depth
        .checked_add(1)
        .ok_or_else(|| WireError::new(WireErrorKind::IntegerOutOfRange, WirePath::root(), None))
}

fn push<'a>(
    pending: &mut Vec<(&'a DecodedSignatureTypeKey, u64)>,
    nodes: &'a [DecodedSignatureTypeKey],
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    if nodes.is_empty() {
        return Ok(());
    }
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path)?;
    meter.charge_work(nodes.len() as u64, &path)?;
    meter.charge_collection_slots(nodes.len() as u64, &path)?;
    meter.charge_edges(nodes.len() as u64, &path)?;
    meter.try_reserve_collection_slots(pending, nodes.len(), &path)?;
    pending.extend(nodes.iter().map(|node| (node, depth)));
    Ok(())
}
