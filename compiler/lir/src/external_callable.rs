//! Typed external Scoop callables shared by compiler protocols and dependency selections.

use std::fmt;

use scoop_identity::{
    CallableBodyKey, CanonicalScoopAbiFunctionSignature, ConeIdentity,
    DependencyCallableDeclarationId, GcEffect as CanonicalGcEffect, ObjectDefinitionPlanId,
    PersistentCallableBodyId, PersistentSymbolRequest, StrongCallableDefinitionOwner,
};
use scoop_wire::HashError;

use crate::{
    AbiStorageSignature, CallingConvention, GcEffect, ScoopAbiSignature,
    SelectedDependencyLirCallableV1,
};

/// One external definition declared by the consumer's LLVM module. The
/// selected semantic bridge owns all link-facing facts; the target-specific
/// signature binds consumer-local storage to the provider's saved physical ABI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalCallable {
    origin: ExternalCallableOrigin,
    provider: ConeIdentity,
    target: StrongCallableDefinitionOwner,
    body: PersistentCallableBodyId,
    canonical_signature: CanonicalScoopAbiFunctionSignature,
    signature: ScoopAbiSignature,
    calling_convention: CallingConvention,
    root_plan: crate::ExternalCallableRootPlan,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalCallableOrigin {
    Legacy(DependencyCallableDeclarationId),
    LayoutV1,
}

impl ExternalCallable {
    pub(crate) fn new(
        selected: SelectedDependencyLirCallableV1,
        signature: AbiStorageSignature,
    ) -> Result<Self, ExternalCallableBuildError> {
        let bridge = selected.bridge();
        Self::from_direct(
            selected.provider(),
            bridge,
            signature,
            ExternalCallableOrigin::Legacy(bridge.declaration()),
        )
    }

    fn from_direct(
        provider: ConeIdentity,
        bridge: &crate::ParamFreeLirCallableExportV1,
        signature: AbiStorageSignature,
        origin: ExternalCallableOrigin,
    ) -> Result<Self, ExternalCallableBuildError> {
        let signature = signature.bind(bridge.abi_signature(), bridge.calling_convention())?;
        validate_root_plan(bridge.abi_signature(), bridge.root_plan())?;
        let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(bridge.target()))
            .map_err(ExternalCallableBuildError::Identity)?;
        Ok(Self {
            origin,
            provider,
            target: bridge.target(),
            body,
            canonical_signature: bridge.abi_signature().clone(),
            signature,
            calling_convention: bridge.calling_convention(),
            root_plan: bridge.root_plan(),
            expected_symbol: bridge.expected_symbol(),
            required_definition: bridge.required_definition(),
        })
    }

    pub(crate) fn from_layout_direct(
        provider: ConeIdentity,
        record: &crate::ParamFreeLirCallableExportV1,
        signature: AbiStorageSignature,
    ) -> Result<Self, crate::LayoutExternalMaterializationError> {
        Self::from_direct(
            provider,
            record,
            signature,
            ExternalCallableOrigin::LayoutV1,
        )
        .map_err(crate::LayoutExternalMaterializationError::CallableAbi)
    }

    pub(crate) fn from_layout_v1(
        provider: ConeIdentity,
        record: &crate::ExactCallableAbiExportV1,
        expected_symbol: PersistentSymbolRequest,
        required_definition: ObjectDefinitionPlanId,
        signature: AbiStorageSignature,
    ) -> Result<Self, crate::LayoutExternalMaterializationError> {
        let scoop_identity::CallableDefinitionOwner::Strong(target) = record.target() else {
            return Err(crate::LayoutExternalMaterializationError::PhysicalCallable(
                record.target(),
            ));
        };
        let root_plan = match record.call_protocol() {
            crate::ExactCallableProtocolV1::OrdinaryManaged => {
                crate::ExternalCallableRootPlan::ManagedStatepoint
            }
            crate::ExactCallableProtocolV1::OrdinaryNoGc => crate::ExternalCallableRootPlan::NoGc,
        };
        let signature = signature
            .bind(record.canonical_signature(), record.calling_convention())
            .map_err(crate::LayoutExternalMaterializationError::CallableAbi)?;
        validate_root_plan(record.canonical_signature(), root_plan)
            .map_err(crate::LayoutExternalMaterializationError::CallableAbi)?;
        let physical = record.physical_definition();
        if physical.provider() != provider
            || physical.symbol() != expected_symbol
            || physical.definition() != required_definition
        {
            return Err(crate::LayoutExternalMaterializationError::PhysicalCallable(
                record.target(),
            ));
        }
        Ok(Self {
            origin: ExternalCallableOrigin::LayoutV1,
            provider,
            target,
            body: record.definition().semantic_id(),
            canonical_signature: record.canonical_signature().clone(),
            signature,
            calling_convention: record.calling_convention(),
            root_plan,
            expected_symbol,
            required_definition,
        })
    }

    pub const fn origin(&self) -> ExternalCallableOrigin {
        self.origin
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn legacy_declaration(&self) -> Option<DependencyCallableDeclarationId> {
        match self.origin {
            ExternalCallableOrigin::Legacy(declaration) => Some(declaration),
            ExternalCallableOrigin::LayoutV1 => None,
        }
    }

    pub const fn target(&self) -> StrongCallableDefinitionOwner {
        self.target
    }

    pub const fn body(&self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn canonical_signature(&self) -> &CanonicalScoopAbiFunctionSignature {
        &self.canonical_signature
    }

    pub const fn signature(&self) -> &ScoopAbiSignature {
        &self.signature
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }

    pub const fn root_plan(&self) -> crate::ExternalCallableRootPlan {
        self.root_plan
    }

    pub const fn gc_effect(&self) -> GcEffect {
        self.root_plan().gc_effect()
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
}

fn validate_root_plan(
    canonical: &CanonicalScoopAbiFunctionSignature,
    root_plan: ExternalCallableRootPlan,
) -> Result<(), ExternalCallableBuildError> {
    let expected = match canonical.gc_effect() {
        CanonicalGcEffect::Managed => GcEffect::Managed,
        CanonicalGcEffect::NoGc => GcEffect::NoGc,
    };
    if root_plan.gc_effect() != expected {
        return Err(ExternalCallableBuildError::RootProtocolMismatch);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalCallableBuildError {
    AbiArgumentCount { expected: usize, actual: usize },
    AbiArgumentMismatch { index: usize },
    AbiResultMismatch,
    RootProtocolMismatch,
    Identity(HashError),
}

impl fmt::Display for ExternalCallableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbiArgumentCount { expected, actual } => write!(
                formatter,
                "external signature requires {expected} logical ABI arguments, found {actual}"
            ),
            Self::AbiArgumentMismatch { index } => write!(
                formatter,
                "external canonical and physical ABI disagree at argument {index}"
            ),
            Self::AbiResultMismatch => {
                formatter.write_str("external canonical and physical result ABI disagree")
            }
            Self::RootProtocolMismatch => formatter
                .write_str("external caller root protocol disagrees with its canonical GC effect"),
            Self::Identity(source) => {
                write!(formatter, "cannot derive external identity: {source}")
            }
        }
    }
}

impl std::error::Error for ExternalCallableBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(source) => Some(source),
            Self::AbiArgumentCount { .. }
            | Self::AbiArgumentMismatch { .. }
            | Self::AbiResultMismatch
            | Self::RootProtocolMismatch => None,
        }
    }
}

mod roots;
pub use roots::ExternalCallableRootPlan;

#[cfg(test)]
mod tests;
