use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::DecodedSignatureTypeKey;

#[cfg(test)]
mod tests;

impl DecodedSignatureTypeKey {
    /// Accounts for the resolved tree before an aggregate resolver allocates
    /// its owned signature nodes. The walk itself uses the same shared meter.
    pub fn charge_resolution(&self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        let path = WirePath::root();
        let mut pending = Vec::new();
        meter.try_reserve_collection_slots(&mut pending, 1, &path)?;
        pending.push((self, 1_u64));
        while let Some((node, depth)) = pending.pop() {
            meter.check_semantic_depth(depth, &path)?;
            meter.charge_nodes(1, &path)?;
            meter.charge_work(1, &path)?;
            match node {
                Self::Nominal(_) | Self::Binder { .. } => {}
                Self::NominalApplication { arguments, .. } | Self::Tuple(arguments) => {
                    push(&mut pending, arguments.as_slice(), depth + 1, meter)?;
                }
                Self::Function {
                    parameters, result, ..
                }
                | Self::NativeFunctionPointer {
                    parameters, result, ..
                } => {
                    push(&mut pending, parameters, depth + 1, meter)?;
                    push(
                        &mut pending,
                        std::slice::from_ref(result.as_ref()),
                        depth + 1,
                        meter,
                    )?;
                }
                Self::RawPointer(pointee) => push(
                    &mut pending,
                    std::slice::from_ref(pointee.as_ref()),
                    depth + 1,
                    meter,
                )?,
            }
        }
        Ok(())
    }
}

fn push<'a>(
    pending: &mut Vec<(&'a DecodedSignatureTypeKey, u64)>,
    nodes: &'a [DecodedSignatureTypeKey],
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.charge_collection_slots(nodes.len() as u64, &path)?;
    meter.charge_edges(nodes.len() as u64, &path)?;
    meter.try_reserve_collection_slots(pending, nodes.len(), &path)?;
    pending.extend(nodes.iter().map(|node| (node, depth)));
    Ok(())
}
