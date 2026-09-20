use std::fmt;

use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::super::wire;

mod callables;
mod constructors;
mod dependencies;
mod edges;
mod inheritance;
mod interface_dispatch;
mod nominals;
mod roots;
mod slot_selections;
pub use callables::*;
pub use constructors::*;
pub use dependencies::*;
pub use edges::*;
pub use inheritance::*;
pub use interface_dispatch::*;
pub use nominals::*;
pub use roots::*;
pub use slot_selections::*;

fn charge_sort(count: usize, meter: &mut BudgetMeter) -> Result<(), SourceInventoryError> {
    let count = count as u64;
    let path = WirePath::root();
    meter.check_table_entries(count, &path)?;
    meter.charge_work(
        count.saturating_mul(u64::from(count.max(1).ilog2()) + 1),
        &path,
    )?;
    Ok(())
}

fn validate_order<T, K: Ord>(
    records: &[T],
    key: impl Fn(&T) -> K,
    table: &'static str,
    meter: &mut BudgetMeter,
) -> Result<(), SourceInventoryError> {
    let path = WirePath::root();
    meter.check_table_entries(records.len() as u64, &path)?;
    meter.charge_nodes(records.len() as u64, &path)?;
    meter.charge_work(records.len() as u64, &path)?;
    if let Some(index) = records
        .windows(2)
        .position(|pair| key(&pair[0]) >= key(&pair[1]))
    {
        return Err(SourceInventoryError::NonCanonicalOrder {
            table,
            index: index + 1,
        });
    }
    Ok(())
}

fn reserve<T>(count: usize, meter: &mut BudgetMeter) -> Result<Vec<T>, SourceInventoryError> {
    let path = WirePath::root();
    meter.check_semantic_depth(1, &path)?;
    meter.check_table_entries(count as u64, &path)?;
    meter.charge_nodes(count as u64, &path)?;
    meter.charge_work(count as u64, &path)?;
    let mut output = Vec::new();
    meter.try_reserve_collection_slots(&mut output, count, &path)?;
    Ok(output)
}

fn reference(error: impl fmt::Display) -> SourceInventoryError {
    SourceInventoryError::Reference(error.to_string())
}

#[derive(Debug)]
pub enum SourceInventoryError {
    Resource(WireError),
    Reference(String),
    NonCanonicalOrder {
        table: &'static str,
        index: usize,
    },
    InvalidInterfaceDispatch {
        owner: scoop_identity::PersistentExactTypeId,
        reason: &'static str,
    },
    ConstructorInMembers {
        owner: scoop_identity::PersistentExactTypeId,
        constructor: scoop_identity::PersistentConstructorId,
    },
}

impl From<WireError> for SourceInventoryError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for SourceInventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Reference(error) => write!(f, "invalid source inventory reference: {error}"),
            Self::NonCanonicalOrder { table, index } => {
                write!(
                    f,
                    "duplicate or noncanonical {table} source inventory at index {index}"
                )
            }
            Self::InvalidInterfaceDispatch { owner, reason } => {
                write!(f, "invalid interface dispatch source {owner}: {reason}")
            }
            Self::ConstructorInMembers { owner, constructor } => write!(
                f,
                "source inheritance owner {owner} lists constructor {constructor} as a member"
            ),
        }
    }
}

impl std::error::Error for SourceInventoryError {}

#[cfg(test)]
mod tests;
