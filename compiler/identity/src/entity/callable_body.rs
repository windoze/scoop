use scoop_wire::{HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, WireEncode};

use super::CallableOdrMemberId;
use crate::ids::derive_runtime_persistent_id;
use crate::{
    ConeIdentity, PersistentCallableBodyId, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentInitializationUnitId, PersistentPropertyAccessorId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StrongCallableDefinitionOwner {
    Function(PersistentFunctionId),
    Constructor(PersistentConstructorId),
    PropertyAccessor(PersistentPropertyAccessorId),
    GeneratedCallable(PersistentGeneratedCallableId),
}

impl RuntimeEncode for StrongCallableDefinitionOwner {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        match self {
            Self::Function(id) => encode_runtime_sum(encoder, 1, id),
            Self::Constructor(id) => encode_runtime_sum(encoder, 2, id),
            Self::PropertyAccessor(id) => encode_runtime_sum(encoder, 3, id),
            Self::GeneratedCallable(id) => encode_runtime_sum(encoder, 4, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableBodyKeyKind {
    Strong(StrongCallableDefinitionOwner),
    Odr(CallableOdrMemberId),
    RootGateway {
        root_cone: ConeIdentity,
        main: MainCallableBodyId,
    },
    InitializationStartupGateway(PersistentInitializationUnitId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableBodyKey(CallableBodyKeyKind);

impl CallableBodyKey {
    pub const fn strong(owner: StrongCallableDefinitionOwner) -> Self {
        Self(CallableBodyKeyKind::Strong(owner))
    }

    pub const fn odr(member: CallableOdrMemberId) -> Self {
        Self(CallableBodyKeyKind::Odr(member))
    }

    pub const fn root_gateway(root_cone: ConeIdentity, main: MainCallableBodyId) -> Self {
        Self(CallableBodyKeyKind::RootGateway { root_cone, main })
    }

    pub const fn initialization_startup_gateway(unit: PersistentInitializationUnitId) -> Self {
        Self(CallableBodyKeyKind::InitializationStartupGateway(unit))
    }

    pub const fn kind(&self) -> CallableBodyKeyKind {
        self.0
    }
}

impl RuntimeEncode for CallableBodyKey {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        match self.0 {
            CallableBodyKeyKind::Strong(owner) => encode_runtime_sum(encoder, 1, &owner),
            CallableBodyKeyKind::Odr(member) => {
                encoder.u32(2)?;
                encoder.fixed(member.member().as_array())
            }
            CallableBodyKeyKind::RootGateway { root_cone, main } => {
                encoder.u32(3)?;
                encoder.fixed(root_cone.as_array())?;
                encoder.fixed(main.body().as_array())
            }
            CallableBodyKeyKind::InitializationStartupGateway(unit) => {
                encoder.u32(4)?;
                encoder.fixed(unit.as_array())
            }
        }
    }
}

impl PersistentCallableBodyId {
    pub fn from_key(key: &CallableBodyKey) -> Result<Self, HashError> {
        derive_runtime_persistent_id("scoop-callable-body-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MainCallableBodyId(PersistentCallableBodyId);

impl MainCallableBodyId {
    pub const fn body(self) -> PersistentCallableBodyId {
        self.0
    }
}

impl RuntimeEncode for MainCallableBodyId {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.fixed(self.0.as_array())
    }
}

impl WireEncode for MainCallableBodyId {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

fn encode_runtime_sum(
    encoder: &mut RuntimeEncoder,
    tag: u32,
    value: &impl RuntimeEncode,
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(tag)?;
    value.runtime_encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode_runtime;

    use super::{
        CallableBodyKey, CallableBodyKeyKind, MainCallableBodyId, StrongCallableDefinitionOwner,
    };
    use crate::{ConeIdentity, PersistentCallableBodyId, PersistentFunctionId};

    #[test]
    fn strong_callable_body_has_fixed_runtime_bytes_and_identity() {
        let function = PersistentFunctionId(ConeIdentity::CORE.0);
        let key = CallableBodyKey::strong(StrongCallableDefinitionOwner::Function(function));

        assert_eq!(
            encode_runtime(&key).unwrap(),
            [b"\x01\0\0\0\x01\0\0\0".as_slice(), function.as_array()].concat()
        );
        assert_eq!(
            PersistentCallableBodyId::from_key(&key)
                .unwrap()
                .to_string(),
            "e8de63b8e2758608238897adc56513f83fd4083605bfcbe293c67e8d41c9d2bd"
        );
    }

    #[test]
    fn root_gateway_keeps_root_and_main_as_separate_typed_fields() {
        let main_body = PersistentCallableBodyId(ConeIdentity::CORE.0);
        let main = MainCallableBodyId(main_body);
        let key = CallableBodyKey::root_gateway(ConeIdentity::SINGLE_FILE, main);
        let encoded = encode_runtime(&key).unwrap();

        assert_eq!(&encoded[..4], b"\x03\0\0\0");
        assert_eq!(&encoded[4..36], ConeIdentity::SINGLE_FILE.as_array());
        assert_eq!(&encoded[36..], main_body.as_array());
        assert_eq!(
            key.kind(),
            CallableBodyKeyKind::RootGateway {
                root_cone: ConeIdentity::SINGLE_FILE,
                main,
            }
        );
    }
}
