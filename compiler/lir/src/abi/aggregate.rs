//! Complete laid-out leaves used as input to backend ABI planning.

use crate::AbiCarrier;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbiScalarLeaf {
    pub offset: u64,
    pub carrier: AbiCarrier,
    pub alignment: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbiAggregateLayout {
    pub size: u64,
    pub alignment: u64,
    pub leaves: Vec<AbiScalarLeaf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbiValuePosition {
    Argument,
    Result,
}
