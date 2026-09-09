use scoop_wire::{Encoder, HashError, WireEncode};

use super::{CallableMaterialization, NonEmptyVec, StructuralDefinitionPath};
use crate::ids::derive_persistent_id;
use crate::{
    PersistentExactTypeId, PersistentExtensionPropertyId, PersistentInitializationUnitId,
    PersistentLocalValueId, PersistentPropertyId, PersistentTypeId,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InitializationUnitKey {
    TopLevelProperty(PersistentPropertyId),
    ExtensionProperty(PersistentExtensionPropertyId),
    Object(PersistentTypeId),
    Companion(PersistentTypeId),
    GenericDelegatedExtensionApplication {
        property: PersistentExtensionPropertyId,
        receiver_arguments: NonEmptyVec<PersistentExactTypeId>,
    },
}

impl WireEncode for InitializationUnitKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::TopLevelProperty(id) => encode_id_sum(encoder, 1, id),
            Self::ExtensionProperty(id) => encode_id_sum(encoder, 2, id),
            Self::Object(id) => encode_id_sum(encoder, 3, id),
            Self::Companion(id) => encode_id_sum(encoder, 4, id),
            Self::GenericDelegatedExtensionApplication {
                property,
                receiver_arguments,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 5)?;
                encoder.field(1)?;
                property.encode(encoder)?;
                encoder.field(2)?;
                encoder.array(receiver_arguments.as_slice().len() as u64)?;
                for argument in receiver_arguments.as_slice() {
                    argument.encode(encoder)?;
                }
                Ok(())
            }
        }
    }
}

impl PersistentInitializationUnitId {
    pub fn from_key(key: &InitializationUnitKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-initialization-unit-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntheticLocalRole {
    Temporary,
    DefaultValue,
    DesugaredIterator,
    CoroutineProtocol,
    CallbackContext,
}

impl WireEncode for SyntheticLocalRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Temporary => 1,
            Self::DefaultValue => 2,
            Self::DesugaredIterator => 3,
            Self::CoroutineProtocol => 4,
            Self::CallbackContext => 5,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LocalValueSelector {
    This,
    Parameter {
        declaration_index: u32,
    },
    LocalDeclaration {
        path: StructuralDefinitionPath,
    },
    BoundReceiver {
        path: StructuralDefinitionPath,
    },
    SuspensionResult {
        site: StructuralDefinitionPath,
    },
    Synthetic {
        path: StructuralDefinitionPath,
        role: SyntheticLocalRole,
    },
}

impl WireEncode for LocalValueSelector {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::This => encode_empty_sum(encoder, 1),
            Self::Parameter { declaration_index } => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*declaration_index))
            }
            Self::LocalDeclaration { path } => encode_id_sum(encoder, 3, path),
            Self::BoundReceiver { path } => encode_id_sum(encoder, 4, path),
            Self::SuspensionResult { site } => encode_id_sum(encoder, 5, site),
            Self::Synthetic { path, role } => {
                encoder.map(3)?;
                encode_tag(encoder, 6)?;
                encoder.field(1)?;
                path.encode(encoder)?;
                encoder.field(2)?;
                role.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalValueKey {
    owner: CallableMaterialization,
    selector: LocalValueSelector,
}

impl LocalValueKey {
    pub const fn new(owner: CallableMaterialization, selector: LocalValueSelector) -> Self {
        Self { owner, selector }
    }

    pub const fn owner(&self) -> CallableMaterialization {
        self.owner
    }

    pub fn selector(&self) -> &LocalValueSelector {
        &self.selector
    }
}

impl WireEncode for LocalValueKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.selector.encode(encoder)
    }
}

impl PersistentLocalValueId {
    pub fn from_key(key: &LocalValueKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-local-value-id-v1", key)
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_id_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{InitializationUnitKey, LocalValueKey, LocalValueSelector};
    use crate::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        ConeIdentity, NonEmptyVec, PersistentExactTypeId, PersistentExtensionPropertyId,
        PersistentFunctionId, PersistentInitializationUnitId, PersistentLocalValueId,
    };

    #[test]
    fn delegated_initialization_arguments_are_non_empty_and_have_fixed_identity() {
        let property = PersistentExtensionPropertyId(ConeIdentity::CORE.0);
        let exact = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        let key = InitializationUnitKey::GenericDelegatedExtensionApplication {
            property,
            receiver_arguments: NonEmptyVec::from_first(exact, []),
        };
        assert_eq!(
            hex(&encode(&key).unwrap()),
            format!("a30005015820{property}02815820{exact}")
        );
        assert_eq!(
            PersistentInitializationUnitId::from_key(&key)
                .unwrap()
                .to_string(),
            "f28e76568ebc7b2d2a62f0da0b448e368f80458985f60738a758d4dc043587d4"
        );
    }

    #[test]
    fn local_value_identity_keeps_materialization_and_selector() {
        let template = PersistentFunctionId(ConeIdentity::CORE.0);
        let owner = CallableMaterialization::new(
            CallableTemplateOwner::Function(template),
            CallableMaterializationContext::NoSubstitution,
        );
        let key = LocalValueKey::new(
            owner,
            LocalValueSelector::Parameter {
                declaration_index: 3,
            },
        );
        assert_eq!(
            hex(&encode(&key).unwrap()),
            format!("a201a201a20001015820{template}02a1000102a200020103")
        );
        assert_eq!(
            PersistentLocalValueId::from_key(&key).unwrap().to_string(),
            "8fea610342c8179e3b77c07cce8d45949815fadfda94b4de5552f810ec671491"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
