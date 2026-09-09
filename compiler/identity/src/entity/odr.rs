use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{CallableApplicationKey, NonEmptyVec, StructuralDefinitionPath};
use crate::ids::derive_persistent_id;
use crate::{
    OdrGroupId, OdrMemberId, PersistentCallableApplicationId, PersistentCallableBodyId,
    PersistentDispatchSlotId, PersistentDispatchTableId, PersistentExactTypeId,
    PersistentExtensionPropertyId, PersistentGeneratedCallableId, PersistentGenericTypeId,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentLayoutId,
    PersistentScanId, PersistentStaticStorageId, PersistentTypeId,
};

mod decode;

pub use decode::{
    DecodedOdrMemberDiscriminator, DecodedOdrMemberKey, DecodedSpecializationKey,
    OdrIdentityResolutionError, OdrMemberResolver, SpecializationResolver,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SpecializationKey {
    Nominal {
        origin: PersistentGenericTypeId,
        arguments: NonEmptyVec<PersistentExactTypeId>,
    },
    Callable {
        application: CallableApplicationKey,
    },
    DelegatedProperty {
        origin: PersistentExtensionPropertyId,
        receiver_arguments: NonEmptyVec<PersistentExactTypeId>,
    },
    StructuralType {
        exact_type: PersistentExactTypeId,
    },
}

impl WireEncode for SpecializationKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal { origin, arguments } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                origin.encode(encoder)?;
                encoder.field(2)?;
                encode_exact_types(encoder, arguments)
            }
            Self::Callable { application } => encode_value_sum(encoder, 2, application),
            Self::DelegatedProperty {
                origin,
                receiver_arguments,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                origin.encode(encoder)?;
                encoder.field(2)?;
                encode_exact_types(encoder, receiver_arguments)
            }
            Self::StructuralType { exact_type } => encode_value_sum(encoder, 4, exact_type),
        }
    }
}

impl OdrGroupId {
    pub fn from_key(key: &SpecializationKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-odr-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OdrMemberRole {
    CallableBody,
    GeneratedNominal,
    Layout,
    ScanProgram,
    TypeDescriptor,
    DispatchTable,
    DispatchAdapter,
    StaticStorage,
    ImmortalObject,
    InitializationCell,
    InitializationDescriptor,
    RegistrationRecord,
    DiagnosticBytes,
    AddressTakenConstant,
    ObjectSupport,
    ReleaseHook,
}

impl WireEncode for OdrMemberRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::CallableBody => 1,
            Self::GeneratedNominal => 2,
            Self::Layout => 3,
            Self::ScanProgram => 4,
            Self::TypeDescriptor => 5,
            Self::DispatchTable => 6,
            Self::DispatchAdapter => 7,
            Self::StaticStorage => 8,
            Self::ImmortalObject => 9,
            Self::InitializationCell => 10,
            Self::InitializationDescriptor => 11,
            Self::RegistrationRecord => 12,
            Self::DiagnosticBytes => 13,
            Self::AddressTakenConstant => 14,
            Self::ObjectSupport => 15,
            Self::ReleaseHook => 16,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OdrMemberDiscriminator {
    Singleton,
    CallableApplication(PersistentCallableApplicationId),
    GeneratedCallable(PersistentGeneratedCallableId),
    GeneratedNominal(PersistentTypeId),
    ExactType(PersistentExactTypeId),
    Layout(PersistentLayoutId),
    Scan(PersistentScanId),
    DispatchTable(PersistentDispatchTableId),
    DispatchSlot(PersistentDispatchSlotId),
    StaticStorage(PersistentStaticStorageId),
    ImmortalObject(PersistentImmortalObjectId),
    InitializationUnit(PersistentInitializationUnitId),
    StructuralPath(StructuralDefinitionPath),
    CallableBody(PersistentCallableBodyId),
    SafepointSite(crate::PersistentSafepointSiteId),
}

impl WireEncode for OdrMemberDiscriminator {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Singleton => encode_empty_sum(encoder, 1),
            Self::CallableApplication(id) => encode_value_sum(encoder, 2, id),
            Self::GeneratedCallable(id) => encode_value_sum(encoder, 3, id),
            Self::GeneratedNominal(id) => encode_value_sum(encoder, 4, id),
            Self::ExactType(id) => encode_value_sum(encoder, 5, id),
            Self::Layout(id) => encode_value_sum(encoder, 6, id),
            Self::Scan(id) => encode_value_sum(encoder, 7, id),
            Self::DispatchTable(id) => encode_value_sum(encoder, 8, id),
            Self::DispatchSlot(id) => encode_value_sum(encoder, 9, id),
            Self::StaticStorage(id) => encode_value_sum(encoder, 10, id),
            Self::ImmortalObject(id) => encode_value_sum(encoder, 11, id),
            Self::InitializationUnit(id) => encode_value_sum(encoder, 12, id),
            Self::StructuralPath(path) => encode_value_sum(encoder, 13, path),
            Self::CallableBody(id) => encode_value_sum(encoder, 14, id),
            Self::SafepointSite(id) => encode_value_sum(encoder, 15, id),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OdrMemberKey {
    group: OdrGroupId,
    role: OdrMemberRole,
    discriminator: OdrMemberDiscriminator,
}

impl OdrMemberKey {
    pub fn new(
        group: OdrGroupId,
        role: OdrMemberRole,
        discriminator: OdrMemberDiscriminator,
    ) -> Result<Self, OdrMemberIdentityError> {
        if !role_accepts_discriminator(role, &discriminator) {
            return Err(OdrMemberIdentityError::RoleDiscriminatorMismatch);
        }
        Ok(Self {
            group,
            role,
            discriminator,
        })
    }

    pub const fn group(&self) -> OdrGroupId {
        self.group
    }

    pub const fn role(&self) -> OdrMemberRole {
        self.role
    }

    pub fn discriminator(&self) -> &OdrMemberDiscriminator {
        &self.discriminator
    }
}

impl WireEncode for OdrMemberKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.group.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.discriminator.encode(encoder)
    }
}

impl OdrMemberId {
    pub fn from_key(key: &OdrMemberKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-odr-member-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableOdrMemberId(OdrMemberId);

impl CallableOdrMemberId {
    pub fn from_key(key: &OdrMemberKey) -> Result<Self, OdrMemberIdentityError> {
        if !is_callable_member(key.role, &key.discriminator) {
            return Err(OdrMemberIdentityError::ExpectedCallableMember);
        }
        OdrMemberId::from_key(key)
            .map(Self)
            .map_err(OdrMemberIdentityError::Hash)
    }

    pub const fn member(self) -> OdrMemberId {
        self.0
    }
}

impl WireEncode for CallableOdrMemberId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OdrMemberIdentityError {
    RoleDiscriminatorMismatch,
    ExpectedCallableMember,
    Hash(HashError),
}

impl fmt::Display for OdrMemberIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoleDiscriminatorMismatch => {
                formatter.write_str("ODR member role does not accept this discriminator")
            }
            Self::ExpectedCallableMember => formatter
                .write_str("ODR callable member requires a callable role and discriminator"),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for OdrMemberIdentityError {}

fn role_accepts_discriminator(role: OdrMemberRole, discriminator: &OdrMemberDiscriminator) -> bool {
    use OdrMemberDiscriminator as D;
    use OdrMemberRole as R;

    matches!(
        (role, discriminator),
        (
            R::CallableBody,
            D::CallableApplication(_) | D::GeneratedCallable(_) | D::InitializationUnit(_)
        ) | (R::GeneratedNominal, D::GeneratedNominal(_))
            | (R::Layout, D::Layout(_))
            | (R::ScanProgram, D::Scan(_))
            | (R::TypeDescriptor, D::ExactType(_))
            | (R::DispatchTable, D::DispatchTable(_))
            | (
                R::DispatchAdapter,
                D::DispatchSlot(_) | D::GeneratedCallable(_)
            )
            | (R::StaticStorage, D::StaticStorage(_))
            | (R::ImmortalObject, D::ImmortalObject(_))
            | (
                R::InitializationCell | R::InitializationDescriptor,
                D::InitializationUnit(_)
            )
            | (
                R::RegistrationRecord,
                D::CallableBody(_)
                    | D::SafepointSite(_)
                    | D::ExactType(_)
                    | D::StaticStorage(_)
                    | D::ImmortalObject(_)
                    | D::InitializationUnit(_)
            )
            | (
                R::DiagnosticBytes,
                D::ExactType(_)
                    | D::CallableApplication(_)
                    | D::GeneratedNominal(_)
                    | D::StructuralPath(_)
            )
            | (
                R::AddressTakenConstant,
                D::ImmortalObject(_) | D::StructuralPath(_)
            )
            | (R::ObjectSupport, D::Singleton | D::StructuralPath(_))
            | (R::ReleaseHook, D::ExactType(_))
    )
}

fn is_callable_member(role: OdrMemberRole, discriminator: &OdrMemberDiscriminator) -> bool {
    matches!(
        (role, discriminator),
        (
            OdrMemberRole::CallableBody,
            OdrMemberDiscriminator::CallableApplication(_)
                | OdrMemberDiscriminator::GeneratedCallable(_)
                | OdrMemberDiscriminator::InitializationUnit(_)
        ) | (
            OdrMemberRole::DispatchAdapter,
            OdrMemberDiscriminator::GeneratedCallable(_)
        )
    )
}

fn encode_exact_types(
    encoder: &mut Encoder,
    values: &NonEmptyVec<PersistentExactTypeId>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.as_slice().len() as u64)?;
    for value in values.as_slice() {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_value_sum(
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
    use super::{
        CallableOdrMemberId, OdrMemberDiscriminator, OdrMemberIdentityError, OdrMemberKey,
        OdrMemberRole, SpecializationKey,
    };
    use crate::{
        ConeIdentity, NonEmptyVec, OdrGroupId, OdrMemberId, PersistentExactTypeId,
        PersistentLayoutId,
    };

    #[test]
    fn nominal_specialization_and_layout_member_have_fixed_identities() {
        let origin = crate::PersistentGenericTypeId(ConeIdentity::CORE.0);
        let exact = PersistentExactTypeId(ConeIdentity::SINGLE_FILE.0);
        let group_key = SpecializationKey::Nominal {
            origin,
            arguments: NonEmptyVec::from_first(exact, []),
        };
        let group = OdrGroupId::from_key(&group_key).unwrap();
        assert_eq!(
            group.to_string(),
            "344d96411adce71434a292bc6a5b49788988f7620e798388818935171be5c61a"
        );

        let layout = PersistentLayoutId(ConeIdentity::CORE.0);
        let member_key = OdrMemberKey::new(
            group,
            OdrMemberRole::Layout,
            OdrMemberDiscriminator::Layout(layout),
        )
        .unwrap();
        assert_eq!(
            OdrMemberId::from_key(&member_key).unwrap().to_string(),
            "2d61aab6d20644b3459ccace2c8e7c1518b599c901e9341c1bc0e9b576a35e7b"
        );
    }

    #[test]
    fn role_discriminator_matrix_is_closed() {
        let group = OdrGroupId(ConeIdentity::CORE.0);
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        assert_eq!(
            OdrMemberKey::new(
                group,
                OdrMemberRole::Layout,
                OdrMemberDiscriminator::ExactType(exact),
            ),
            Err(OdrMemberIdentityError::RoleDiscriminatorMismatch)
        );
    }

    #[test]
    fn callable_refinement_rejects_non_callable_members() {
        let group = OdrGroupId(ConeIdentity::CORE.0);
        let layout = PersistentLayoutId(ConeIdentity::SINGLE_FILE.0);
        let key = OdrMemberKey::new(
            group,
            OdrMemberRole::Layout,
            OdrMemberDiscriminator::Layout(layout),
        )
        .unwrap();
        assert_eq!(
            CallableOdrMemberId::from_key(&key),
            Err(OdrMemberIdentityError::ExpectedCallableMember)
        );
    }

    #[test]
    fn callable_refinement_preserves_the_validated_member_id() {
        let group = OdrGroupId(ConeIdentity::CORE.0);
        let callable = crate::PersistentGeneratedCallableId(ConeIdentity::SINGLE_FILE.0);
        let key = OdrMemberKey::new(
            group,
            OdrMemberRole::CallableBody,
            OdrMemberDiscriminator::GeneratedCallable(callable),
        )
        .unwrap();

        assert_eq!(
            CallableOdrMemberId::from_key(&key).unwrap().member(),
            OdrMemberId::from_key(&key).unwrap()
        );
    }
}
