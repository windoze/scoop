//! Exact-type identities for MIR-generated nominal types.

use std::fmt;

use scoop_identity::{
    CborIdentityRecord, ExactTypeKey, GeneratedNominalKey, OdrMemberDiscriminator, OdrMemberId,
    OdrMemberKey, OdrMemberRole, PersistentExactTypeId, PersistentTypeId,
};
use scoop_wire::HashError;

use crate::{ClassId, ClosureClassId, EnumId};

pub type GeneratedNominalRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;
pub type GeneratedExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
pub type GeneratedExactTypeOdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;

/// The complete materialization owner of one MIR-generated nominal exact type.
///
/// A source-anchored generated nominal resolves to its defining Cone. An ODR-
/// owned generated nominal retains the exact `GeneratedNominal` member that
/// MIR already introduced, so later stages never infer its group from a
/// generated type id or a coincidentally related callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedExactTypeOwner {
    ConeOwned,
    OdrOwned(GeneratedExactTypeOdrMemberRecord),
}

impl GeneratedExactTypeOwner {
    pub const fn odr_member_record(&self) -> Option<&GeneratedExactTypeOdrMemberRecord> {
        match self {
            Self::ConeOwned => None,
            Self::OdrOwned(member) => Some(member),
        }
    }
}

/// Typed physical location of one MIR-generated nominal type.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneratedExactTypeLocation {
    Closure(ClosureClassId),
    Class(ClassId),
    Enum(EnumId),
}

/// Persistent nominal and exact-type identities for one generated MIR type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedExactTypeIdentity {
    location: GeneratedExactTypeLocation,
    nominal: GeneratedNominalRecord,
    exact: GeneratedExactTypeRecord,
    owner: GeneratedExactTypeOwner,
}

impl GeneratedExactTypeIdentity {
    pub fn new(
        location: GeneratedExactTypeLocation,
        nominal: &GeneratedNominalRecord,
        odr_member: Option<&GeneratedExactTypeOdrMemberRecord>,
    ) -> Result<Self, GeneratedExactTypeIdentityError> {
        let expected = match nominal.key() {
            GeneratedNominalKey::ClosureEnvironment { .. }
            | GeneratedNominalKey::CallableAdapterEnvironment { .. } => {
                GeneratedExactTypeKind::Closure
            }
            GeneratedNominalKey::CoroutineFrame { .. }
            | GeneratedNominalKey::ContinuationAdapterEnvironment { .. }
            | GeneratedNominalKey::BoxedValue { .. } => GeneratedExactTypeKind::Class,
            GeneratedNominalKey::CoroutineStep { .. }
            | GeneratedNominalKey::CoroutineSlot { .. } => GeneratedExactTypeKind::Enum,
            GeneratedNominalKey::ObjectBackingClass { .. } => {
                return Err(GeneratedExactTypeIdentityError::HirOwnedNominal);
            }
        };
        if GeneratedExactTypeKind::of(location) != expected {
            return Err(GeneratedExactTypeIdentityError::LocationKindMismatch);
        }
        let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal.id()))
            .map_err(GeneratedExactTypeIdentityError::ExactType)?;
        let owner = match odr_member {
            Some(member)
                if member.key().role() == OdrMemberRole::GeneratedNominal
                    && member.key().discriminator()
                        == &OdrMemberDiscriminator::GeneratedNominal(nominal.id()) =>
            {
                GeneratedExactTypeOwner::OdrOwned(member.clone())
            }
            Some(_) => return Err(GeneratedExactTypeIdentityError::InvalidOdrMember),
            None => GeneratedExactTypeOwner::ConeOwned,
        };
        Ok(Self {
            location,
            nominal: nominal.clone(),
            exact,
            owner,
        })
    }

    pub const fn location(&self) -> GeneratedExactTypeLocation {
        self.location
    }

    pub const fn nominal_record(&self) -> &GeneratedNominalRecord {
        &self.nominal
    }

    pub const fn exact_record(&self) -> &GeneratedExactTypeRecord {
        &self.exact
    }

    pub const fn owner(&self) -> &GeneratedExactTypeOwner {
        &self.owner
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneratedExactTypeKind {
    Closure,
    Class,
    Enum,
}

impl GeneratedExactTypeKind {
    const fn of(location: GeneratedExactTypeLocation) -> Self {
        match location {
            GeneratedExactTypeLocation::Closure(_) => Self::Closure,
            GeneratedExactTypeLocation::Class(_) => Self::Class,
            GeneratedExactTypeLocation::Enum(_) => Self::Enum,
        }
    }
}

/// Complete one-to-one relation for MIR-generated nominal exact types.
#[derive(Clone, Debug, Default)]
pub struct GeneratedExactTypeIdentities {
    entries: Vec<GeneratedExactTypeIdentity>,
}

impl GeneratedExactTypeIdentities {
    pub fn checked(
        mut entries: Vec<GeneratedExactTypeIdentity>,
    ) -> Result<Self, GeneratedExactTypeRelationError> {
        entries.sort_by_key(|entry| entry.exact.id());
        for (index, entry) in entries.iter().enumerate() {
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.location == entry.location)
            {
                return Err(GeneratedExactTypeRelationError::DuplicateLocation { first, index });
            }
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.nominal.id() == entry.nominal.id())
            {
                return Err(GeneratedExactTypeRelationError::DuplicateNominalIdentity {
                    first,
                    index,
                });
            }
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.exact.id() == entry.exact.id())
            {
                return Err(GeneratedExactTypeRelationError::DuplicateExactIdentity {
                    first,
                    index,
                });
            }
        }
        Ok(Self { entries })
    }

    pub fn get(&self, location: GeneratedExactTypeLocation) -> Option<&GeneratedExactTypeIdentity> {
        self.entries
            .iter()
            .find(|entry| entry.location() == location)
    }

    pub fn get_by_identity(
        &self,
        identity: PersistentExactTypeId,
    ) -> Option<&GeneratedExactTypeIdentity> {
        self.entries
            .iter()
            .find(|entry| entry.exact_record().id() == identity)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &GeneratedExactTypeIdentity> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedExactTypeIdentityError {
    HirOwnedNominal,
    LocationKindMismatch,
    InvalidOdrMember,
    ExactType(HashError),
}

impl fmt::Display for GeneratedExactTypeIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HirOwnedNominal => {
                formatter.write_str("object backing classes retain their HIR-owned exact identity")
            }
            Self::LocationKindMismatch => {
                formatter.write_str("generated nominal kind does not match its MIR location")
            }
            Self::InvalidOdrMember => formatter
                .write_str("generated nominal ODR owner does not identify the same generated type"),
            Self::ExactType(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedExactTypeIdentityError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedExactTypeRelationError {
    DuplicateLocation { first: usize, index: usize },
    DuplicateNominalIdentity { first: usize, index: usize },
    DuplicateExactIdentity { first: usize, index: usize },
}

impl fmt::Display for GeneratedExactTypeRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateLocation { first, index } => write!(
                formatter,
                "generated exact type entries {first} and {index} have the same MIR location"
            ),
            Self::DuplicateNominalIdentity { first, index } => write!(
                formatter,
                "generated exact type entries {first} and {index} have the same nominal identity"
            ),
            Self::DuplicateExactIdentity { first, index } => write!(
                formatter,
                "generated exact type entries {first} and {index} have the same exact identity"
            ),
        }
    }
}

impl std::error::Error for GeneratedExactTypeRelationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        CanonicalIdentifier, ClosureEnvironmentRole, ConeIdentity, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, PackagePath, PersistentFunctionId,
        SourceDeclarationKey, SourceDeclarationSite, SpecializationKey,
    };

    use super::*;

    fn source_function() -> PersistentFunctionId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new("generatedExactTypeOwner").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap()
    }

    fn core_exact(nominal: CoreBuiltinNominal) -> PersistentExactTypeId {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal.identity_record().id()))
            .unwrap()
            .id()
    }

    fn closure_nominal() -> GeneratedNominalRecord {
        CborIdentityRecord::from_key(GeneratedNominalKey::ClosureEnvironment {
            callable: CallableMaterialization::new(
                CallableTemplateOwner::Function(source_function()),
                CallableMaterializationContext::NoSubstitution,
            ),
            role: ClosureEnvironmentRole::Lambda,
        })
        .unwrap()
    }

    fn boxed_nominal() -> GeneratedNominalRecord {
        CborIdentityRecord::from_key(GeneratedNominalKey::BoxedValue {
            payload: core_exact(CoreBuiltinNominal::Unit),
        })
        .unwrap()
    }

    #[test]
    fn generated_nominal_has_its_exact_nominal_identity() {
        let nominal = boxed_nominal();
        let location = GeneratedExactTypeLocation::Class(ClassId::from_raw(3_u32.into()));
        let identity = GeneratedExactTypeIdentity::new(location, &nominal, None).unwrap();
        assert_eq!(identity.location(), location);
        assert_eq!(identity.nominal_record(), &nominal);
        assert_eq!(
            identity.exact_record().key(),
            &ExactTypeKey::Nominal(nominal.id())
        );
    }

    #[test]
    fn nominal_kind_must_match_its_physical_arena() {
        let nominal = closure_nominal();
        assert_eq!(
            GeneratedExactTypeIdentity::new(
                GeneratedExactTypeLocation::Class(ClassId::from_raw(0_u32.into())),
                &nominal,
                None,
            )
            .unwrap_err(),
            GeneratedExactTypeIdentityError::LocationKindMismatch
        );
    }

    #[test]
    fn generated_exact_type_retains_its_exact_odr_owner() {
        let nominal = boxed_nominal();
        let group = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: core_exact(CoreBuiltinNominal::Unit),
        })
        .unwrap();
        let member = CborIdentityRecord::from_key(
            OdrMemberKey::new(
                group.id(),
                OdrMemberRole::GeneratedNominal,
                OdrMemberDiscriminator::GeneratedNominal(nominal.id()),
            )
            .unwrap(),
        )
        .unwrap();

        let identity = GeneratedExactTypeIdentity::new(
            GeneratedExactTypeLocation::Class(ClassId::from_raw(0_u32.into())),
            &nominal,
            Some(&member),
        )
        .unwrap();

        assert_eq!(identity.owner().odr_member_record(), Some(&member));
    }

    #[test]
    fn generated_exact_type_rejects_another_nominal_odr_member() {
        let nominal = boxed_nominal();
        let other = closure_nominal();
        let group = CborIdentityRecord::from_key(SpecializationKey::StructuralType {
            exact_type: core_exact(CoreBuiltinNominal::Unit),
        })
        .unwrap();
        let member = CborIdentityRecord::from_key(
            OdrMemberKey::new(
                group.id(),
                OdrMemberRole::GeneratedNominal,
                OdrMemberDiscriminator::GeneratedNominal(other.id()),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            GeneratedExactTypeIdentity::new(
                GeneratedExactTypeLocation::Class(ClassId::from_raw(0_u32.into())),
                &nominal,
                Some(&member),
            )
            .unwrap_err(),
            GeneratedExactTypeIdentityError::InvalidOdrMember
        );
    }

    #[test]
    fn relation_rejects_duplicate_locations_and_identities() {
        let nominal = boxed_nominal();
        let first = GeneratedExactTypeIdentity::new(
            GeneratedExactTypeLocation::Class(ClassId::from_raw(0_u32.into())),
            &nominal,
            None,
        )
        .unwrap();
        let duplicate_location = GeneratedExactTypeIdentity::new(
            GeneratedExactTypeLocation::Class(ClassId::from_raw(0_u32.into())),
            &CborIdentityRecord::from_key(GeneratedNominalKey::BoxedValue {
                payload: core_exact(CoreBuiltinNominal::Any),
            })
            .unwrap(),
            None,
        )
        .unwrap();
        assert!(matches!(
            GeneratedExactTypeIdentities::checked(vec![first.clone(), duplicate_location]),
            Err(GeneratedExactTypeRelationError::DuplicateLocation { .. })
        ));

        let duplicate_identity = GeneratedExactTypeIdentity::new(
            GeneratedExactTypeLocation::Class(ClassId::from_raw(1_u32.into())),
            &nominal,
            None,
        )
        .unwrap();
        assert!(matches!(
            GeneratedExactTypeIdentities::checked(vec![first, duplicate_identity]),
            Err(GeneratedExactTypeRelationError::DuplicateNominalIdentity { .. })
        ));
    }
}
