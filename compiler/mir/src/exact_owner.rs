//! Persistent materialization ownership shared by exact-type shape helpers.

use std::fmt;

use scoop_identity::{
    CborIdentityRecord, CoreBuiltinNominal, ExactTypeKey, OdrGroupId, OdrMemberDiscriminator,
    OdrMemberId, OdrMemberIdentityError, OdrMemberKey, OdrMemberRole, PersistentExactTypeId,
    PersistentTypeId, SpecializationKey,
};
use scoop_wire::HashError;

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// The unique materialization root selected by `ExactOwnerRoot(exact_type)`.
///
/// Parameter-free nominals belong to their definition Cone. Nominal
/// applications reuse their HIR specialization group, while structural exact
/// types introduce a structural group in the MIR identity delta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactOwnerRoot {
    SourceNominal(PersistentTypeId),
    NominalApplication(Box<NominalApplicationRoot>),
    Structural(Box<StructuralExactRoot>),
}

impl ExactOwnerRoot {
    pub fn for_member(
        exact: &ExactTypeRecord,
        nominal_group: Option<&OdrGroupRecord>,
        role: OdrMemberRole,
        discriminator: OdrMemberDiscriminator,
    ) -> Result<Self, ExactOwnerRootError> {
        match exact.key() {
            ExactTypeKey::Nominal(_) => {
                if nominal_group.is_some() {
                    return Err(ExactOwnerRootError::UnexpectedNominalGroup);
                }
                Self::source_nominal(exact)
            }
            ExactTypeKey::NominalApplication { .. } => Self::nominal_application(
                exact,
                nominal_group.ok_or(ExactOwnerRootError::MissingNominalGroup)?,
                role,
                discriminator,
            ),
            ExactTypeKey::Tuple(_)
            | ExactTypeKey::Function { .. }
            | ExactTypeKey::RawPointer(_)
            | ExactTypeKey::NativeFunctionPointer { .. } => {
                if nominal_group.is_some() {
                    return Err(ExactOwnerRootError::UnexpectedNominalGroup);
                }
                Self::structural(exact, role, discriminator)
            }
        }
    }

    pub fn source_nominal(exact: &ExactTypeRecord) -> Result<Self, ExactOwnerRootError> {
        let ExactTypeKey::Nominal(owner) = exact.key() else {
            return Err(ExactOwnerRootError::ExpectedSourceNominal);
        };
        Ok(Self::SourceNominal(*owner))
    }

    pub fn nominal_application(
        exact: &ExactTypeRecord,
        group: &OdrGroupRecord,
        role: OdrMemberRole,
        discriminator: OdrMemberDiscriminator,
    ) -> Result<Self, ExactOwnerRootError> {
        let ExactTypeKey::NominalApplication { origin, arguments } = exact.key() else {
            return Err(ExactOwnerRootError::ExpectedNominalApplication);
        };
        let expected = SpecializationKey::Nominal {
            origin: *origin,
            arguments: arguments.clone(),
        };
        if group.key() != &expected {
            return Err(ExactOwnerRootError::NominalGroupMismatch);
        }
        let member = member(group.id(), role, discriminator)?;
        Ok(Self::NominalApplication(Box::new(NominalApplicationRoot {
            group: group.id(),
            member,
        })))
    }

    pub fn structural(
        exact: &ExactTypeRecord,
        role: OdrMemberRole,
        discriminator: OdrMemberDiscriminator,
    ) -> Result<Self, ExactOwnerRootError> {
        // Builtin Unit equality can be requested independently in each Cone.
        let source_nominal = matches!(exact.key(), ExactTypeKey::Nominal(owner)
            if *owner != CoreBuiltinNominal::Unit.identity_record().id());
        if source_nominal || matches!(exact.key(), ExactTypeKey::NominalApplication { .. }) {
            return Err(ExactOwnerRootError::ExpectedStructuralType);
        }
        let group = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: exact.id(),
        })
        .map_err(ExactOwnerRootError::OdrGroup)?;
        let member = member(group.id(), role, discriminator)?;
        Ok(Self::Structural(Box::new(StructuralExactRoot {
            group,
            member,
        })))
    }

    pub const fn member_record(&self) -> Option<&OdrMemberRecord> {
        match self {
            Self::SourceNominal(_) => None,
            Self::NominalApplication(root) => Some(root.member_record()),
            Self::Structural(root) => Some(root.member_record()),
        }
    }

    /// The ODR group first created by MIR for this root. Nominal-application
    /// groups are HIR-owned and therefore deliberately excluded.
    pub const fn mir_odr_group_record(&self) -> Option<&OdrGroupRecord> {
        match self {
            Self::Structural(root) => Some(root.group_record()),
            Self::SourceNominal(_) | Self::NominalApplication(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalApplicationRoot {
    group: OdrGroupId,
    member: OdrMemberRecord,
}

impl NominalApplicationRoot {
    pub const fn group(&self) -> OdrGroupId {
        self.group
    }

    pub const fn member_record(&self) -> &OdrMemberRecord {
        &self.member
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralExactRoot {
    group: OdrGroupRecord,
    member: OdrMemberRecord,
}

impl StructuralExactRoot {
    pub const fn group_record(&self) -> &OdrGroupRecord {
        &self.group
    }

    pub const fn member_record(&self) -> &OdrMemberRecord {
        &self.member
    }
}

fn member(
    group: OdrGroupId,
    role: OdrMemberRole,
    discriminator: OdrMemberDiscriminator,
) -> Result<OdrMemberRecord, ExactOwnerRootError> {
    let key =
        OdrMemberKey::new(group, role, discriminator).map_err(ExactOwnerRootError::OdrMember)?;
    CborIdentityRecord::from_key(key).map_err(ExactOwnerRootError::OdrMemberRecord)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactOwnerRootError {
    ExpectedSourceNominal,
    ExpectedNominalApplication,
    ExpectedStructuralType,
    MissingNominalGroup,
    UnexpectedNominalGroup,
    NominalGroupMismatch,
    OdrGroup(HashError),
    OdrMember(OdrMemberIdentityError),
    OdrMemberRecord(HashError),
}

impl fmt::Display for ExactOwnerRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedSourceNominal => {
                formatter.write_str("exact owner is not a parameter-free source nominal")
            }
            Self::ExpectedNominalApplication => {
                formatter.write_str("exact owner is not a nominal application")
            }
            Self::ExpectedStructuralType => {
                formatter.write_str("exact owner is not a structural type")
            }
            Self::MissingNominalGroup => {
                formatter.write_str("nominal application is missing its specialization group")
            }
            Self::UnexpectedNominalGroup => {
                formatter.write_str("non-application exact owner has a nominal group")
            }
            Self::NominalGroupMismatch => {
                formatter.write_str("nominal specialization group does not match the exact owner")
            }
            Self::OdrGroup(error) | Self::OdrMemberRecord(error) => error.fmt(formatter),
            Self::OdrMember(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ExactOwnerRootError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
        DefinitionOwnerChain, Effect, NonEmptyVec, PackagePath, PersistentGenericTypeId,
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
            CanonicalIdentifier::new("Container").unwrap(),
            SourceNominalKind::Struct,
            1,
        ))
        .unwrap()
    }

    fn generated_member() -> OdrMemberDiscriminator {
        OdrMemberDiscriminator::GeneratedNominal(CoreBuiltinNominal::Any.identity_record().id())
    }

    #[test]
    fn source_nominal_has_no_odr_member() {
        let root = ExactOwnerRoot::source_nominal(&unit()).unwrap();
        assert_eq!(
            root,
            ExactOwnerRoot::SourceNominal(CoreBuiltinNominal::Unit.identity_record().id())
        );
        assert_eq!(root.member_record(), None);
        assert_eq!(root.mir_odr_group_record(), None);
    }

    #[test]
    fn nominal_application_reuses_the_matching_group() {
        let argument = unit().id();
        let origin = generic_origin();
        let arguments = NonEmptyVec::from_first(argument, []);
        let exact = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
            origin,
            arguments: arguments.clone(),
        })
        .unwrap();
        let group =
            CborIdentityRecord::from_key(SpecializationKey::Nominal { origin, arguments }).unwrap();
        let root = ExactOwnerRoot::nominal_application(
            &exact,
            &group,
            OdrMemberRole::GeneratedNominal,
            generated_member(),
        )
        .unwrap();
        assert_eq!(root.mir_odr_group_record(), None);
        let ExactOwnerRoot::NominalApplication(root) = root else {
            panic!("nominal applications use their existing ODR group")
        };
        assert_eq!(root.group(), group.id());
        assert_eq!(root.member_record().key().group(), group.id());
    }

    #[test]
    fn structural_shapes_create_their_exact_type_group() {
        let exact = CborIdentityRecord::from_key(ExactTypeKey::Function {
            effect: Effect::Ordinary,
            parameters: Vec::new(),
            result: unit().id(),
        })
        .unwrap();
        let root =
            ExactOwnerRoot::structural(&exact, OdrMemberRole::GeneratedNominal, generated_member())
                .unwrap();
        assert_eq!(
            root.mir_odr_group_record().map(CborIdentityRecord::id),
            Some(
                CborIdentityRecord::from_key(SpecializationKey::StructuralType {
                    exact_type: exact.id()
                })
                .unwrap()
                .id()
            )
        );
        let ExactOwnerRoot::Structural(root) = root else {
            panic!("function shapes use a structural ODR group")
        };
        assert_eq!(
            root.group_record().key(),
            &SpecializationKey::StructuralType {
                exact_type: exact.id()
            }
        );
    }

    #[test]
    fn nominal_applications_reject_unrelated_groups() {
        let arguments = NonEmptyVec::from_first(unit().id(), []);
        let exact = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
            origin: generic_origin(),
            arguments,
        })
        .unwrap();
        let wrong = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: exact.id(),
        })
        .unwrap();
        assert_eq!(
            ExactOwnerRoot::nominal_application(
                &exact,
                &wrong,
                OdrMemberRole::GeneratedNominal,
                generated_member(),
            ),
            Err(ExactOwnerRootError::NominalGroupMismatch)
        );
    }

    #[test]
    fn total_constructor_requires_exactly_the_applicable_group_kind() {
        assert_eq!(
            ExactOwnerRoot::for_member(
                &CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
                    origin: generic_origin(),
                    arguments: NonEmptyVec::from_first(unit().id(), []),
                })
                .unwrap(),
                None,
                OdrMemberRole::GeneratedNominal,
                generated_member(),
            ),
            Err(ExactOwnerRootError::MissingNominalGroup)
        );
        let unrelated = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: unit().id(),
        })
        .unwrap();
        assert_eq!(
            ExactOwnerRoot::for_member(
                &unit(),
                Some(&unrelated),
                OdrMemberRole::GeneratedNominal,
                generated_member(),
            ),
            Err(ExactOwnerRootError::UnexpectedNominalGroup)
        );
    }
}
