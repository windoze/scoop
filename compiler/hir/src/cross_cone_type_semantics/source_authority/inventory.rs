use std::fmt;

use scoop_wire::WireError;

mod callables;
mod inheritance;
mod roots;
mod slot_selections;
pub use callables::*;
pub use inheritance::*;
pub use roots::*;
pub use slot_selections::*;

fn validate_order<T, K: Ord>(
    records: &[T],
    key: impl Fn(&T) -> K,
    table: &'static str,
) -> Result<(), SourceInventoryError> {
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

#[derive(Debug)]
pub enum SourceInventoryError {
    Resource(WireError),
    Reference(String),
    NonCanonicalOrder { table: &'static str, index: usize },
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
        }
    }
}

impl std::error::Error for SourceInventoryError {}
