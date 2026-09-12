use std::fmt;

use scoop_identity::{
    CanonicalCAbiLayoutFingerprint, CanonicalCAbiResolutionError,
    CanonicalCAbiSignatureFingerprint, ConeIdentity, GeneratedBridgeAtomId, GeneratedBridgeUnitId,
    IdentityReferenceError, IdentityValidationError, NativeExternalContractFingerprint,
    NativeExternalContractResolutionError, ObjectDefinitionPlanId, PersistentCallableBodyId,
    PersistentCallbackApplicationId, PersistentCallbackRegistrationId, PersistentSafepointSiteId,
    PersistentSourceNativeExternalContractId, PersistentSymbolResolutionError, SafepointSiteRole,
};

use crate::{
    CallbackBridgeResolutionError, LirFoundationBuildError, RuntimeTypeMappingResolutionError,
    SafepointMappingResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafepointRelationError {
    MissingOwner {
        site: PersistentSafepointSiteId,
        owner: PersistentCallableBodyId,
    },
    DuplicateOrdinal {
        owner: PersistentCallableBodyId,
        role: SafepointSiteRole,
        ordinal: u32,
    },
    NonContiguousOrdinal {
        owner: PersistentCallableBodyId,
        role: SafepointSiteRole,
        expected: u32,
        actual: u32,
    },
    UnexpectedMapping {
        site: PersistentSafepointSiteId,
    },
    DuplicateMapping {
        site: PersistentSafepointSiteId,
    },
    MissingMapping {
        site: PersistentSafepointSiteId,
    },
}

impl fmt::Display for SafepointRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOwner { site, owner } => {
                write!(
                    formatter,
                    "LIR safepoint site {site} has absent owner body {owner}"
                )
            }
            Self::DuplicateOrdinal {
                owner,
                role,
                ordinal,
            } => write!(
                formatter,
                "LIR callable body {owner} repeats {role:?} safepoint ordinal {ordinal}"
            ),
            Self::NonContiguousOrdinal {
                owner,
                role,
                expected,
                actual,
            } => write!(
                formatter,
                "LIR callable body {owner} has non-contiguous {role:?} safepoint ordinals: expected {expected}, found {actual}"
            ),
            Self::UnexpectedMapping { site } => {
                write!(
                    formatter,
                    "safepoint mapping refers to absent LIR site {site}"
                )
            }
            Self::DuplicateMapping { site } => {
                write!(
                    formatter,
                    "LIR safepoint site {site} has multiple runtime mappings"
                )
            }
            Self::MissingMapping { site } => {
                write!(
                    formatter,
                    "LIR safepoint site {site} has no runtime mapping"
                )
            }
        }
    }
}

impl std::error::Error for SafepointRelationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeContractRelationError {
    UnexpectedRecord {
        source: PersistentSourceNativeExternalContractId,
    },
    DuplicateRecord {
        source: PersistentSourceNativeExternalContractId,
    },
    MissingRecord {
        source: PersistentSourceNativeExternalContractId,
    },
}

impl fmt::Display for NativeContractRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedRecord { source } => {
                write!(
                    formatter,
                    "LIR native contract refers to absent HIR contract {source}"
                )
            }
            Self::DuplicateRecord { source } => {
                write!(
                    formatter,
                    "HIR native contract {source} has multiple LIR contracts"
                )
            }
            Self::MissingRecord { source } => {
                write!(
                    formatter,
                    "HIR native contract {source} has no LIR contract"
                )
            }
        }
    }
}

impl std::error::Error for NativeContractRelationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeRelationError {
    MissingNativeContract {
        unit: GeneratedBridgeUnitId,
        contract: NativeExternalContractFingerprint,
    },
    MissingUnitSignature {
        unit: GeneratedBridgeUnitId,
        signature: CanonicalCAbiSignatureFingerprint,
    },
    DuplicateCallbackRecord {
        application: PersistentCallbackApplicationId,
    },
    UnexpectedCallbackRecord {
        application: PersistentCallbackApplicationId,
    },
    MissingCallbackRecord {
        application: PersistentCallbackApplicationId,
    },
    MissingCallbackRegistration {
        application: PersistentCallbackApplicationId,
        registration: PersistentCallbackRegistrationId,
    },
    MissingCallbackSignature {
        application: PersistentCallbackApplicationId,
        signature: CanonicalCAbiSignatureFingerprint,
    },
    MissingCallbackUnit {
        application: PersistentCallbackApplicationId,
        unit: GeneratedBridgeUnitId,
    },
    CallbackUnitMismatch {
        application: PersistentCallbackApplicationId,
        unit: GeneratedBridgeUnitId,
    },
    UnusedCallbackUnit {
        unit: GeneratedBridgeUnitId,
    },
    MissingAtomUnit {
        atom: GeneratedBridgeAtomId,
        unit: GeneratedBridgeUnitId,
    },
    MissingAtomSignature {
        atom: GeneratedBridgeAtomId,
        signature: CanonicalCAbiSignatureFingerprint,
    },
    AtomSignatureMismatch {
        atom: GeneratedBridgeAtomId,
        unit: GeneratedBridgeUnitId,
    },
    ContextForNonCallbackUnit {
        atom: GeneratedBridgeAtomId,
        unit: GeneratedBridgeUnitId,
    },
    AtomContextMismatch {
        atom: GeneratedBridgeAtomId,
        unit: GeneratedBridgeUnitId,
    },
    MissingAtomLayout {
        atom: GeneratedBridgeAtomId,
        layout: CanonicalCAbiLayoutFingerprint,
    },
}

impl fmt::Display for BridgeRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNativeContract { unit, contract } => write!(
                formatter,
                "generated bridge unit {unit} refers to absent native contract {contract}"
            ),
            Self::MissingUnitSignature { unit, signature } => write!(
                formatter,
                "generated bridge unit {unit} refers to absent C ABI signature {signature}"
            ),
            Self::DuplicateCallbackRecord { application } => write!(
                formatter,
                "callback application {application} has multiple LIR bridge records"
            ),
            Self::UnexpectedCallbackRecord { application } => write!(
                formatter,
                "LIR callback bridge refers to absent MIR application {application}"
            ),
            Self::MissingCallbackRecord { application } => {
                write!(
                    formatter,
                    "MIR callback application {application} has no LIR bridge"
                )
            }
            Self::MissingCallbackRegistration {
                application,
                registration,
            } => write!(
                formatter,
                "callback application {application} refers to absent HIR registration {registration}"
            ),
            Self::MissingCallbackSignature {
                application,
                signature,
            } => write!(
                formatter,
                "callback application {application} refers to absent C ABI signature {signature}"
            ),
            Self::MissingCallbackUnit { application, unit } => write!(
                formatter,
                "callback application {application} refers to absent bridge unit {unit}"
            ),
            Self::CallbackUnitMismatch { application, unit } => write!(
                formatter,
                "callback application {application} bridge unit {unit} does not match its signature and context parameter"
            ),
            Self::UnusedCallbackUnit { unit } => {
                write!(
                    formatter,
                    "callback bridge unit {unit} has no MIR application"
                )
            }
            Self::MissingAtomUnit { atom, unit } => {
                write!(
                    formatter,
                    "generated bridge atom {atom} refers to absent unit {unit}"
                )
            }
            Self::MissingAtomSignature { atom, signature } => write!(
                formatter,
                "generated bridge atom {atom} refers to absent C ABI signature {signature}"
            ),
            Self::AtomSignatureMismatch { atom, unit } => write!(
                formatter,
                "generated bridge atom {atom} records a signature different from callback unit {unit}"
            ),
            Self::ContextForNonCallbackUnit { atom, unit } => write!(
                formatter,
                "generated bridge atom {atom} records callback context for non-callback unit {unit}"
            ),
            Self::AtomContextMismatch { atom, unit } => write!(
                formatter,
                "generated bridge atom {atom} records a context parameter different from callback unit {unit}"
            ),
            Self::MissingAtomLayout { atom, layout } => write!(
                formatter,
                "generated bridge atom {atom} refers to absent C ABI layout {layout}"
            ),
        }
    }
}

impl std::error::Error for BridgeRelationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LirFoundationOwnershipError {
    ForeignBridgeAtom {
        atom: GeneratedBridgeAtomId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    ForeignStrongDefinitionPlan {
        plan: ObjectDefinitionPlanId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
}

impl fmt::Display for LirFoundationOwnershipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignBridgeAtom {
                atom,
                expected,
                actual,
            } => write!(
                formatter,
                "generated bridge atom {atom} belongs to Cone {actual}, not artifact Cone {expected}"
            ),
            Self::ForeignStrongDefinitionPlan {
                plan,
                expected,
                actual,
            } => write!(
                formatter,
                "strong definition plan {plan} belongs to Cone {actual}, not artifact Cone {expected}"
            ),
        }
    }
}

impl std::error::Error for LirFoundationOwnershipError {}

#[derive(Debug)]
pub enum LirFoundationValidationError {
    WireEncode(scoop_wire::cbor::EncodeError),
    Resource(scoop_wire::WireError),
    Identity(IdentityValidationError),
    RuntimeType {
        index: usize,
        error: RuntimeTypeMappingResolutionError<IdentityReferenceError>,
    },
    Safepoint {
        index: usize,
        error: SafepointMappingResolutionError<IdentityReferenceError>,
    },
    SymbolRequests(PersistentSymbolResolutionError<IdentityReferenceError>),
    NativeContract {
        index: usize,
        error: NativeExternalContractResolutionError<IdentityReferenceError>,
    },
    CAbiSignature {
        index: usize,
        error: CanonicalCAbiResolutionError<IdentityReferenceError>,
    },
    CAbiLayout {
        index: usize,
        error: CanonicalCAbiResolutionError<IdentityReferenceError>,
    },
    CallbackBridge {
        index: usize,
        error: CallbackBridgeResolutionError<IdentityReferenceError>,
    },
    SafepointRelation(SafepointRelationError),
    NativeContractRelation(NativeContractRelationError),
    BridgeRelation(BridgeRelationError),
    Ownership(LirFoundationOwnershipError),
    Build(LirFoundationBuildError),
    NonCanonicalFoundation,
}

impl From<SafepointRelationError> for LirFoundationValidationError {
    fn from(error: SafepointRelationError) -> Self {
        Self::SafepointRelation(error)
    }
}

impl From<NativeContractRelationError> for LirFoundationValidationError {
    fn from(error: NativeContractRelationError) -> Self {
        Self::NativeContractRelation(error)
    }
}

impl From<BridgeRelationError> for LirFoundationValidationError {
    fn from(error: BridgeRelationError) -> Self {
        Self::BridgeRelation(error)
    }
}

impl From<LirFoundationOwnershipError> for LirFoundationValidationError {
    fn from(error: LirFoundationOwnershipError) -> Self {
        Self::Ownership(error)
    }
}

impl fmt::Display for LirFoundationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WireEncode(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
            Self::Identity(error) => error.fmt(formatter),
            Self::RuntimeType { index, error } => {
                write!(
                    formatter,
                    "LIR runtime type mapping {index} is invalid: {error}"
                )
            }
            Self::Safepoint { index, error } => {
                write!(
                    formatter,
                    "LIR safepoint mapping {index} is invalid: {error}"
                )
            }
            Self::SymbolRequests(error) => {
                write!(
                    formatter,
                    "LIR persistent symbol request table is invalid: {error}"
                )
            }
            Self::NativeContract { index, error } => {
                write!(formatter, "LIR native contract {index} is invalid: {error}")
            }
            Self::CAbiSignature { index, error } => write!(
                formatter,
                "LIR canonical C ABI signature {index} is invalid: {error}"
            ),
            Self::CAbiLayout { index, error } => write!(
                formatter,
                "LIR canonical C ABI layout {index} is invalid: {error}"
            ),
            Self::CallbackBridge { index, error } => {
                write!(formatter, "LIR callback bridge {index} is invalid: {error}")
            }
            Self::SafepointRelation(error) => error.fmt(formatter),
            Self::NativeContractRelation(error) => error.fmt(formatter),
            Self::BridgeRelation(error) => error.fmt(formatter),
            Self::Ownership(error) => error.fmt(formatter),
            Self::Build(error) => error.fmt(formatter),
            Self::NonCanonicalFoundation => {
                formatter.write_str("LIR foundation tables are not in canonical structural order")
            }
        }
    }
}

impl std::error::Error for LirFoundationValidationError {}
