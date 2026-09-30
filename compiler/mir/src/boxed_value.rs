//! Persistent identity bundle for MIR-generated boxed value types.

use std::fmt;

use scoop_identity::{
    CborIdentityRecord, ExactTypeKey, FieldIdentityError, FieldIdentityKey,
    GeneratedNominalIdentityError, GeneratedNominalKey, OdrGroupId, OdrMemberDiscriminator,
    OdrMemberRole, PersistentExactTypeId, PersistentFieldId, PersistentTypeId, SpecializationKey,
};

use crate::{ExactOwnerRoot, ExactOwnerRootError};

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;
type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;

/// Complete persistent identity projection for one generated value box.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxedValueIdentity {
    generated_type: GeneratedTypeRecord,
    payload_field: FieldRecord,
    root: ExactOwnerRoot,
}

impl BoxedValueIdentity {
    pub fn for_source_nominal(payload: &ExactTypeRecord) -> Result<Self, BoxedValueIdentityError> {
        let generated_type = Self::generated_type(payload.id())?;
        let root =
            ExactOwnerRoot::source_nominal(payload).map_err(BoxedValueIdentityError::Root)?;
        Self::finish(generated_type, root)
    }

    pub fn for_nominal_application(
        payload: &ExactTypeRecord,
        group: &OdrGroupRecord,
    ) -> Result<Self, BoxedValueIdentityError> {
        let generated_type = Self::generated_type(payload.id())?;
        let root = ExactOwnerRoot::nominal_application(
            payload,
            group,
            OdrMemberRole::GeneratedNominal,
            OdrMemberDiscriminator::GeneratedNominal(generated_type.id()),
        )
        .map_err(BoxedValueIdentityError::Root)?;
        Self::finish(generated_type, root)
    }

    pub fn for_structural(payload: &ExactTypeRecord) -> Result<Self, BoxedValueIdentityError> {
        if !matches!(
            payload.key(),
            ExactTypeKey::Tuple(_)
                | ExactTypeKey::RawPointer(_)
                | ExactTypeKey::NativeFunctionPointer { .. }
        ) {
            return Err(BoxedValueIdentityError::ExpectedStructuralValue);
        }
        let generated_type = Self::generated_type(payload.id())?;
        let root = ExactOwnerRoot::structural(
            payload,
            OdrMemberRole::GeneratedNominal,
            OdrMemberDiscriminator::GeneratedNominal(generated_type.id()),
        )
        .map_err(BoxedValueIdentityError::Root)?;
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
        root: ExactOwnerRoot,
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

    pub const fn generated_type_record(&self) -> &GeneratedTypeRecord {
        &self.generated_type
    }

    pub const fn payload_field_record(&self) -> &FieldRecord {
        &self.payload_field
    }

    pub const fn root(&self) -> &ExactOwnerRoot {
        &self.root
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoxedValueIdentityError {
    ExpectedStructuralValue,
    GeneratedType(GeneratedNominalIdentityError),
    PayloadField(FieldIdentityError),
    Root(ExactOwnerRootError),
}

impl fmt::Display for BoxedValueIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedStructuralValue => {
                formatter.write_str("boxed payload is not a structural value")
            }
            Self::GeneratedType(error) => error.fmt(formatter),
            Self::PayloadField(error) => error.fmt(formatter),
            Self::Root(error) => error.fmt(formatter),
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
            &ExactOwnerRoot::SourceNominal(CoreBuiltinNominal::Unit.identity_record().id())
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
        let ExactOwnerRoot::NominalApplication(root) = identity.root() else {
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
        let identity = BoxedValueIdentity::for_structural(&payload).unwrap();
        let ExactOwnerRoot::Structural(root) = identity.root() else {
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
            Err(BoxedValueIdentityError::Root(
                ExactOwnerRootError::ExpectedSourceNominal
            ))
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
            Err(BoxedValueIdentityError::Root(
                ExactOwnerRootError::NominalGroupMismatch
            ))
        );
    }
}
