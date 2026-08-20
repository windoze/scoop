use scoop_identity::{
    CborIdentityRecord, ConeIdentity, ExactTypeKey, OdrGroupId, PersistentExactTypeId,
    SpecializationKey,
};

use crate::Type;

pub type SourceExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
pub type SourceNominalSpecializationRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;

/// The canonical materialization root class of a source exact type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceExactTypeOwner {
    Cone(ConeIdentity),
    NominalApplication(OdrGroupId),
    Structural,
}

/// Complete provenance for an exact source type. A nominal provider and an
/// application specialization cannot be omitted or confused with each other.
#[derive(Clone, Debug)]
pub enum SourceExactTypeOrigin {
    Nominal(ConeIdentity),
    NominalApplication(SourceNominalSpecializationRecord),
    Structural,
}

/// One exact LocalConcrete HIR type after structural transposition into MIR.
/// Its HIR-owned identity and materialization provenance travel together.
#[derive(Clone, Debug)]
pub struct SourceExactTypeIdentity {
    ty: Type,
    identity: SourceExactTypeRecord,
    origin: SourceExactTypeOrigin,
}

impl SourceExactTypeIdentity {
    pub fn checked(
        ty: Type,
        identity: SourceExactTypeRecord,
        origin: SourceExactTypeOrigin,
    ) -> Result<Self, SourceExactTypeIdentityError> {
        match (identity.key(), &origin) {
            (ExactTypeKey::Nominal(_), SourceExactTypeOrigin::Nominal(_)) => {}
            (
                ExactTypeKey::NominalApplication { origin, arguments },
                SourceExactTypeOrigin::NominalApplication(specialization),
            ) => {
                if specialization.key()
                    != &(SpecializationKey::Nominal {
                        origin: *origin,
                        arguments: arguments.clone(),
                    })
                {
                    return Err(SourceExactTypeIdentityError::InvalidNominalSpecialization);
                }
            }
            (
                ExactTypeKey::Tuple(_)
                | ExactTypeKey::Function { .. }
                | ExactTypeKey::RawPointer(_)
                | ExactTypeKey::NativeFunctionPointer { .. },
                SourceExactTypeOrigin::Structural,
            ) => {}
            _ => return Err(SourceExactTypeIdentityError::OriginKindMismatch),
        }
        Ok(Self {
            ty,
            identity,
            origin,
        })
    }

    pub const fn ty(&self) -> &Type {
        &self.ty
    }

    pub const fn identity_record(&self) -> &SourceExactTypeRecord {
        &self.identity
    }

    pub const fn nominal_specialization(&self) -> Option<&SourceNominalSpecializationRecord> {
        match &self.origin {
            SourceExactTypeOrigin::NominalApplication(record) => Some(record),
            SourceExactTypeOrigin::Nominal(_) | SourceExactTypeOrigin::Structural => None,
        }
    }

    pub const fn owner(&self) -> SourceExactTypeOwner {
        match &self.origin {
            SourceExactTypeOrigin::Nominal(provider) => SourceExactTypeOwner::Cone(*provider),
            SourceExactTypeOrigin::NominalApplication(record) => {
                SourceExactTypeOwner::NominalApplication(record.id())
            }
            SourceExactTypeOrigin::Structural => SourceExactTypeOwner::Structural,
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
    OriginKindMismatch,
    InvalidNominalSpecialization,
}

impl std::fmt::Display for SourceExactTypeIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OriginKindMismatch => {
                formatter.write_str("source exact provenance does not match its identity kind")
            }
            Self::InvalidNominalSpecialization => formatter.write_str(
                "nominal specialization group does not match the exact type application",
            ),
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
mod tests;
