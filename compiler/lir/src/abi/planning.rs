//! Complete layout and logical signature inputs for target ABI planning.

use la_arena::{Arena, Idx};

use crate::{
    AbiCarrier, AbiValue, AbiZst, CType, DirectCSignature, LirTargetProfile, ScoopAbiSignature,
};

/// Scalar leaves are meaningful only for reference-free aggregates. A scanned
/// aggregate remains complete storage input without inventing scalar leaves.
#[derive(Clone, Debug)]
pub enum AbiPlanningValue {
    ZeroSized(AbiZst),
    Scalar {
        value: AbiValue,
        carrier: AbiCarrier,
    },
    Interface(AbiValue),
    Aggregate {
        value: AbiValue,
        fields: Vec<AbiAggregateField>,
    },
    ScannedAggregate(AbiValue),
    /// Shared tagged payload storage, whose scalar contents are integer bytes.
    IntegerStorage(AbiValue),
}

pub type AbiPlanningValueId = Idx<AbiPlanningValue>;

#[derive(Clone, Copy, Debug)]
pub struct AbiAggregateField {
    pub offset: u64,
    pub value: AbiPlanningValueId,
}

#[derive(Clone, Copy, Debug)]
pub enum LogicalAbiResult {
    Void,
    Value(AbiPlanningValueId),
}

#[derive(Clone, Debug)]
pub struct ScoopAbiRequest {
    pub arguments: Vec<AbiPlanningValueId>,
    pub result: LogicalAbiResult,
}

pub type ScoopAbiRequestId = Idx<ScoopAbiRequest>;

#[derive(Clone, Debug)]
pub struct CAbiValueInput {
    pub value: AbiPlanningValueId,
    pub ty: CType,
}

#[derive(Clone, Debug)]
pub enum LogicalCAbiResult {
    Void,
    Value(CAbiValueInput),
}

#[derive(Clone, Debug)]
pub struct CAbiRequest {
    pub arguments: Vec<CAbiValueInput>,
    pub result: LogicalCAbiResult,
}

pub type CAbiRequestId = Idx<CAbiRequest>;

#[derive(Clone, Debug)]
pub struct AbiPlanningInput {
    pub target: LirTargetProfile,
    pub values: Arena<AbiPlanningValue>,
    pub scoop: Arena<ScoopAbiRequest>,
    pub c: Arena<CAbiRequest>,
}

/// Each arena retains the corresponding request arena's order. These are
/// fully classified signatures, never defaults or slots to fill during LIR.
pub struct AbiPlanSet {
    pub scoop: Arena<ScoopAbiSignature>,
    pub c: Arena<DirectCSignature>,
}

impl AbiPlanSet {
    pub fn scoop(&self, request: ScoopAbiRequestId) -> &ScoopAbiSignature {
        &self.scoop[Idx::from_raw(request.into_raw())]
    }

    pub fn c(&self, request: CAbiRequestId) -> &DirectCSignature {
        &self.c[Idx::from_raw(request.into_raw())]
    }
}
