use scoop_identity::{
    CborIdentityRecord, ExactTypeKey, OdrGroupId, PersistentExactTypeId, SpecializationKey,
};

use crate::Type;

pub type SourceExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
pub type SourceNominalSpecializationRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;

/// The canonical materialization root class of a source exact type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceExactTypeOwner {
    ConeOwned,
    NominalApplication(OdrGroupId),
    Structural,
}

/// One exact LocalConcrete HIR type after structural transposition into MIR.
///
/// The persistent identity remains HIR-owned. MIR retains this relation so
/// later transforms never reconstruct an exact source type from a display
/// name, nominal arena id, or a transform-local cache.
#[derive(Clone, Debug)]
pub struct SourceExactTypeIdentity {
    ty: Type,
    identity: SourceExactTypeRecord,
    nominal_specialization: Option<SourceNominalSpecializationRecord>,
}

impl SourceExactTypeIdentity {
    pub fn checked(
        ty: Type,
        identity: SourceExactTypeRecord,
        nominal_specialization: Option<SourceNominalSpecializationRecord>,
    ) -> Result<Self, SourceExactTypeIdentityError> {
        match (identity.key(), nominal_specialization.as_ref()) {
            (ExactTypeKey::NominalApplication { origin, arguments }, Some(specialization))
                if specialization.key()
                    == &(SpecializationKey::Nominal {
                        origin: *origin,
                        arguments: arguments.clone(),
                    }) => {}
            (ExactTypeKey::NominalApplication { .. }, None) => {
                return Err(SourceExactTypeIdentityError::MissingNominalSpecialization);
            }
            (ExactTypeKey::NominalApplication { .. }, Some(_)) => {
                return Err(SourceExactTypeIdentityError::InvalidNominalSpecialization);
            }
            (_, Some(_)) => {
                return Err(SourceExactTypeIdentityError::UnexpectedNominalSpecialization);
            }
            (_, None) => {}
        }
        Ok(Self {
            ty,
            identity,
            nominal_specialization,
        })
    }

    pub const fn ty(&self) -> &Type {
        &self.ty
    }

    pub const fn identity_record(&self) -> &SourceExactTypeRecord {
        &self.identity
    }

    pub const fn nominal_specialization(&self) -> Option<&SourceNominalSpecializationRecord> {
        self.nominal_specialization.as_ref()
    }

    pub fn owner(&self) -> SourceExactTypeOwner {
        match self.identity.key() {
            ExactTypeKey::Nominal(_) => SourceExactTypeOwner::ConeOwned,
            ExactTypeKey::NominalApplication { .. } => SourceExactTypeOwner::NominalApplication(
                self.nominal_specialization
                    .as_ref()
                    .expect("a checked nominal application retains its ODR group")
                    .id(),
            ),
            ExactTypeKey::Tuple(_)
            | ExactTypeKey::Function { .. }
            | ExactTypeKey::RawPointer(_)
            | ExactTypeKey::NativeFunctionPointer { .. } => SourceExactTypeOwner::Structural,
        }
    }
}

/// Complete one-to-one relation for LocalConcrete HIR exact types that occur
/// in this MIR module.
#[derive(Clone, Debug, Default)]
pub struct SourceExactTypeIdentities {
    entries: Vec<SourceExactTypeIdentity>,
}

impl SourceExactTypeIdentities {
    pub fn checked(
        entries: Vec<SourceExactTypeIdentity>,
    ) -> Result<Self, SourceExactTypeRelationError> {
        for (index, entry) in entries.iter().enumerate() {
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.ty == entry.ty)
            {
                return Err(SourceExactTypeRelationError::DuplicateType { first, index });
            }
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.identity.id() == entry.identity.id())
            {
                return Err(SourceExactTypeRelationError::DuplicateIdentity { first, index });
            }
        }
        Ok(Self { entries })
    }

    pub fn get(&self, ty: &Type) -> Option<&SourceExactTypeIdentity> {
        self.entries.iter().find(|entry| entry.ty() == ty)
    }

    pub fn get_by_identity(
        &self,
        identity: PersistentExactTypeId,
    ) -> Option<&SourceExactTypeIdentity> {
        self.entries
            .iter()
            .find(|entry| entry.identity_record().id() == identity)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SourceExactTypeIdentity> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceExactTypeIdentityError {
    MissingNominalSpecialization,
    InvalidNominalSpecialization,
    UnexpectedNominalSpecialization,
}

impl std::fmt::Display for SourceExactTypeIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingNominalSpecialization => {
                formatter.write_str("nominal application is missing its specialization group")
            }
            Self::InvalidNominalSpecialization => formatter.write_str(
                "nominal specialization group does not match the exact type application",
            ),
            Self::UnexpectedNominalSpecialization => formatter
                .write_str("a non-application exact type cannot have a nominal specialization"),
        }
    }
}

impl std::error::Error for SourceExactTypeIdentityError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceExactTypeRelationError {
    DuplicateType { first: usize, index: usize },
    DuplicateIdentity { first: usize, index: usize },
}

impl std::fmt::Display for SourceExactTypeRelationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateType { first, index } => write!(
                formatter,
                "source exact type entries {first} and {index} have the same MIR type"
            ),
            Self::DuplicateIdentity { first, index } => write!(
                formatter,
                "source exact type entries {first} and {index} have the same persistent identity"
            ),
        }
    }
}

impl std::error::Error for SourceExactTypeRelationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
        DefinitionOwnerChain, NonEmptyVec, PackagePath, PersistentGenericTypeId,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };

    use super::*;

    fn unit() -> SourceExactTypeRecord {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
    }

    fn application() -> (SourceExactTypeRecord, SourceNominalSpecializationRecord) {
        let declaration = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Box").unwrap(),
            SourceNominalKind::Class,
            1,
        );
        let origin = PersistentGenericTypeId::from_source_declaration(&declaration).unwrap();
        let arguments = NonEmptyVec::from_first(unit().id(), []);
        let exact = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
            origin,
            arguments: arguments.clone(),
        })
        .unwrap();
        let specialization =
            CborIdentityRecord::from_key(SpecializationKey::Nominal { origin, arguments }).unwrap();
        (exact, specialization)
    }

    #[test]
    fn nominal_application_keeps_its_exact_specialization_group() {
        let (exact, specialization) = application();
        let identity = SourceExactTypeIdentity::checked(
            Type::Class(crate::ClassId::from_raw(3_u32.into())),
            exact.clone(),
            Some(specialization.clone()),
        )
        .unwrap();
        let relation = SourceExactTypeIdentities::checked(vec![identity]).unwrap();

        let found = relation.get_by_identity(exact.id()).unwrap();
        assert_eq!(found.identity_record().id(), exact.id());
        assert_eq!(
            found.nominal_specialization().unwrap().id(),
            specialization.id()
        );
    }

    #[test]
    fn nominal_application_requires_its_specialization_group() {
        let (exact, _) = application();
        assert_eq!(
            SourceExactTypeIdentity::checked(Type::Unit, exact, None).unwrap_err(),
            SourceExactTypeIdentityError::MissingNominalSpecialization
        );
    }

    #[test]
    fn relation_rejects_duplicate_mir_types_and_exact_identities() {
        let exact = unit();
        let first = SourceExactTypeIdentity::checked(Type::Unit, exact.clone(), None).unwrap();
        let same_type = SourceExactTypeIdentity::checked(
            Type::Unit,
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(
                CoreBuiltinNominal::Any.identity_record().id(),
            ))
            .unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(
            SourceExactTypeIdentities::checked(vec![first.clone(), same_type]).unwrap_err(),
            SourceExactTypeRelationError::DuplicateType { first: 0, index: 1 }
        );

        let same_identity = SourceExactTypeIdentity::checked(Type::Boolean, exact, None).unwrap();
        assert_eq!(
            SourceExactTypeIdentities::checked(vec![first, same_identity]).unwrap_err(),
            SourceExactTypeRelationError::DuplicateIdentity { first: 0, index: 1 }
        );
    }
}
