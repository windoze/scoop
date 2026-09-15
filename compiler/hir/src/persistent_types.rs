//! Persistent identities for closed HIR types and explicit binder sets for
//! open HIR types.

use std::ops::Index;

use la_arena::Arena;
use scoop_identity::{CborIdentityRecord, ExactTypeKey, PersistentExactTypeId};

use crate::{
    ClassApplication, ClassDecl, EnumApplication, EnumDecl, FunctionType, HirNominalIdentities,
    InterfaceApplication, InterfaceDecl, ObjectDecl, StructApplication, StructDecl, Type, TypeId,
    TypeParamId,
};

mod builder;
mod error;
mod signature;
pub use error::{HirTypeIdentityError, HirTypeRelation};
pub use signature::{HirSignatureBinder, HirSignatureTypeMapper, HirSignatureTypeMappingError};

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOpenTypeIdentity {
    parameters: Vec<TypeParamId>,
}

impl HirOpenTypeIdentity {
    pub fn parameters(&self) -> &[TypeParamId] {
        &self.parameters
    }
}

/// Every HIR type is either closed with one persistent exact identity or
/// explicitly open over a non-empty set of source type parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirTypeIdentity {
    Exact(ExactTypeRecord),
    Open(HirOpenTypeIdentity),
}

impl HirTypeIdentity {
    pub const fn exact(&self) -> Option<&ExactTypeRecord> {
        match self {
            Self::Exact(record) => Some(record),
            Self::Open(_) => None,
        }
    }

    pub const fn open(&self) -> Option<&HirOpenTypeIdentity> {
        match self {
            Self::Exact(_) => None,
            Self::Open(open) => Some(open),
        }
    }
}

#[derive(Clone, Copy)]
pub struct HirTypeIdentityInputs<'a> {
    pub types: &'a Arena<Type>,
    pub function_types: &'a Arena<FunctionType>,
    pub structs: &'a Arena<StructDecl>,
    pub struct_applications: &'a Arena<StructApplication>,
    pub enums: &'a Arena<EnumDecl>,
    pub enum_applications: &'a Arena<EnumApplication>,
    pub classes: &'a Arena<ClassDecl>,
    pub class_applications: &'a Arena<ClassApplication>,
    pub interfaces: &'a Arena<InterfaceDecl>,
    pub interface_applications: &'a Arena<InterfaceApplication>,
    pub objects: &'a Arena<ObjectDecl>,
    pub core_types: HirCoreTypeIdentityAuthority<'a>,
    pub nominal_identities: &'a HirNominalIdentities,
}

/// Origin-refined identity authority for compiler-represented fundamental
/// types. Ordinary HIR resolves these owners from the trusted core artifact;
/// only the defining core graph may use local nominal declarations.
#[derive(Clone, Copy)]
pub enum HirCoreTypeIdentityAuthority<'a> {
    Defined(&'a crate::IntrinsicTypeCore),
    Imported(&'a crate::ImportedCoreFundamentalTypeProtocol),
}

/// Total identity relation aligned with the HIR type arena.
#[derive(Clone, Debug)]
pub struct HirTypeIdentities {
    identities: Vec<HirTypeIdentity>,
}

impl HirTypeIdentities {
    pub fn from_types(inputs: HirTypeIdentityInputs<'_>) -> Result<Self, HirTypeIdentityError> {
        builder::build(inputs)
    }

    pub fn get(&self, id: TypeId) -> Option<&HirTypeIdentity> {
        self.identities.get(id.into_raw().into_u32() as usize)
    }
}

impl Index<TypeId> for HirTypeIdentities {
    type Output = HirTypeIdentity;

    fn index(&self, id: TypeId) -> &Self::Output {
        &self.identities[id.into_raw().into_u32() as usize]
    }
}
