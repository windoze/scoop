use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::{
    CallableMaterializationContextV1, CallbackModeV1, EffectV1, LexicalCallableParentV1,
    OptionalSignatureTypeV1, SignatureTypeKeyV1, SourceCAbiFunctionSignatureV1, SourceCAbiReturnV1,
    StructuralDefinitionPathV1,
};
use crate::ids::derive_persistent_id;
use crate::{PersistentCallbackApplicationId, PersistentCallbackRegistrationId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallbackParameterIndex(u32);

impl CallbackParameterIndex {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl WireEncodeV1 for CallbackParameterIndex {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.0))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SignatureCallableShapeV1 {
    effect: EffectV1,
    receiver: OptionalSignatureTypeV1,
    parameters: Vec<SignatureTypeKeyV1>,
    result: SignatureTypeKeyV1,
}

impl SignatureCallableShapeV1 {
    pub fn new(
        effect: EffectV1,
        receiver: Option<SignatureTypeKeyV1>,
        parameters: Vec<SignatureTypeKeyV1>,
        result: SignatureTypeKeyV1,
    ) -> Self {
        Self {
            effect,
            receiver: OptionalSignatureTypeV1::from_option(receiver),
            parameters,
            result,
        }
    }

    pub const fn effect(&self) -> EffectV1 {
        self.effect
    }

    pub fn receiver(&self) -> &OptionalSignatureTypeV1 {
        &self.receiver
    }

    pub fn parameters(&self) -> &[SignatureTypeKeyV1] {
        &self.parameters
    }

    pub fn result(&self) -> &SignatureTypeKeyV1 {
        &self.result
    }

    fn contains_binder(&self) -> bool {
        optional_contains_binder(&self.receiver)
            || self
                .parameters
                .iter()
                .any(SignatureTypeKeyV1::contains_binder)
            || self.result.contains_binder()
    }
}

impl WireEncodeV1 for SignatureCallableShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.effect.encode(encoder)?;
        encoder.field(2)?;
        self.receiver.encode(encoder)?;
        encoder.field(3)?;
        encode_signature_types(encoder, &self.parameters)?;
        encoder.field(4)?;
        self.result.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallbackRegistrationKeyV1 {
    parent: LexicalCallableParentV1,
    path: StructuralDefinitionPathV1,
    source_signature: SourceCAbiFunctionSignatureV1,
    context_index: CallbackParameterIndex,
    managed_signature: SignatureCallableShapeV1,
    mode: CallbackModeV1,
}

impl CallbackRegistrationKeyV1 {
    pub fn new(
        parent: LexicalCallableParentV1,
        path: StructuralDefinitionPathV1,
        source_signature: SourceCAbiFunctionSignatureV1,
        context_index: CallbackParameterIndex,
        managed_signature: SignatureCallableShapeV1,
        mode: CallbackModeV1,
    ) -> Self {
        Self {
            parent,
            path,
            source_signature,
            context_index,
            managed_signature,
            mode,
        }
    }

    pub const fn parent(&self) -> LexicalCallableParentV1 {
        self.parent
    }

    pub fn path(&self) -> &StructuralDefinitionPathV1 {
        &self.path
    }

    pub fn source_signature(&self) -> &SourceCAbiFunctionSignatureV1 {
        &self.source_signature
    }

    pub const fn context_index(&self) -> CallbackParameterIndex {
        self.context_index
    }

    pub fn managed_signature(&self) -> &SignatureCallableShapeV1 {
        &self.managed_signature
    }

    pub const fn mode(&self) -> CallbackModeV1 {
        self.mode
    }

    fn contains_binder(&self) -> bool {
        self.source_signature
            .parameters()
            .iter()
            .any(SignatureTypeKeyV1::contains_binder)
            || matches!(
                self.source_signature.result(),
                SourceCAbiReturnV1::Value(result) if result.contains_binder()
            )
            || self.managed_signature.contains_binder()
    }
}

impl WireEncodeV1 for CallbackRegistrationKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.parent.encode(encoder)?;
        encoder.field(2)?;
        self.path.encode(encoder)?;
        encoder.field(3)?;
        self.source_signature.encode(encoder)?;
        encoder.field(4)?;
        self.context_index.encode(encoder)?;
        encoder.field(5)?;
        self.managed_signature.encode(encoder)?;
        encoder.field(6)?;
        self.mode.encode(encoder)
    }
}

impl PersistentCallbackRegistrationId {
    pub fn from_key(key: &CallbackRegistrationKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-callback-registration-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallbackApplicationKeyV1 {
    registration: PersistentCallbackRegistrationId,
    context: CallableMaterializationContextV1,
}

impl CallbackApplicationKeyV1 {
    pub fn new(
        registration: &CallbackRegistrationKeyV1,
        context: CallableMaterializationContextV1,
    ) -> Result<Self, CallbackApplicationIdentityError> {
        if context == CallableMaterializationContextV1::NoSubstitution
            && registration.contains_binder()
        {
            return Err(CallbackApplicationIdentityError::BinderRequiresSubstitution);
        }
        let registration = PersistentCallbackRegistrationId::from_key(registration)
            .map_err(CallbackApplicationIdentityError::Hash)?;
        Ok(Self {
            registration,
            context,
        })
    }

    pub const fn registration(&self) -> PersistentCallbackRegistrationId {
        self.registration
    }

    pub const fn context(&self) -> CallableMaterializationContextV1 {
        self.context
    }
}

impl WireEncodeV1 for CallbackApplicationKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.registration.encode(encoder)?;
        encoder.field(2)?;
        self.context.encode(encoder)
    }
}

impl PersistentCallbackApplicationId {
    pub fn from_key(key: &CallbackApplicationKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-callback-application-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackApplicationIdentityError {
    BinderRequiresSubstitution,
    Hash(HashError),
}

impl fmt::Display for CallbackApplicationIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinderRequiresSubstitution => formatter.write_str(
                "callback registration containing binders requires a materialization context",
            ),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CallbackApplicationIdentityError {}

fn optional_contains_binder(value: &OptionalSignatureTypeV1) -> bool {
    matches!(value, OptionalSignatureTypeV1::Present(value) if value.contains_binder())
}

fn encode_signature_types(
    encoder: &mut Encoder,
    values: &[SignatureTypeKeyV1],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        CallbackApplicationIdentityError, CallbackApplicationKeyV1, CallbackParameterIndex,
        CallbackRegistrationKeyV1, SignatureCallableShapeV1,
    };
    use crate::{
        CallableMaterializationContextV1, CallbackModeV1, ConeIdentity, EffectV1,
        LexicalCallableParentV1, PersistentCallbackApplicationId, PersistentCallbackRegistrationId,
        PersistentFunctionId, PersistentTypeId, SignatureTypeKeyV1, SourceCAbiFunctionSignatureV1,
        SourceCAbiReturnV1, StructuralDefinitionPathV1, StructuralDefinitionSiteRoleV1,
        StructuralPathSegmentV1,
    };

    #[test]
    fn registration_and_application_have_fixed_identity() {
        let key = registration(SignatureTypeKeyV1::Nominal(PersistentTypeId(
            ConeIdentity::CORE.0,
        )));
        assert_eq!(
            hex(&encode(&key).unwrap()),
            "a601a200010158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d0281a20106020003a2018002a10001040005a4010102a10001038004a200010158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d0601"
        );
        assert_eq!(
            PersistentCallbackRegistrationId::from_key(&key)
                .unwrap()
                .to_string(),
            "9d03e2a9e1d68de24365c10dfae233afc3cd55554220602c46d805d55deaa33a"
        );
        let application =
            CallbackApplicationKeyV1::new(&key, CallableMaterializationContextV1::NoSubstitution)
                .unwrap();
        assert_eq!(
            PersistentCallbackApplicationId::from_key(&application)
                .unwrap()
                .to_string(),
            "3928f0f82889f35a1cfe65a029da13259707961cf6934bf87429045f5c75b2dd"
        );
    }

    #[test]
    fn binder_requires_materialization_context() {
        let key = registration(SignatureTypeKeyV1::Binder { depth: 0, index: 0 });
        assert_eq!(
            CallbackApplicationKeyV1::new(&key, CallableMaterializationContextV1::NoSubstitution,),
            Err(CallbackApplicationIdentityError::BinderRequiresSubstitution)
        );
    }

    #[test]
    fn context_index_is_zero_based_and_typed() {
        assert_eq!(CallbackParameterIndex::new(0).get(), 0);
        assert_eq!(encode(&CallbackParameterIndex::new(3)).unwrap(), b"\x03");
    }

    fn registration(result: SignatureTypeKeyV1) -> CallbackRegistrationKeyV1 {
        CallbackRegistrationKeyV1::new(
            LexicalCallableParentV1::function(PersistentFunctionId(ConeIdentity::CORE.0)),
            StructuralDefinitionPathV1::from_first(
                StructuralPathSegmentV1::new(StructuralDefinitionSiteRoleV1::CallbackConversion, 0),
                [],
            ),
            SourceCAbiFunctionSignatureV1::new(Vec::new(), SourceCAbiReturnV1::Void),
            CallbackParameterIndex::new(0),
            SignatureCallableShapeV1::new(EffectV1::Ordinary, None, Vec::new(), result),
            CallbackModeV1::Reusable,
        )
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
