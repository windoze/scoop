use std::fmt;

use scoop_wire::{WireError, WirePath};

use super::super::wire;

mod callables;
mod constructors;
mod default_access_declarations;
mod default_profiles;
mod dependencies;
mod edges;
mod inheritance;
mod interface_dispatch;
mod nominal_callables;
mod nominal_constructors;
mod nominal_contracts;
mod nominal_parameters;
mod nominal_properties;
mod nominals;
mod parameter_protocols;
mod properties;
mod protected_callables;
mod roots;
mod slot_selections;
pub use callables::*;
pub use constructors::*;
pub use default_access_declarations::*;
pub use default_profiles::*;
pub use dependencies::*;
pub use edges::*;
pub use inheritance::*;
pub use interface_dispatch::*;
pub use nominal_callables::*;
pub use nominal_constructors::*;
pub use nominal_contracts::*;
pub use nominal_parameters::*;
pub use nominal_properties::*;
pub use nominals::*;
pub use parameter_protocols::*;
pub use properties::*;
pub use protected_callables::*;
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

fn reserve<T>(count: usize) -> Result<Vec<T>, SourceInventoryError> {
    let path = WirePath::root();

    let mut output = Vec::new();
    scoop_wire::allocation::try_reserve(&mut output, count, &path)?;
    Ok(output)
}

fn reference(error: impl fmt::Display) -> SourceInventoryError {
    SourceInventoryError::Reference(error.to_string())
}

#[derive(Debug)]
pub enum SourceInventoryError {
    MissingDefaultProfile(crate::ProtectedDefaultTemplateKeyV1),
    UnexpectedDefaultProfile(crate::ProtectedDefaultTemplateKeyV1),
    InvalidDefaultAccessSubject(scoop_identity::DefinitionOriginSubject),
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
    NonRuntimeProperty(scoop_identity::PersistentPropertyId),
}

impl From<WireError> for SourceInventoryError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for SourceInventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDefaultProfile(key) => {
                write!(f, "missing source default profile: {key:?}")
            }
            Self::UnexpectedDefaultProfile(key) => {
                write!(f, "unexpected source default profile: {key:?}")
            }
            Self::InvalidDefaultAccessSubject(subject) => {
                write!(f, "invalid default access declaration subject: {subject:?}")
            }
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
            Self::NonRuntimeProperty(property) => {
                write!(f, "inheritance property source {property} cannot be const")
            }
        }
    }
}

impl std::error::Error for SourceInventoryError {}

#[cfg(test)]
mod tests;
