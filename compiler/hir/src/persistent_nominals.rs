//! Persistent identities aligned with the export HIR nominal arenas.

use std::fmt;
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CborIdentityRecord, CoreBuiltinNominal, DefinitionOwnerAtom, GeneratedNominalIdentityError,
    GeneratedNominalKey, PersistentGenericTypeId, PersistentTypeId, SourceDeclarationIdentityError,
    SourceDeclarationKey,
};

use crate::{
    ClassDecl, ClassId, EnumDecl, EnumId, InterfaceDecl, InterfaceId, ObjectDecl, ObjectId,
    StructDecl, StructId,
};

/// A source nominal declaration has exactly one persistent identity kind,
/// selected by whether its declaration introduces type parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSourceNominalIdentity {
    Concrete(CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>),
    Generic(CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey>),
}

impl HirSourceNominalIdentity {
    pub fn from_declaration(
        declaration: SourceDeclarationKey,
    ) -> Result<Self, HirNominalIdentityError> {
        if declaration.duplicate_signature().type_parameter_count() == 0 {
            CborIdentityRecord::from_key(declaration)
                .map(Self::Concrete)
                .map_err(HirNominalIdentityError::Source)
        } else {
            CborIdentityRecord::from_key(declaration)
                .map(Self::Generic)
                .map_err(HirNominalIdentityError::Source)
        }
    }

    pub fn declaration(&self) -> &SourceDeclarationKey {
        match self {
            Self::Concrete(record) => record.key(),
            Self::Generic(record) => record.key(),
        }
    }

    pub const fn concrete_id(&self) -> Option<PersistentTypeId> {
        match self {
            Self::Concrete(record) => Some(record.id()),
            Self::Generic(_) => None,
        }
    }

    pub const fn generic_id(&self) -> Option<PersistentGenericTypeId> {
        match self {
            Self::Concrete(_) => None,
            Self::Generic(record) => Some(record.id()),
        }
    }

    pub const fn definition_owner(&self) -> DefinitionOwnerAtom {
        match self {
            Self::Concrete(record) => DefinitionOwnerAtom::Type(record.id()),
            Self::Generic(record) => DefinitionOwnerAtom::GenericType(record.id()),
        }
    }
}

/// Complete persistent nominal identity. Compiler-generated nominal entities
/// use their structural key and cannot be confused with source declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirNominalIdentity {
    Source(HirSourceNominalIdentity),
    Generated(CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>),
}

impl HirNominalIdentity {
    /// The original declaration/template identity used by semantic queries.
    pub const fn declaration_id(&self) -> crate::SourceNominalId {
        match self {
            Self::Source(HirSourceNominalIdentity::Concrete(record)) => {
                crate::SourceNominalId::Concrete(record.id())
            }
            Self::Generated(record) => crate::SourceNominalId::Concrete(record.id()),
            Self::Source(HirSourceNominalIdentity::Generic(record)) => {
                crate::SourceNominalId::GenericTemplate(record.id())
            }
        }
    }

    pub fn from_source_declaration(
        declaration: SourceDeclarationKey,
    ) -> Result<Self, HirNominalIdentityError> {
        HirSourceNominalIdentity::from_declaration(declaration).map(Self::Source)
    }

    pub fn from_generated_key(key: GeneratedNominalKey) -> Result<Self, HirNominalIdentityError> {
        CborIdentityRecord::from_key(key)
            .map(Self::Generated)
            .map_err(HirNominalIdentityError::Generated)
    }

    pub const fn source(&self) -> Option<&HirSourceNominalIdentity> {
        match self {
            Self::Source(identity) => Some(identity),
            Self::Generated(_) => None,
        }
    }

    pub const fn generated(
        &self,
    ) -> Option<&CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>> {
        match self {
            Self::Source(_) => None,
            Self::Generated(record) => Some(record),
        }
    }

    pub const fn concrete_type_id(&self) -> Option<PersistentTypeId> {
        match self {
            Self::Source(identity) => identity.concrete_id(),
            Self::Generated(record) => Some(record.id()),
        }
    }

    pub const fn generic_type_id(&self) -> Option<PersistentGenericTypeId> {
        match self {
            Self::Source(identity) => identity.generic_id(),
            Self::Generated(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirNominalIdentityError {
    Source(SourceDeclarationIdentityError),
    Generated(GeneratedNominalIdentityError),
}

impl fmt::Display for HirNominalIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Generated(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HirNominalIdentityError {}

/// Arena-aligned persistent identities for every export HIR nominal kind.
///
/// The compiler-owned Unit and Any identities are stored alongside the five
/// source/generated arena tables. Construction checks every table length
/// against its authoritative arena; indexing therefore cannot observe a
/// missing identity for a valid local id.
#[derive(Clone, Debug)]
pub struct HirNominalIdentities {
    unit: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    any: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    structs: Vec<HirNominalIdentity>,
    enums: Vec<HirNominalIdentity>,
    classes: Vec<HirNominalIdentity>,
    interfaces: Vec<HirNominalIdentity>,
    objects: Vec<HirNominalIdentity>,
    declarations: std::collections::HashMap<crate::SourceNominalId, crate::NominalOwner>,
}

impl HirNominalIdentities {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        structs: &Arena<StructDecl>,
        struct_identities: Vec<HirNominalIdentity>,
        enums: &Arena<EnumDecl>,
        enum_identities: Vec<HirNominalIdentity>,
        classes: &Arena<ClassDecl>,
        class_identities: Vec<HirNominalIdentity>,
        interfaces: &Arena<InterfaceDecl>,
        interface_identities: Vec<HirNominalIdentity>,
        objects: &Arena<ObjectDecl>,
        object_identities: Vec<HirNominalIdentity>,
    ) -> Result<Self, HirNominalIdentityTableError> {
        require_length(
            HirNominalIdentityTable::Struct,
            structs.len(),
            struct_identities.len(),
        )?;
        require_length(
            HirNominalIdentityTable::Enum,
            enums.len(),
            enum_identities.len(),
        )?;
        require_length(
            HirNominalIdentityTable::Class,
            classes.len(),
            class_identities.len(),
        )?;
        require_length(
            HirNominalIdentityTable::Interface,
            interfaces.len(),
            interface_identities.len(),
        )?;
        require_length(
            HirNominalIdentityTable::Object,
            objects.len(),
            object_identities.len(),
        )?;
        let mut declarations = std::collections::HashMap::new();
        for (id, identity) in structs.iter().map(|(id, _)| id).zip(&struct_identities) {
            declarations.insert(identity.declaration_id(), crate::NominalOwner::Struct(id));
        }
        for (id, identity) in enums.iter().map(|(id, _)| id).zip(&enum_identities) {
            declarations.insert(identity.declaration_id(), crate::NominalOwner::Enum(id));
        }
        for (id, identity) in classes.iter().map(|(id, _)| id).zip(&class_identities) {
            declarations.insert(identity.declaration_id(), crate::NominalOwner::Class(id));
        }
        for (id, identity) in interfaces
            .iter()
            .map(|(id, _)| id)
            .zip(&interface_identities)
        {
            declarations.insert(
                identity.declaration_id(),
                crate::NominalOwner::Interface(id),
            );
        }
        for (id, identity) in objects.iter().map(|(id, _)| id).zip(&object_identities) {
            declarations.insert(identity.declaration_id(), crate::NominalOwner::Object(id));
        }
        Ok(Self {
            unit: CoreBuiltinNominal::Unit.identity_record(),
            any: CoreBuiltinNominal::Any.identity_record(),
            structs: struct_identities,
            enums: enum_identities,
            classes: class_identities,
            interfaces: interface_identities,
            objects: object_identities,
            declarations,
        })
    }

    pub fn declaration(&self, identity: crate::SourceNominalId) -> Option<crate::NominalOwner> {
        self.declarations.get(&identity).copied()
    }

    pub fn struct_id(&self, identity: crate::SourceNominalId) -> Option<StructId> {
        match self.declaration(identity)? {
            crate::NominalOwner::Struct(id) => Some(id),
            _ => None,
        }
    }

    pub fn enum_id(&self, identity: crate::SourceNominalId) -> Option<EnumId> {
        match self.declaration(identity)? {
            crate::NominalOwner::Enum(id) => Some(id),
            _ => None,
        }
    }

    pub fn class_id(&self, identity: crate::SourceNominalId) -> Option<ClassId> {
        match self.declaration(identity)? {
            crate::NominalOwner::Class(id) => Some(id),
            _ => None,
        }
    }

    pub fn interface_id(&self, identity: crate::SourceNominalId) -> Option<InterfaceId> {
        match self.declaration(identity)? {
            crate::NominalOwner::Interface(id) => Some(id),
            _ => None,
        }
    }

    pub const fn core_builtin(
        &self,
        builtin: CoreBuiltinNominal,
    ) -> &CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
        match builtin {
            CoreBuiltinNominal::Unit => &self.unit,
            CoreBuiltinNominal::Any => &self.any,
        }
    }

    pub(crate) fn get_struct(&self, id: StructId) -> Option<&HirNominalIdentity> {
        self.structs.get(local_index(id))
    }

    pub(crate) fn get_enum(&self, id: EnumId) -> Option<&HirNominalIdentity> {
        self.enums.get(local_index(id))
    }

    pub(crate) fn get_class(&self, id: ClassId) -> Option<&HirNominalIdentity> {
        self.classes.get(local_index(id))
    }

    pub(crate) fn get_interface(&self, id: InterfaceId) -> Option<&HirNominalIdentity> {
        self.interfaces.get(local_index(id))
    }

    pub(crate) fn get_object(&self, id: ObjectId) -> Option<&HirNominalIdentity> {
        self.objects.get(local_index(id))
    }
}

macro_rules! nominal_index {
    ($id:ty, $field:ident) => {
        impl Index<$id> for HirNominalIdentities {
            type Output = HirNominalIdentity;

            fn index(&self, id: $id) -> &Self::Output {
                &self.$field[local_index(id)]
            }
        }
    };
}

nominal_index!(StructId, structs);
nominal_index!(EnumId, enums);
nominal_index!(ClassId, classes);
nominal_index!(InterfaceId, interfaces);
nominal_index!(ObjectId, objects);

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirNominalIdentityTable {
    Struct,
    Enum,
    Class,
    Interface,
    Object,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirNominalIdentityTableError {
    table: HirNominalIdentityTable,
    expected: usize,
    actual: usize,
}

impl HirNominalIdentityTableError {
    pub const fn table(self) -> HirNominalIdentityTable {
        self.table
    }

    pub const fn expected(self) -> usize {
        self.expected
    }

    pub const fn actual(self) -> usize {
        self.actual
    }
}

impl fmt::Display for HirNominalIdentityTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} identity table has {} entries, expected {}",
            self.table, self.actual, self.expected
        )
    }
}

impl std::error::Error for HirNominalIdentityTableError {}

fn require_length(
    table: HirNominalIdentityTable,
    expected: usize,
    actual: usize,
) -> Result<(), HirNominalIdentityTableError> {
    if expected == actual {
        Ok(())
    } else {
        Err(HirNominalIdentityTableError {
            table,
            expected,
            actual,
        })
    }
}

#[cfg(test)]
mod tests;
