use std::fmt;

use scoop_wire::{Encoder, WireEncode};

use crate::cross_cone_type_semantics::wire;

mod decode;
#[cfg(test)]
mod tests;

pub use decode::*;

/// Transport data only. The body visitor proves the actual receiver relation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProtectedDefaultReceiverUseV1 {
    None,
    ImplicitThis,
    Explicit { receiver_expression_index: u32 },
    ConstructorDelegation,
}

impl WireEncode for ProtectedDefaultReceiverUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => wire::tag(encoder, 1, 1),
            Self::ImplicitThis => wire::tag(encoder, 1, 2),
            Self::Explicit {
                receiver_expression_index,
            } => {
                wire::tag(encoder, 2, 3)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*receiver_expression_index))
            }
            Self::ConstructorDelegation => wire::tag(encoder, 1, 4),
        }
    }
}

/// Expression indices are body traversal positions, never persistent identities.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedDefaultExpressionUseV1 {
    expression_index: u32,
    receiver_use: ProtectedDefaultReceiverUseV1,
}

impl ProtectedDefaultExpressionUseV1 {
    pub const fn new(expression_index: u32, receiver_use: ProtectedDefaultReceiverUseV1) -> Self {
        Self {
            expression_index,
            receiver_use,
        }
    }
    pub const fn expression_index(self) -> u32 {
        self.expression_index
    }
    pub const fn receiver_use(self) -> ProtectedDefaultReceiverUseV1 {
        self.receiver_use
    }
}

impl WireEncode for ProtectedDefaultExpressionUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.expression_index))?;
        encoder.field(2)?;
        self.receiver_use.encode(encoder)
    }
}

/// An ordered occurrence list. Empty data does not prove an empty body closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalProtectedDefaultExpressionUsesV1 {
    values: Vec<ProtectedDefaultExpressionUseV1>,
}

impl CanonicalProtectedDefaultExpressionUsesV1 {
    pub fn try_new(
        mut values: Vec<ProtectedDefaultExpressionUseV1>,
    ) -> Result<Self, ProtectedDefaultExpressionUsesBuildError> {
        values.sort_unstable();
        Self::from_ordered(values)
    }
    pub fn values(&self) -> &[ProtectedDefaultExpressionUseV1] {
        &self.values
    }
    fn from_ordered(
        values: Vec<ProtectedDefaultExpressionUseV1>,
    ) -> Result<Self, ProtectedDefaultExpressionUsesBuildError> {
        u32::try_from(values.len())
            .map_err(|_| ProtectedDefaultExpressionUsesBuildError::TooMany)?;
        for (index, pair) in values.windows(2).enumerate() {
            match pair[0].cmp(&pair[1]) {
                std::cmp::Ordering::Equal => {
                    return Err(ProtectedDefaultExpressionUsesBuildError::Duplicate {
                        index: index + 1,
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(
                        ProtectedDefaultExpressionUsesBuildError::NonCanonicalOrder {
                            index: index + 1,
                        },
                    );
                }
                std::cmp::Ordering::Less => {}
            }
        }
        Ok(Self { values })
    }
}

impl WireEncode for CanonicalProtectedDefaultExpressionUsesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.values)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedDefaultExpressionUsesBuildError {
    TooMany,
    Duplicate { index: usize },
    NonCanonicalOrder { index: usize },
}

impl fmt::Display for ProtectedDefaultExpressionUsesBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => f.write_str("too many protected default expression uses"),
            Self::Duplicate { index } => write!(
                f,
                "duplicate protected default expression use at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                f,
                "noncanonical protected default expression use order at index {index}"
            ),
        }
    }
}

impl std::error::Error for ProtectedDefaultExpressionUsesBuildError {}
