use super::*;

#[derive(Debug)]
pub enum NativeBoundaryTargetError {
    InvalidSignatureShape,
    MissingExactType {
        exact: PersistentExactTypeId,
    },
    MissingCallbackApplication {
        application: PersistentCallbackApplicationId,
    },
    MissingCallableApplication {
        application: PersistentCallableApplicationId,
    },
    MissingCallbackRegistration {
        registration: PersistentCallbackRegistrationId,
    },
    MissingInitializationUnit {
        unit: PersistentInitializationUnitId,
    },
    BinderDepthOutOfRange {
        depth: u32,
    },
    BinderIndexOutOfRange {
        depth: u32,
        index: u32,
    },
    ExpectedNominal {
        exact: PersistentExactTypeId,
    },
    NotCAbiSafe {
        exact: PersistentExactTypeId,
    },
    CLayoutCycle {
        exact: PersistentExactTypeId,
    },
    ScoopLayoutCycle {
        exact: PersistentExactTypeId,
    },
    CallableApplicationCycle {
        application: PersistentCallableApplicationId,
    },
    LayoutOverflow {
        exact: PersistentExactTypeId,
    },
    MissingComputedCLayout {
        layout: CanonicalCAbiLayoutFingerprint,
    },
    ArithmeticOverflow,
    NativeContractMismatch,
    CallbackSignatureMismatch {
        application: PersistentCallbackApplicationId,
    },
    ManagedCallbackSignatureMismatch {
        application: PersistentCallbackApplicationId,
    },
    CAbiSignatureSetMismatch,
    CAbiLayoutSetMismatch,
    NativeRequirementSetMismatch,
    NativeName(scoop_identity::CanonicalNativeNameError),
    NativeSymbol(scoop_identity::NativeLinkSymbolError),
    CanonicalCAbi(scoop_identity::CanonicalCAbiError),
    ScoopAbi(scoop_identity::ScoopAbiError),
    Hash(scoop_wire::HashError),
}

impl std::fmt::Display for NativeBoundaryTargetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "native boundary target normalization failed: {self:?}"
        )
    }
}

impl std::error::Error for NativeBoundaryTargetError {}

impl From<NativeBoundaryTargetError> for NativeBoundaryCompileError {
    fn from(error: NativeBoundaryTargetError) -> Self {
        Self::Target(error)
    }
}
