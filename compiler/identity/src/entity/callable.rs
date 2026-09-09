use scoop_wire::{Encoder, HashError, WireEncode};

use super::{NonEmptyVec, PropertyOwner};
use crate::ids::derive_persistent_id;
use crate::{
    PersistentCallableApplicationId, PersistentConstructorId, PersistentExactTypeId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentInitializationUnitId, PersistentPropertyAccessorId,
};

mod decode;

pub use decode::{
    CallableApplicationResolutionError, DecodedCallableApplicationKey, DecodedCallableArguments,
    DecodedCallableInstantiationOwner, DecodedCallableMaterialization,
    DecodedCallableMaterializationContext, DecodedCallableTemplateOrigin,
    DecodedCallableTemplateOwner, DecodedPropertyAccessorKey,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AccessorRole {
    Getter,
    Setter,
}

impl WireEncode for AccessorRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Getter => 1,
            Self::Setter => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PropertyAccessorKey {
    owner: PropertyOwner,
    role: AccessorRole,
}

impl PropertyAccessorKey {
    pub const fn new(owner: PropertyOwner, role: AccessorRole) -> Self {
        Self { owner, role }
    }

    pub const fn owner(&self) -> PropertyOwner {
        self.owner
    }

    pub const fn role(&self) -> AccessorRole {
        self.role
    }
}

impl WireEncode for PropertyAccessorKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl PersistentPropertyAccessorId {
    pub fn from_key(key: &PropertyAccessorKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-property-accessor-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableTemplateOrigin {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    Accessor(PersistentPropertyAccessorId),
}

impl WireEncode for CallableTemplateOrigin {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_id_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_id_sum(encoder, 2, id),
            Self::Constructor(id) => encode_id_sum(encoder, 3, id),
            Self::Accessor(id) => encode_id_sum(encoder, 4, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableInstantiationOwner {
    NoOwner,
    ExactNominalOwner(PersistentExactTypeId),
    EnclosingCallableApplication(PersistentCallableApplicationId),
    EnclosingInitializationApplication(PersistentInitializationUnitId),
}

impl WireEncode for CallableInstantiationOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoOwner => encode_empty_sum(encoder, 1),
            Self::ExactNominalOwner(id) => encode_id_sum(encoder, 2, id),
            Self::EnclosingCallableApplication(id) => encode_id_sum(encoder, 3, id),
            Self::EnclosingInitializationApplication(id) => encode_id_sum(encoder, 4, id),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableArguments {
    NoCallableArguments,
    Arguments(NonEmptyVec<PersistentExactTypeId>),
}

impl CallableArguments {
    pub fn has_arguments(&self) -> bool {
        matches!(self, Self::Arguments(_))
    }
}

impl WireEncode for CallableArguments {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoCallableArguments => encode_empty_sum(encoder, 1),
            Self::Arguments(arguments) => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.array(arguments.as_slice().len() as u64)?;
                for argument in arguments.as_slice() {
                    argument.encode(encoder)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableApplicationKey {
    origin: CallableTemplateOrigin,
    instantiation_owner: CallableInstantiationOwner,
    callable_arguments: CallableArguments,
}

impl CallableApplicationKey {
    pub const fn for_function(
        origin: PersistentFunctionId,
        instantiation_owner: CallableInstantiationOwner,
    ) -> Self {
        Self {
            origin: CallableTemplateOrigin::Function(origin),
            instantiation_owner,
            callable_arguments: CallableArguments::NoCallableArguments,
        }
    }

    pub fn for_generic_function(
        origin: PersistentGenericFunctionId,
        instantiation_owner: CallableInstantiationOwner,
        callable_arguments: NonEmptyVec<PersistentExactTypeId>,
    ) -> Self {
        Self {
            origin: CallableTemplateOrigin::GenericFunction(origin),
            instantiation_owner,
            callable_arguments: CallableArguments::Arguments(callable_arguments),
        }
    }

    pub const fn for_constructor(
        origin: PersistentConstructorId,
        instantiation_owner: CallableInstantiationOwner,
    ) -> Self {
        Self {
            origin: CallableTemplateOrigin::Constructor(origin),
            instantiation_owner,
            callable_arguments: CallableArguments::NoCallableArguments,
        }
    }

    pub const fn for_accessor(
        origin: PersistentPropertyAccessorId,
        instantiation_owner: CallableInstantiationOwner,
    ) -> Self {
        Self {
            origin: CallableTemplateOrigin::Accessor(origin),
            instantiation_owner,
            callable_arguments: CallableArguments::NoCallableArguments,
        }
    }

    pub fn for_generic_extension_accessor(
        origin: PersistentPropertyAccessorId,
        instantiation_owner: CallableInstantiationOwner,
        callable_arguments: NonEmptyVec<PersistentExactTypeId>,
    ) -> Self {
        Self {
            origin: CallableTemplateOrigin::Accessor(origin),
            instantiation_owner,
            callable_arguments: CallableArguments::Arguments(callable_arguments),
        }
    }

    pub const fn origin(&self) -> CallableTemplateOrigin {
        self.origin
    }

    pub const fn instantiation_owner(&self) -> CallableInstantiationOwner {
        self.instantiation_owner
    }

    pub fn callable_arguments(&self) -> &CallableArguments {
        &self.callable_arguments
    }
}

impl WireEncode for CallableApplicationKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.instantiation_owner.encode(encoder)?;
        encoder.field(3)?;
        self.callable_arguments.encode(encoder)
    }
}

impl PersistentCallableApplicationId {
    pub fn from_key(key: &CallableApplicationKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-callable-application-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableTemplateOwner {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    Accessor(PersistentPropertyAccessorId),
    Generated(PersistentGeneratedCallableId),
}

impl WireEncode for CallableTemplateOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_id_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_id_sum(encoder, 2, id),
            Self::Constructor(id) => encode_id_sum(encoder, 3, id),
            Self::Accessor(id) => encode_id_sum(encoder, 4, id),
            Self::Generated(id) => encode_id_sum(encoder, 5, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableMaterializationContext {
    NoSubstitution,
    Application(PersistentCallableApplicationId),
    InitializationApplication(PersistentInitializationUnitId),
}

impl WireEncode for CallableMaterializationContext {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoSubstitution => encode_empty_sum(encoder, 1),
            Self::Application(id) => encode_id_sum(encoder, 2, id),
            Self::InitializationApplication(id) => encode_id_sum(encoder, 3, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableMaterialization {
    template: CallableTemplateOwner,
    context: CallableMaterializationContext,
}

impl CallableMaterialization {
    pub const fn new(
        template: CallableTemplateOwner,
        context: CallableMaterializationContext,
    ) -> Self {
        Self { template, context }
    }

    pub const fn template(&self) -> CallableTemplateOwner {
        self.template
    }

    pub const fn context(&self) -> CallableMaterializationContext {
        self.context
    }
}

impl WireEncode for CallableMaterialization {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.template.encode(encoder)?;
        encoder.field(2)?;
        self.context.encode(encoder)
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
    id: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    id.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        AccessorRole, CallableApplicationKey, CallableInstantiationOwner, CallableMaterialization,
        CallableMaterializationContext, CallableTemplateOwner, PropertyAccessorKey,
    };
    use crate::{
        ConeIdentity, NonEmptyVec, PersistentCallableApplicationId, PersistentExactTypeId,
        PersistentFunctionId, PersistentGenericFunctionId, PersistentPropertyAccessorId,
        PersistentPropertyId, PropertyOwner,
    };

    #[test]
    fn property_accessor_role_is_part_of_the_identity() {
        let property = PersistentPropertyId(ConeIdentity::CORE.0);
        let getter_key =
            PropertyAccessorKey::new(PropertyOwner::Property(property), AccessorRole::Getter);
        let setter_key =
            PropertyAccessorKey::new(PropertyOwner::Property(property), AccessorRole::Setter);
        let getter = PersistentPropertyAccessorId::from_key(&getter_key).unwrap();
        let setter = PersistentPropertyAccessorId::from_key(&setter_key).unwrap();
        assert_ne!(getter, setter);
        assert_eq!(
            hex(&encode(&getter_key).unwrap()),
            format!("a201a20001015820{property}0201")
        );
    }

    #[test]
    fn callable_constructors_make_genericity_structural() {
        let function = PersistentFunctionId(ConeIdentity::CORE.0);
        let generic = PersistentGenericFunctionId(ConeIdentity::CORE.0);
        let exact = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        let ordinary =
            CallableApplicationKey::for_function(function, CallableInstantiationOwner::NoOwner);
        let generic = CallableApplicationKey::for_generic_function(
            generic,
            CallableInstantiationOwner::NoOwner,
            NonEmptyVec::from_first(exact, []),
        );
        assert!(!ordinary.callable_arguments().has_arguments());
        assert!(generic.callable_arguments().has_arguments());
    }

    #[test]
    fn callable_application_has_fixed_wire_and_hash() {
        let generic = PersistentGenericFunctionId(ConeIdentity::CORE.0);
        let exact = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        let key = CallableApplicationKey::for_generic_function(
            generic,
            CallableInstantiationOwner::NoOwner,
            NonEmptyVec::from_first(exact, []),
        );
        assert_eq!(
            hex(&encode(&key).unwrap()),
            format!("a301a20002015820{generic}02a1000103a2000201815820{exact}")
        );
        assert_eq!(
            PersistentCallableApplicationId::from_key(&key)
                .unwrap()
                .to_string(),
            "6cf76e118f0bdb8e12504687bc4b26c54f5527804f2ca4af74eda801fae966d4"
        );
    }

    #[test]
    fn materialization_keeps_template_and_context_separate() {
        let template = PersistentFunctionId(ConeIdentity::CORE.0);
        let application = PersistentCallableApplicationId(ConeIdentity::SINGLE_FILE.0);
        let materialization = CallableMaterialization::new(
            CallableTemplateOwner::Function(template),
            CallableMaterializationContext::Application(application),
        );
        assert_eq!(
            hex(&encode(&materialization).unwrap()),
            format!("a201a20001015820{template}02a20002015820{application}")
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
