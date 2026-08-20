use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{
    CallableMaterializationContext, CallbackMode, Effect, LexicalCallableParent,
    OptionalSignatureType, SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn,
    StructuralDefinitionPath,
};
use crate::ids::derive_persistent_id;
use crate::{PersistentCallbackApplicationId, PersistentCallbackRegistrationId};

mod decode;

pub use decode::{
    CallbackIdentityResolutionError, DecodedCallbackApplicationKey, DecodedCallbackRegistrationKey,
    DecodedSignatureCallableShape,
};

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

impl WireEncode for CallbackParameterIndex {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.0))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SignatureCallableShape {
    effect: Effect,
    receiver: OptionalSignatureType,
    parameters: Vec<SignatureTypeKey>,
    result: SignatureTypeKey,
}

impl SignatureCallableShape {
    pub fn new(
        effect: Effect,
        receiver: Option<SignatureTypeKey>,
        parameters: Vec<SignatureTypeKey>,
        result: SignatureTypeKey,
    ) -> Self {
        Self {
            effect,
            receiver: OptionalSignatureType::from_option(receiver),
            parameters,
            result,
        }
    }

    pub const fn effect(&self) -> Effect {
        self.effect
    }

    pub fn receiver(&self) -> &OptionalSignatureType {
        &self.receiver
    }

    pub fn parameters(&self) -> &[SignatureTypeKey] {
        &self.parameters
    }

    pub fn result(&self) -> &SignatureTypeKey {
        &self.result
    }

    fn contains_binder(&self) -> bool {
        optional_contains_binder(&self.receiver)
            || self
                .parameters
                .iter()
                .any(SignatureTypeKey::contains_binder)
            || self.result.contains_binder()
    }
}

impl WireEncode for SignatureCallableShape {
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
pub struct CallbackRegistrationKey {
    parent: LexicalCallableParent,
    path: StructuralDefinitionPath,
    source_signature: SourceCAbiFunctionSignature,
    context_index: CallbackParameterIndex,
    managed_signature: SignatureCallableShape,
    mode: CallbackMode,
}

impl CallbackRegistrationKey {
    pub fn new(
        parent: LexicalCallableParent,
        path: StructuralDefinitionPath,
        source_signature: SourceCAbiFunctionSignature,
        context_index: CallbackParameterIndex,
        managed_signature: SignatureCallableShape,
        mode: CallbackMode,
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

    pub const fn parent(&self) -> LexicalCallableParent {
        self.parent
    }

    pub fn path(&self) -> &StructuralDefinitionPath {
        &self.path
    }

    pub fn source_signature(&self) -> &SourceCAbiFunctionSignature {
        &self.source_signature
    }

    pub const fn context_index(&self) -> CallbackParameterIndex {
        self.context_index
    }

    pub fn managed_signature(&self) -> &SignatureCallableShape {
        &self.managed_signature
    }

    pub const fn mode(&self) -> CallbackMode {
        self.mode
    }

    fn contains_binder(&self) -> bool {
        self.source_signature
            .parameters()
            .iter()
            .any(SignatureTypeKey::contains_binder)
            || matches!(
                self.source_signature.result(),
                SourceCAbiReturn::Value(result) if result.contains_binder()
            )
            || self.managed_signature.contains_binder()
    }
}

impl WireEncode for CallbackRegistrationKey {
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
    pub fn from_key(key: &CallbackRegistrationKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-callback-registration-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallbackApplicationKey {
    registration: PersistentCallbackRegistrationId,
    context: CallableMaterializationContext,
}

impl CallbackApplicationKey {
    pub fn new(
        registration: &CallbackRegistrationKey,
        context: CallableMaterializationContext,
    ) -> Result<Self, CallbackApplicationIdentityError> {
        if context == CallableMaterializationContext::NoSubstitution
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

    pub const fn context(&self) -> CallableMaterializationContext {
        self.context
    }
}

impl WireEncode for CallbackApplicationKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.registration.encode(encoder)?;
        encoder.field(2)?;
        self.context.encode(encoder)
    }
}

impl PersistentCallbackApplicationId {
    pub fn from_key(key: &CallbackApplicationKey) -> Result<Self, HashError> {
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

fn optional_contains_binder(value: &OptionalSignatureType) -> bool {
    matches!(value, OptionalSignatureType::Present(value) if value.contains_binder())
}

fn encode_signature_types(
    encoder: &mut Encoder,
    values: &[SignatureTypeKey],
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
        CallbackApplicationIdentityError, CallbackApplicationKey, CallbackParameterIndex,
        CallbackRegistrationKey, SignatureCallableShape,
    };
    use crate::{
        CallableMaterializationContext, CallbackMode, ConeIdentity, Effect, LexicalCallableParent,
        PersistentCallbackApplicationId, PersistentCallbackRegistrationId, PersistentFunctionId,
        PersistentTypeId, SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn,
        StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
    };

    #[test]
    fn registration_and_application_have_fixed_identity() {
        let key = registration(SignatureTypeKey::Nominal(PersistentTypeId(
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
            CallbackApplicationKey::new(&key, CallableMaterializationContext::NoSubstitution)
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
        let key = registration(SignatureTypeKey::Binder { depth: 0, index: 0 });
        assert_eq!(
            CallbackApplicationKey::new(&key, CallableMaterializationContext::NoSubstitution,),
            Err(CallbackApplicationIdentityError::BinderRequiresSubstitution)
        );
    }

    #[test]
    fn context_index_is_zero_based_and_typed() {
        assert_eq!(CallbackParameterIndex::new(0).get(), 0);
        assert_eq!(encode(&CallbackParameterIndex::new(3)).unwrap(), b"\x03");
    }

    fn registration(result: SignatureTypeKey) -> CallbackRegistrationKey {
        CallbackRegistrationKey::new(
            LexicalCallableParent::function(PersistentFunctionId(ConeIdentity::CORE.0)),
            StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::CallbackConversion, 0),
                [],
            ),
            SourceCAbiFunctionSignature::new(Vec::new(), SourceCAbiReturn::Void),
            CallbackParameterIndex::new(0),
            SignatureCallableShape::new(Effect::Ordinary, None, Vec::new(), result),
            CallbackMode::Reusable,
        )
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
