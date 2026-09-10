//! Persistent identities aligned with the export HIR constructor arenas.

use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CborIdentityRecord, GeneratedCallableKey, PersistentConstructorId,
    PersistentGeneratedCallableId, SourceDeclarationKey,
};

use crate::{
    ClassConstructor, ClassConstructorApplication, ClassConstructorId, HirTypeIdentityInputs,
    StructConstructor, StructConstructorId,
};

mod error;
mod source;
mod validation;
pub use error::{ConstructorIdentityTable, HirConstructorIdentityError};
pub use source::{HirSourceConstructorIdentityError, derive_source_constructor_identity};

pub type HirSourceConstructorIdentity =
    CborIdentityRecord<PersistentConstructorId, SourceDeclarationKey>;
pub type HirGeneratedConstructorIdentity =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirClassConstructorIdentity {
    Source(HirSourceConstructorIdentity),
    ZeroArgumentAdapter {
        source: ClassConstructorId,
        record: HirGeneratedConstructorIdentity,
    },
}

impl HirClassConstructorIdentity {
    pub fn source(declaration: SourceDeclarationKey) -> Result<Self, HirConstructorIdentityError> {
        CborIdentityRecord::from_key(declaration)
            .map(Self::Source)
            .map_err(HirConstructorIdentityError::SourceIdentity)
    }

    pub fn zero_argument_adapter(
        source: ClassConstructorId,
        constructor: PersistentConstructorId,
    ) -> Result<Self, HirConstructorIdentityError> {
        CborIdentityRecord::from_key(GeneratedCallableKey::ZeroArgumentConstructorAdapter {
            constructor,
        })
        .map(|record| Self::ZeroArgumentAdapter { source, record })
        .map_err(HirConstructorIdentityError::GeneratedIdentity)
    }

    pub const fn source_record(&self) -> Option<&HirSourceConstructorIdentity> {
        match self {
            Self::Source(record) => Some(record),
            Self::ZeroArgumentAdapter { .. } => None,
        }
    }

    pub const fn generated_record(&self) -> Option<&HirGeneratedConstructorIdentity> {
        match self {
            Self::Source(_) => None,
            Self::ZeroArgumentAdapter { record, .. } => Some(record),
        }
    }
}

pub struct HirConstructorIdentityInputs<'a> {
    pub type_inputs: HirTypeIdentityInputs<'a>,
    pub struct_constructors: &'a Arena<StructConstructor>,
    pub class_constructors: &'a Arena<ClassConstructor>,
    pub class_constructor_applications: &'a Arena<ClassConstructorApplication>,
}

/// Total persistent identity relation for both export constructor arenas.
#[derive(Clone, Debug)]
pub struct HirConstructorIdentities {
    structs: Vec<HirSourceConstructorIdentity>,
    classes: Vec<HirClassConstructorIdentity>,
}

impl HirConstructorIdentities {
    pub fn checked(
        inputs: HirConstructorIdentityInputs<'_>,
        structs: Vec<HirSourceConstructorIdentity>,
        classes: Vec<HirClassConstructorIdentity>,
    ) -> Result<Self, HirConstructorIdentityError> {
        validation::validate(&inputs, &structs, &classes)?;
        Ok(Self { structs, classes })
    }
}

impl Index<StructConstructorId> for HirConstructorIdentities {
    type Output = HirSourceConstructorIdentity;

    fn index(&self, id: StructConstructorId) -> &Self::Output {
        &self.structs[local_index(id)]
    }
}

impl Index<ClassConstructorId> for HirConstructorIdentities {
    type Output = HirClassConstructorIdentity;

    fn index(&self, id: ClassConstructorId) -> &Self::Output {
        &self.classes[local_index(id)]
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
