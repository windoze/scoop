//! Closed semantic contracts borrowed from complete provider records.
use scoop_identity::{CanonicalScoopAbiFunctionSignature, PersistentLayoutId, ScanRole};

use crate::{
    CallingConvention, ExactCallableProtocolV1, ExactDescriptorExportV1, ExactDispatchExportV1,
    ExactLayoutExportV1, ExternalStrongShapeSubjectV1, RefScan,
    StrongInitializationUnitSemanticPlanV2, StrongStaticStorageSemanticPlanV1,
};

mod decoded;
mod validate;
mod wire;
pub use decoded::DecodedShapeLinkContractV1;

/// Semantic data alone does not grant a physical import or selected handle.
#[derive(Clone, Copy, Debug)]
pub enum ShapeLinkContractV1<'a> {
    CallableAbi {
        canonical_signature: &'a CanonicalScoopAbiFunctionSignature,
        calling_convention: CallingConvention,
        protocol: ExactCallableProtocolV1,
    },
    Layout {
        record: &'a ExactLayoutExportV1,
    },
    Scan {
        layout: PersistentLayoutId,
        role: ScanRole,
        canonical_scan: &'a RefScan,
    },
    Type {
        descriptor_projection: &'a ExactDescriptorExportV1,
    },
    Dispatch {
        table_projection: &'a ExactDispatchExportV1,
    },
    StaticStorage {
        storage_projection: &'a StrongStaticStorageSemanticPlanV1,
    },
    Initialization {
        unit_projection: &'a StrongInitializationUnitSemanticPlanV2,
    },
}

impl ShapeLinkContractV1<'_> {
    pub const fn tag(&self) -> u32 {
        match self {
            Self::CallableAbi { .. } => 1,
            Self::Layout { .. } => 2,
            Self::Scan { .. } => 3,
            Self::Type { .. } => 4,
            Self::Dispatch { .. } => 5,
            Self::StaticStorage { .. } => 6,
            Self::Initialization { .. } => 7,
        }
    }

    pub const fn matches_subject(&self, subject: ExternalStrongShapeSubjectV1) -> bool {
        self.tag() == contract_tag(subject)
    }
}

pub(super) const fn contract_tag(subject: ExternalStrongShapeSubjectV1) -> u32 {
    use ExternalStrongShapeSubjectV1 as Subject;
    match subject {
        Subject::Callable(_) => 1,
        Subject::Layout(_) => 2,
        Subject::Scan(_) => 3,
        Subject::TypeDescriptor(_) | Subject::TypeRegistration(_) => 4,
        Subject::DispatchTable(_) => 5,
        Subject::StaticStorage(_) | Subject::StaticStorageRegistration(_) => 6,
        Subject::InitializationCell(_) | Subject::InitializationDescriptor(_) => 7,
    }
}
