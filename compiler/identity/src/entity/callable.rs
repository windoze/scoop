use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::{NonEmptyVec, PropertyOwnerV1};
use crate::ids::derive_persistent_id;
use crate::{
    PersistentCallableApplicationId, PersistentConstructorId, PersistentExactTypeId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentInitializationUnitId, PersistentPropertyAccessorId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AccessorRoleV1 {
    Getter,
    Setter,
}

impl WireEncodeV1 for AccessorRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Getter => 1,
            Self::Setter => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PropertyAccessorKeyV1 {
    owner: PropertyOwnerV1,
    role: AccessorRoleV1,
}

impl PropertyAccessorKeyV1 {
    pub const fn new(owner: PropertyOwnerV1, role: AccessorRoleV1) -> Self {
        Self { owner, role }
    }

    pub const fn owner(&self) -> PropertyOwnerV1 {
        self.owner
    }

    pub const fn role(&self) -> AccessorRoleV1 {
        self.role
    }
}

impl WireEncodeV1 for PropertyAccessorKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl PersistentPropertyAccessorId {
    pub fn from_key(key: &PropertyAccessorKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-property-accessor-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableTemplateOriginV1 {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    Accessor(PersistentPropertyAccessorId),
}

impl WireEncodeV1 for CallableTemplateOriginV1 {
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
pub enum CallableInstantiationOwnerV1 {
    NoOwner,
    ExactNominalOwner(PersistentExactTypeId),
    EnclosingCallableApplication(PersistentCallableApplicationId),
    EnclosingInitializationApplication(PersistentInitializationUnitId),
}

impl WireEncodeV1 for CallableInstantiationOwnerV1 {
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
pub enum CallableArgumentsV1 {
    NoCallableArguments,
    Arguments(NonEmptyVec<PersistentExactTypeId>),
}

impl CallableArgumentsV1 {
    pub fn has_arguments(&self) -> bool {
        matches!(self, Self::Arguments(_))
    }
}

impl WireEncodeV1 for CallableArgumentsV1 {
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
pub struct CallableApplicationKeyV1 {
    origin: CallableTemplateOriginV1,
    instantiation_owner: CallableInstantiationOwnerV1,
    callable_arguments: CallableArgumentsV1,
}

impl CallableApplicationKeyV1 {
    pub const fn for_function(
        origin: PersistentFunctionId,
        instantiation_owner: CallableInstantiationOwnerV1,
    ) -> Self {
        Self {
            origin: CallableTemplateOriginV1::Function(origin),
            instantiation_owner,
            callable_arguments: CallableArgumentsV1::NoCallableArguments,
        }
    }

    pub fn for_generic_function(
        origin: PersistentGenericFunctionId,
        instantiation_owner: CallableInstantiationOwnerV1,
        callable_arguments: NonEmptyVec<PersistentExactTypeId>,
    ) -> Self {
        Self {
            origin: CallableTemplateOriginV1::GenericFunction(origin),
            instantiation_owner,
            callable_arguments: CallableArgumentsV1::Arguments(callable_arguments),
        }
    }

    pub const fn for_constructor(
        origin: PersistentConstructorId,
        instantiation_owner: CallableInstantiationOwnerV1,
    ) -> Self {
        Self {
            origin: CallableTemplateOriginV1::Constructor(origin),
            instantiation_owner,
            callable_arguments: CallableArgumentsV1::NoCallableArguments,
        }
    }

    pub const fn for_accessor(
        origin: PersistentPropertyAccessorId,
        instantiation_owner: CallableInstantiationOwnerV1,
    ) -> Self {
        Self {
            origin: CallableTemplateOriginV1::Accessor(origin),
            instantiation_owner,
            callable_arguments: CallableArgumentsV1::NoCallableArguments,
        }
    }

    pub fn for_generic_extension_accessor(
        origin: PersistentPropertyAccessorId,
        instantiation_owner: CallableInstantiationOwnerV1,
        callable_arguments: NonEmptyVec<PersistentExactTypeId>,
    ) -> Self {
        Self {
            origin: CallableTemplateOriginV1::Accessor(origin),
            instantiation_owner,
            callable_arguments: CallableArgumentsV1::Arguments(callable_arguments),
        }
    }

    pub const fn origin(&self) -> CallableTemplateOriginV1 {
        self.origin
    }

    pub const fn instantiation_owner(&self) -> CallableInstantiationOwnerV1 {
        self.instantiation_owner
    }

    pub fn callable_arguments(&self) -> &CallableArgumentsV1 {
        &self.callable_arguments
    }
}

impl WireEncodeV1 for CallableApplicationKeyV1 {
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
    pub fn from_key(key: &CallableApplicationKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-callable-application-id-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableTemplateOwnerV1 {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    Accessor(PersistentPropertyAccessorId),
    Generated(PersistentGeneratedCallableId),
}

impl WireEncodeV1 for CallableTemplateOwnerV1 {
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
pub enum CallableMaterializationContextV1 {
    NoSubstitution,
    Application(PersistentCallableApplicationId),
    InitializationApplication(PersistentInitializationUnitId),
}

impl WireEncodeV1 for CallableMaterializationContextV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoSubstitution => encode_empty_sum(encoder, 1),
            Self::Application(id) => encode_id_sum(encoder, 2, id),
            Self::InitializationApplication(id) => encode_id_sum(encoder, 3, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableMaterializationV1 {
    template: CallableTemplateOwnerV1,
    context: CallableMaterializationContextV1,
}

impl CallableMaterializationV1 {
    pub const fn new(
        template: CallableTemplateOwnerV1,
        context: CallableMaterializationContextV1,
    ) -> Self {
        Self { template, context }
    }

    pub const fn template(&self) -> CallableTemplateOwnerV1 {
        self.template
    }

    pub const fn context(&self) -> CallableMaterializationContextV1 {
        self.context
    }
}

impl WireEncodeV1 for CallableMaterializationV1 {
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
    id: &impl WireEncodeV1,
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
        AccessorRoleV1, CallableApplicationKeyV1, CallableInstantiationOwnerV1,
        CallableMaterializationContextV1, CallableMaterializationV1, CallableTemplateOwnerV1,
        PropertyAccessorKeyV1,
    };
    use crate::{
        ConeIdentity, NonEmptyVec, PersistentCallableApplicationId, PersistentExactTypeId,
        PersistentFunctionId, PersistentGenericFunctionId, PersistentPropertyAccessorId,
        PersistentPropertyId, PropertyOwnerV1,
    };

    #[test]
    fn property_accessor_role_is_part_of_the_identity() {
        let property = PersistentPropertyId(ConeIdentity::CORE.0);
        let getter_key =
            PropertyAccessorKeyV1::new(PropertyOwnerV1::Property(property), AccessorRoleV1::Getter);
        let setter_key =
            PropertyAccessorKeyV1::new(PropertyOwnerV1::Property(property), AccessorRoleV1::Setter);
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
            CallableApplicationKeyV1::for_function(function, CallableInstantiationOwnerV1::NoOwner);
        let generic = CallableApplicationKeyV1::for_generic_function(
            generic,
            CallableInstantiationOwnerV1::NoOwner,
            NonEmptyVec::from_first(exact, []),
        );
        assert!(!ordinary.callable_arguments().has_arguments());
        assert!(generic.callable_arguments().has_arguments());
    }

    #[test]
    fn callable_application_has_fixed_wire_and_hash() {
        let generic = PersistentGenericFunctionId(ConeIdentity::CORE.0);
        let exact = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        let key = CallableApplicationKeyV1::for_generic_function(
            generic,
            CallableInstantiationOwnerV1::NoOwner,
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
        let materialization = CallableMaterializationV1::new(
            CallableTemplateOwnerV1::Function(template),
            CallableMaterializationContextV1::Application(application),
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
