//! Typed external definitions selected from dependency layout authorities.

use std::fmt;

use scoop_identity::{
    CallableBodyKey, CanonicalScoopAbiFunctionSignature, ConeIdentity,
    DependencyCallableDeclarationId, GcEffect as CanonicalGcEffect, ObjectDefinitionPlanId,
    PersistentCallableBodyId, PersistentExactTypeId, PersistentSymbolRequest,
    StrongCallableDefinitionOwner,
};
use scoop_wire::HashError;

use crate::{
    CallingConvention, DependencyExternalCallableRootPlanV1, GcEffect, ScoopAbiSignature,
    SelectedDependencyLirCallableV1,
    external_callable_abi::{CanonicalScoopAbiMismatch, validate_canonical_scoop_abi},
};

/// One dependency definition declared by the consumer's LLVM module. The
/// selected semantic bridge owns all link-facing facts; the target-specific
/// signature is admitted only after exact ABI replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyExternalCallable {
    origin: DependencyExternalCallableOrigin,
    provider: ConeIdentity,
    target: StrongCallableDefinitionOwner,
    body: PersistentCallableBodyId,
    canonical_signature: CanonicalScoopAbiFunctionSignature,
    signature: ScoopAbiSignature,
    calling_convention: CallingConvention,
    root_plan: DependencyExternalCallableRootPlanV1,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DependencyExternalCallableOrigin {
    Legacy(DependencyCallableDeclarationId),
    LayoutV1,
}

impl DependencyExternalCallable {
    pub(crate) fn new(
        selected: SelectedDependencyLirCallableV1,
        signature: ScoopAbiSignature,
    ) -> Result<Self, DependencyExternalBuildError> {
        let bridge = selected.bridge();
        validate_canonical_scoop_abi(bridge.abi_signature(), &signature).map_err(|error| {
            match error {
                CanonicalScoopAbiMismatch::ArgumentCount { expected, actual } => {
                    DependencyExternalBuildError::AbiArgumentCount { expected, actual }
                }
                CanonicalScoopAbiMismatch::Argument { index } => {
                    DependencyExternalBuildError::AbiArgumentMismatch { index }
                }
                CanonicalScoopAbiMismatch::Result => {
                    DependencyExternalBuildError::AbiResultMismatch
                }
            }
        })?;
        if signature.calling_convention() != bridge.calling_convention() {
            return Err(DependencyExternalBuildError::CallingConventionMismatch {
                expected: bridge.calling_convention(),
                actual: signature.calling_convention(),
            });
        }
        let expected_effect = match bridge.abi_signature().gc_effect() {
            CanonicalGcEffect::Managed => GcEffect::Managed,
            CanonicalGcEffect::NoGc => GcEffect::NoGc,
        };
        if root_plan_gc_effect(bridge.root_plan()) != expected_effect {
            return Err(DependencyExternalBuildError::RootProtocolMismatch);
        }
        let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(bridge.target()))
            .map_err(DependencyExternalBuildError::Identity)?;
        Ok(Self {
            origin: DependencyExternalCallableOrigin::Legacy(bridge.declaration()),
            provider: selected.provider(),
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

    pub(crate) fn from_layout_v1(
        provider: ConeIdentity,
        record: &crate::ExactCallableAbiExportV1,
        expected_symbol: PersistentSymbolRequest,
        required_definition: ObjectDefinitionPlanId,
        signature: ScoopAbiSignature,
        enums: &crate::EnumDefs,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<Self, crate::LayoutExternalMaterializationError> {
        record
            .validate_physical_signature(
                enums,
                &signature,
                protocol_effect(record.call_protocol()),
                meter,
            )
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
            origin: DependencyExternalCallableOrigin::LayoutV1,
            provider,
            target: record.target(),
            body: record.definition().semantic_id(),
            canonical_signature: record.canonical_signature().clone(),
            signature,
            calling_convention: record.calling_convention(),
            root_plan: match record.call_protocol() {
                crate::ExactCallableProtocolV1::OrdinaryManaged => {
                    DependencyExternalCallableRootPlanV1::ManagedStatepoint
                }
                crate::ExactCallableProtocolV1::OrdinaryNoGc => {
                    DependencyExternalCallableRootPlanV1::NoGc
                }
            },
            expected_symbol,
            required_definition,
        })
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn legacy_declaration(&self) -> Option<DependencyCallableDeclarationId> {
        match self.origin {
            DependencyExternalCallableOrigin::Legacy(declaration) => Some(declaration),
            DependencyExternalCallableOrigin::LayoutV1 => None,
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

    pub const fn root_plan(&self) -> DependencyExternalCallableRootPlanV1 {
        self.root_plan
    }

    pub const fn gc_effect(&self) -> GcEffect {
        root_plan_gc_effect(self.root_plan())
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
}

fn protocol_effect(protocol: crate::ExactCallableProtocolV1) -> GcEffect {
    match protocol {
        crate::ExactCallableProtocolV1::OrdinaryManaged => GcEffect::Managed,
        crate::ExactCallableProtocolV1::OrdinaryNoGc => GcEffect::NoGc,
    }
}

/// One layout-provider TypeDescriptor admitted by a complete layout/ABI
/// selection. The expected symbol and definition are retained from the
/// checked physical import; neither is reconstructed from a name.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DependencyExternalTypeDescriptorV2 {
    provider: ConeIdentity,
    target: PersistentExactTypeId,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
}

impl DependencyExternalTypeDescriptorV2 {
    pub(crate) const fn from_layout_v1(
        provider: ConeIdentity,
        target: PersistentExactTypeId,
        expected_symbol: PersistentSymbolRequest,
        required_definition: ObjectDefinitionPlanId,
    ) -> Self {
        Self {
            provider,
            target,
            expected_symbol,
            required_definition,
        }
    }

    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }

    pub const fn target(self) -> PersistentExactTypeId {
        self.target
    }

    pub const fn expected_symbol(self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
}

const fn root_plan_gc_effect(root_plan: DependencyExternalCallableRootPlanV1) -> GcEffect {
    match root_plan {
        DependencyExternalCallableRootPlanV1::ManagedStatepoint => GcEffect::Managed,
        DependencyExternalCallableRootPlanV1::NoGc => GcEffect::NoGc,
    }
}

#[derive(Debug)]
pub enum DependencyExternalBuildError {
    AbiArgumentCount {
        expected: usize,
        actual: usize,
    },
    AbiArgumentMismatch {
        index: usize,
    },
    AbiResultMismatch,
    CallingConventionMismatch {
        expected: CallingConvention,
        actual: CallingConvention,
    },
    RootProtocolMismatch,
    Identity(HashError),
}

impl fmt::Display for DependencyExternalBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbiArgumentCount { expected, actual } => write!(
                formatter,
                "dependency external signature requires {expected} logical ABI arguments, found {actual}"
            ),
            Self::AbiArgumentMismatch { index } => write!(
                formatter,
                "dependency external canonical and physical ABI disagree at argument {index}"
            ),
            Self::AbiResultMismatch => formatter
                .write_str("dependency external canonical and physical result ABI disagree"),
            Self::CallingConventionMismatch { expected, actual } => write!(
                formatter,
                "dependency external calling convention {actual:?} disagrees with {expected:?}"
            ),
            Self::RootProtocolMismatch => formatter.write_str(
                "dependency external caller root protocol disagrees with its canonical GC effect",
            ),
            Self::Identity(source) => {
                write!(
                    formatter,
                    "cannot derive dependency external identity: {source}"
                )
            }
        }
    }
}

impl std::error::Error for DependencyExternalBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(source) => Some(source),
            Self::AbiArgumentCount { .. }
            | Self::AbiArgumentMismatch { .. }
            | Self::AbiResultMismatch
            | Self::CallingConventionMismatch { .. }
            | Self::RootProtocolMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests;
