use scoop_identity::{CLayoutByteAlignment, CLayoutOverride};

use super::NativeBoundaryCLayoutPolicy;
use crate::{HirCLayoutValue, NominalCLayoutPolicyV1};

impl From<NominalCLayoutPolicyV1> for NativeBoundaryCLayoutPolicy {
    fn from(policy: NominalCLayoutPolicyV1) -> Self {
        match policy {
            NominalCLayoutPolicyV1::Ordinary => Self::NotCLayout,
            NominalCLayoutPolicyV1::CLayout { contract } => Self::CLayout {
                aligned: alignment(contract.aligned),
                packed: alignment(contract.packed),
            },
        }
    }
}

fn alignment(value: HirCLayoutValue) -> CLayoutOverride {
    match value {
        HirCLayoutValue::Natural => CLayoutOverride::Natural,
        HirCLayoutValue::A1 => CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes1),
        HirCLayoutValue::A2 => CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes2),
        HirCLayoutValue::A4 => CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes4),
        HirCLayoutValue::A8 => CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes8),
        HirCLayoutValue::A16 => CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes16),
    }
}
