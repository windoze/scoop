//! Complete canonical ABI data shared by callable publication and selection.

use crate::{CallingConvention, ExternalCallableRootPlan};
use scoop_identity::{
    CallableBodyKey, CanonicalScoopAbiFunctionSignature, ConeIdentity, LinkageClass,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentCallableBodyId, PersistentSymbolKey,
    PersistentSymbolRequest, StrongCallableDefinitionOwner, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_wire::{Encoder, WireEncode};

mod errors;
pub use errors::*;
mod replay;
pub use replay::CallableAbiReplayError;
mod validation;
mod wire;
pub use wire::DecodedCallableAbiRecordV1;

/// Provider-relative ABI contract shared by ordinary and initialization callables.
/// The containing publication or selection supplies the provider explicitly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableAbiRecordV1 {
    target: StrongCallableDefinitionOwner,
    abi_signature: CanonicalScoopAbiFunctionSignature,
    expected_symbol: PersistentSymbolRequest,
    calling_convention: CallingConvention,
    root_plan: ExternalCallableRootPlan,
    required_definition: ObjectDefinitionPlanId,
}

impl CallableAbiRecordV1 {
    pub fn new(
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
        abi_signature: CanonicalScoopAbiFunctionSignature,
        calling_convention: CallingConvention,
        root_plan: ExternalCallableRootPlan,
    ) -> Result<Self, CallableAbiBuildError> {
        if abi_signature.signature().effect() != scoop_identity::Effect::Ordinary {
            return Err(CallableAbiBuildError::Suspend);
        }
        if root_plan.canonical_gc_effect() != abi_signature.gc_effect() {
            return Err(CallableAbiBuildError::RootProtocolMismatch {
                abi: abi_signature.gc_effect(),
                root: root_plan.canonical_gc_effect(),
            });
        }
        let (_, expected_symbol, required_definition) =
            derive_callable_link_contract(provider, target)
                .map_err(CallableAbiBuildError::Contract)?;
        Ok(Self {
            target,
            abi_signature,
            expected_symbol,
            calling_convention,
            root_plan,
            required_definition,
        })
    }

    pub const fn target(&self) -> StrongCallableDefinitionOwner {
        self.target
    }

    pub const fn abi_signature(&self) -> &CanonicalScoopAbiFunctionSignature {
        &self.abi_signature
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }

    pub const fn root_plan(&self) -> ExternalCallableRootPlan {
        self.root_plan
    }

    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
}

impl WireEncode for CallableAbiRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.abi_signature.encode(encoder)?;
        encoder.field(3)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(4)?;
        self.calling_convention.encode(encoder)?;
        encoder.field(5)?;
        self.root_plan.encode(encoder)?;
        encoder.field(6)?;
        self.required_definition.encode(encoder)
    }
}

pub(crate) fn derive_callable_link_contract(
    provider: ConeIdentity,
    target: StrongCallableDefinitionOwner,
) -> Result<
    (
        PersistentCallableBodyId,
        PersistentSymbolRequest,
        ObjectDefinitionPlanId,
    ),
    CallableLinkContractError,
> {
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target))
        .map_err(CallableLinkContractError::Identity)?;
    let expected_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableBody(body),
        LinkageClass::ConeStrong,
    )
    .map_err(CallableLinkContractError::Symbol)?;
    let definition_key = ObjectDefinitionPlanKey::strong(
        provider,
        StrongDefinitionEntity::callable_body(body),
        StrongDefinitionRole::CallableBody,
    )
    .map_err(CallableLinkContractError::Definition)?;
    let required_definition = ObjectDefinitionPlanId::from_key(&definition_key)
        .map_err(CallableLinkContractError::Identity)?;
    Ok((body, expected_symbol, required_definition))
}
