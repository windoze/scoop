//! Persistent identity bundle for MIR-generated boxed value types.

use std::fmt;

use scoop_identity::{
    CborIdentityRecord, ExactTypeKey, FieldIdentityError, FieldIdentityKey,
    GeneratedNominalIdentityError, GeneratedNominalKey, OdrGroupId, OdrMemberDiscriminator,
    OdrMemberId, OdrMemberIdentityError, OdrMemberKey, OdrMemberRole, PersistentExactTypeId,
    PersistentFieldId, PersistentTypeId, SpecializationKey,
};
use scoop_wire::HashError;

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;
type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// The unique materialization root selected by `ExactOwnerRoot(payload)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoxedValueRoot {
    /// A parameter-free source nominal is materialized by its definition Cone.
    SourceNominal(PersistentTypeId),
    /// A nominal application reuses the specialization group emitted by HIR.
    NominalApplication(Box<BoxedValueNominalRoot>),
    /// A tuple creates its structural group as part of the MIR delta.
    Structural(Box<BoxedValueStructuralRoot>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxedValueNominalRoot {
    group: OdrGroupId,
    member: OdrMemberRecord,
}

impl BoxedValueNominalRoot {
    pub const fn group(&self) -> OdrGroupId {
        self.group
    }

    pub const fn member_record(&self) -> &OdrMemberRecord {
        &self.member
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxedValueStructuralRoot {
    group: OdrGroupRecord,
    member: OdrMemberRecord,
}

impl BoxedValueStructuralRoot {
    pub const fn group_record(&self) -> &OdrGroupRecord {
        &self.group
    }

    pub const fn member_record(&self) -> &OdrMemberRecord {
        &self.member
    }
}

/// Complete persistent identity projection for one generated value box.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxedValueIdentity {
    generated_type: GeneratedTypeRecord,
    payload_field: FieldRecord,
    root: BoxedValueRoot,
}

impl BoxedValueIdentity {
    pub fn for_source_nominal(payload: &ExactTypeRecord) -> Result<Self, BoxedValueIdentityError> {
        let ExactTypeKey::Nominal(owner) = payload.key() else {
            return Err(BoxedValueIdentityError::ExpectedSourceNominal);
        };
        Self::build(payload.id(), BoxedValueRoot::SourceNominal(*owner))
    }

    pub fn for_nominal_application(
        payload: &ExactTypeRecord,
        group: &OdrGroupRecord,
    ) -> Result<Self, BoxedValueIdentityError> {
        let ExactTypeKey::NominalApplication { origin, arguments } = payload.key() else {
            return Err(BoxedValueIdentityError::ExpectedNominalApplication);
        };
        let expected = SpecializationKey::Nominal {
            origin: *origin,
            arguments: arguments.clone(),
        };
        if group.key() != &expected {
            return Err(BoxedValueIdentityError::NominalGroupMismatch);
        }
        let generated_type = Self::generated_type(payload.id())?;
        let member = Self::odr_member(group.id(), generated_type.id())?;
        Self::finish(
            generated_type,
            BoxedValueRoot::NominalApplication(Box::new(BoxedValueNominalRoot {
                group: group.id(),
                member,
            })),
        )
    }

    pub fn for_tuple(payload: &ExactTypeRecord) -> Result<Self, BoxedValueIdentityError> {
        if !matches!(payload.key(), ExactTypeKey::Tuple(_)) {
            return Err(BoxedValueIdentityError::ExpectedTuple);
        }
        let generated_type = Self::generated_type(payload.id())?;
        let group = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: payload.id(),
        })
        .map_err(BoxedValueIdentityError::OdrGroup)?;
        let member = Self::odr_member(group.id(), generated_type.id())?;
        Self::finish(
            generated_type,
            BoxedValueRoot::Structural(Box::new(BoxedValueStructuralRoot { group, member })),
        )
    }

    fn build(
        payload: PersistentExactTypeId,
        root: BoxedValueRoot,
    ) -> Result<Self, BoxedValueIdentityError> {
        let generated_type = Self::generated_type(payload)?;
        Self::finish(generated_type, root)
    }

    fn generated_type(
        payload: PersistentExactTypeId,
    ) -> Result<GeneratedTypeRecord, BoxedValueIdentityError> {
        let generated_key = GeneratedNominalKey::BoxedValue { payload };
        CborIdentityRecord::from_key(generated_key).map_err(BoxedValueIdentityError::GeneratedType)
    }

    fn finish(
        generated_type: GeneratedTypeRecord,
        root: BoxedValueRoot,
    ) -> Result<Self, BoxedValueIdentityError> {
        let payload_field = CborIdentityRecord::from_key(
            FieldIdentityKey::box_payload(generated_type.key())
                .map_err(BoxedValueIdentityError::PayloadField)?,
        )
        .map_err(BoxedValueIdentityError::PayloadField)?;
        Ok(Self {
            generated_type,
            payload_field,
            root,
        })
    }

    fn odr_member(
        group: OdrGroupId,
        generated_type: PersistentTypeId,
    ) -> Result<OdrMemberRecord, BoxedValueIdentityError> {
        let member_key = OdrMemberKey::new(
            group,
            OdrMemberRole::GeneratedNominal,
            OdrMemberDiscriminator::GeneratedNominal(generated_type),
        )
        .map_err(BoxedValueIdentityError::OdrMember)?;
        CborIdentityRecord::from_key(member_key).map_err(BoxedValueIdentityError::OdrMemberRecord)
    }

    pub const fn generated_type_record(&self) -> &GeneratedTypeRecord {
        &self.generated_type
    }

    pub const fn payload_field_record(&self) -> &FieldRecord {
        &self.payload_field
    }

    pub const fn root(&self) -> &BoxedValueRoot {
        &self.root
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoxedValueIdentityError {
    ExpectedSourceNominal,
    ExpectedNominalApplication,
    ExpectedTuple,
    NominalGroupMismatch,
    GeneratedType(GeneratedNominalIdentityError),
    PayloadField(FieldIdentityError),
    OdrGroup(HashError),
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
}

impl fmt::Display for BoxedValueIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedSourceNominal => {
                formatter.write_str("boxed payload is not a parameter-free source nominal")
            }
            Self::ExpectedNominalApplication => {
                formatter.write_str("boxed payload is not a nominal application")
            }
            Self::ExpectedTuple => formatter.write_str("boxed payload is not a tuple"),
            Self::NominalGroupMismatch => {
                formatter.write_str("boxed nominal group does not match the exact payload")
            }
            Self::GeneratedType(error) => error.fmt(formatter),
            Self::PayloadField(error) => error.fmt(formatter),
            Self::OdrGroup(error) | Self::OdrMemberRecord(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BoxedValueIdentityError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
        DefinitionOwnerChain, NonEmptyVec, PackagePath, PersistentGenericTypeId,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };

    use super::*;

    fn unit() -> ExactTypeRecord {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
    }

    fn generic_origin() -> PersistentGenericTypeId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        PersistentGenericTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
            site,
            CanonicalIdentifier::new("Box").unwrap(),
            SourceNominalKind::Struct,
            1,
        ))
        .unwrap()
    }

    #[test]
    fn source_nominal_box_has_a_complete_strong_identity_bundle() {
        let payload = unit();
        let identity = BoxedValueIdentity::for_source_nominal(&payload).unwrap();
        assert_eq!(
            identity.generated_type_record().key(),
            &GeneratedNominalKey::BoxedValue {
                payload: payload.id()
            }
        );
        assert_eq!(
            identity.payload_field_record().key(),
            &FieldIdentityKey::box_payload(identity.generated_type_record().key()).unwrap()
        );
        assert_eq!(
            identity.root(),
            &BoxedValueRoot::SourceNominal(CoreBuiltinNominal::Unit.identity_record().id())
        );
    }

    #[test]
    fn nominal_application_box_reuses_its_nominal_group() {
        let argument = unit().id();
        let origin = generic_origin();
        let arguments = NonEmptyVec::from_first(argument, []);
        let payload = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
            origin,
            arguments: arguments.clone(),
        })
        .unwrap();
        let group = CborIdentityRecord::from_key(SpecializationKey::Nominal {
            origin,
            arguments: arguments.clone(),
        })
        .unwrap();
        let identity = BoxedValueIdentity::for_nominal_application(&payload, &group).unwrap();
        let BoxedValueRoot::NominalApplication(root) = identity.root() else {
            panic!("nominal applications are ODR-owned")
        };
        assert_eq!(root.group(), group.id());
        assert_eq!(root.member_record().key().group(), group.id());
        assert_eq!(
            root.member_record().key().discriminator(),
            &OdrMemberDiscriminator::GeneratedNominal(identity.generated_type_record().id())
        );
    }

    #[test]
    fn tuple_box_creates_a_structural_group() {
        let payload = CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(
            unit().id(),
            [],
        )))
        .unwrap();
        let identity = BoxedValueIdentity::for_tuple(&payload).unwrap();
        let BoxedValueRoot::Structural(root) = identity.root() else {
            panic!("tuples are ODR-owned")
        };
        assert_eq!(
            root.group_record().key(),
            &SpecializationKey::StructuralType {
                exact_type: payload.id()
            }
        );
    }

    #[test]
    fn function_payload_is_rejected_instead_of_getting_a_box_identity() {
        let payload = CborIdentityRecord::from_key(ExactTypeKey::Function {
            effect: scoop_identity::Effect::Ordinary,
            parameters: Vec::new(),
            result: unit().id(),
        })
        .unwrap();
        assert_eq!(
            BoxedValueIdentity::for_source_nominal(&payload),
            Err(BoxedValueIdentityError::ExpectedSourceNominal)
        );
    }

    #[test]
    fn nominal_application_rejects_a_different_hir_group() {
        let argument = unit().id();
        let origin = generic_origin();
        let payload = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
            origin,
            arguments: NonEmptyVec::from_first(argument, []),
        })
        .unwrap();
        let wrong = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: payload.id(),
        })
        .unwrap();
        assert_eq!(
            BoxedValueIdentity::for_nominal_application(&payload, &wrong),
            Err(BoxedValueIdentityError::NominalGroupMismatch)
        );
    }
}
