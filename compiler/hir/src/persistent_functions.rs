//! Total persistent identity relation for export HIR functions.

use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CborIdentityRecord, DefinitionOwnerAtom, GeneratedCallableKey, InitializationCallableRole,
    LexicalCallableParent, LexicalCallableRole, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, SourceDeclarationKey,
    StructuralDefinitionPath,
};

use crate::{
    AnonymousFunction, ClassConstructor, DerivedEqualityApplication, DerivedEqualityApplicationId,
    EnumDecl, Function, FunctionId, HirConstructorIdentities, HirEnumMemberIdentities,
    HirInitializationUnitIdentities, HirPropertyAccessorIdentities, HirTypeIdentities,
    InitializationUnit, InitializationUnitId, Lambda, LocalFunction, PropertyGetter,
    PropertyGetterId, PropertySetter, PropertySetterId, StructConstructor, StructDecl,
};

mod error;
mod validation;
pub use error::{FunctionIdentityRelation, HirFunctionIdentityError};

pub type HirPlainFunctionIdentity = CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>;
pub type HirGenericFunctionIdentity =
    CborIdentityRecord<PersistentGenericFunctionId, SourceDeclarationKey>;
pub type HirGeneratedFunctionIdentity =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;

/// Persistent identity of one source function declaration. Genericity is
/// determined by the declaration's own source binder count, not by inherited
/// binders carried by a lifted local function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSourceFunctionIdentity {
    Plain(HirPlainFunctionIdentity),
    Generic(HirGenericFunctionIdentity),
}

impl HirSourceFunctionIdentity {
    pub fn from_declaration(
        declaration: SourceDeclarationKey,
    ) -> Result<Self, HirFunctionIdentityError> {
        if declaration.duplicate_signature().type_parameter_count() == 0 {
            CborIdentityRecord::from_key(declaration)
                .map(Self::Plain)
                .map_err(HirFunctionIdentityError::SourceIdentity)
        } else {
            CborIdentityRecord::from_key(declaration)
                .map(Self::Generic)
                .map_err(HirFunctionIdentityError::SourceIdentity)
        }
    }

    pub fn declaration(&self) -> &SourceDeclarationKey {
        match self {
            Self::Plain(record) => record.key(),
            Self::Generic(record) => record.key(),
        }
    }

    pub const fn definition_owner(&self) -> DefinitionOwnerAtom {
        match self {
            Self::Plain(record) => DefinitionOwnerAtom::Function(record.id()),
            Self::Generic(record) => DefinitionOwnerAtom::GenericFunction(record.id()),
        }
    }

    pub const fn lexical_parent(&self) -> LexicalCallableParent {
        match self {
            Self::Plain(record) => LexicalCallableParent::function(record.id()),
            Self::Generic(record) => LexicalCallableParent::generic_function(record.id()),
        }
    }
}

/// Typed HIR accessor whose implementation is represented by a FunctionId.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirPropertyAccessorFunction {
    Getter(PropertyGetterId),
    Setter(PropertySetterId),
}

/// One exact application owned by a compiler-derived equality function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDerivedEqualityFunctionIdentity {
    application: DerivedEqualityApplicationId,
    record: HirGeneratedFunctionIdentity,
}

impl HirDerivedEqualityFunctionIdentity {
    pub fn new(
        application: DerivedEqualityApplicationId,
        exact_owner: scoop_identity::PersistentExactTypeId,
    ) -> Result<Self, HirFunctionIdentityError> {
        let record =
            CborIdentityRecord::from_key(GeneratedCallableKey::DerivedEquality { exact_owner })
                .map_err(HirFunctionIdentityError::GeneratedIdentity)?;
        Ok(Self {
            application,
            record,
        })
    }

    pub const fn application(&self) -> DerivedEqualityApplicationId {
        self.application
    }

    pub const fn record(&self) -> &HirGeneratedFunctionIdentity {
        &self.record
    }
}

/// Closed identity classification for one export HIR function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirFunctionIdentity {
    Source(HirSourceFunctionIdentity),
    PropertyAccessor(HirPropertyAccessorFunction),
    LexicalGenerated(HirGeneratedFunctionIdentity),
    Initialization {
        unit: InitializationUnitId,
        role: InitializationCallableRole,
        record: HirGeneratedFunctionIdentity,
    },
    DerivedEquality(Vec<HirDerivedEqualityFunctionIdentity>),
}

impl HirFunctionIdentity {
    pub const fn source(identity: HirSourceFunctionIdentity) -> Self {
        Self::Source(identity)
    }

    pub const fn property_accessor(accessor: HirPropertyAccessorFunction) -> Self {
        Self::PropertyAccessor(accessor)
    }

    pub fn lexical_generated(
        parent: LexicalCallableParent,
        role: LexicalCallableRole,
        path: StructuralDefinitionPath,
    ) -> Result<Self, HirFunctionIdentityError> {
        CborIdentityRecord::from_key(GeneratedCallableKey::Lexical { parent, role, path })
            .map(Self::LexicalGenerated)
            .map_err(HirFunctionIdentityError::GeneratedIdentity)
    }

    pub fn initialization(
        unit: InitializationUnitId,
        persistent_unit: scoop_identity::PersistentInitializationUnitId,
        role: InitializationCallableRole,
    ) -> Result<Self, HirFunctionIdentityError> {
        CborIdentityRecord::from_key(GeneratedCallableKey::Initialization {
            unit: persistent_unit,
            role,
        })
        .map(|record| Self::Initialization { unit, role, record })
        .map_err(HirFunctionIdentityError::GeneratedIdentity)
    }

    pub const fn derived_equality(applications: Vec<HirDerivedEqualityFunctionIdentity>) -> Self {
        Self::DerivedEquality(applications)
    }

    pub const fn source_identity(&self) -> Option<&HirSourceFunctionIdentity> {
        match self {
            Self::Source(identity) => Some(identity),
            _ => None,
        }
    }

    pub const fn property_accessor_identity(&self) -> Option<HirPropertyAccessorFunction> {
        match self {
            Self::PropertyAccessor(accessor) => Some(*accessor),
            _ => None,
        }
    }

    pub const fn generated_record(&self) -> Option<&HirGeneratedFunctionIdentity> {
        match self {
            Self::LexicalGenerated(record) | Self::Initialization { record, .. } => Some(record),
            Self::Source(_) | Self::PropertyAccessor(_) | Self::DerivedEquality(_) => None,
        }
    }

    pub fn derived_equality_applications(&self) -> Option<&[HirDerivedEqualityFunctionIdentity]> {
        match self {
            Self::DerivedEquality(applications) => Some(applications),
            _ => None,
        }
    }
}

pub struct HirFunctionIdentityInputs<'a> {
    pub functions: &'a Arena<Function>,
    pub lambdas: &'a Arena<Lambda>,
    pub anonymous_functions: &'a Arena<AnonymousFunction>,
    pub local_functions: &'a Arena<LocalFunction>,
    pub property_getters: &'a Arena<PropertyGetter>,
    pub property_setters: &'a Arena<PropertySetter>,
    pub property_accessor_identities: &'a HirPropertyAccessorIdentities,
    pub initialization_units: &'a Arena<InitializationUnit>,
    pub initialization_unit_identities: &'a HirInitializationUnitIdentities,
    pub derived_equality_applications: &'a Arena<DerivedEqualityApplication>,
    pub structs: &'a Arena<StructDecl>,
    pub enums: &'a Arena<EnumDecl>,
    pub type_identities: &'a HirTypeIdentities,
    pub struct_constructors: &'a Arena<StructConstructor>,
    pub class_constructors: &'a Arena<ClassConstructor>,
    pub constructor_identities: &'a HirConstructorIdentities,
    pub enum_member_identities: &'a HirEnumMemberIdentities,
}

/// Total persistent identity relation aligned with the Function arena.
#[derive(Clone, Debug)]
pub struct HirFunctionIdentities {
    identities: Vec<HirFunctionIdentity>,
}

impl HirFunctionIdentities {
    pub fn checked(
        inputs: HirFunctionIdentityInputs<'_>,
        identities: Vec<HirFunctionIdentity>,
    ) -> Result<Self, HirFunctionIdentityError> {
        validation::validate(&inputs, &identities)?;
        Ok(Self { identities })
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (FunctionId, &HirFunctionIdentity)> {
        self.identities
            .iter()
            .enumerate()
            .map(|(index, identity)| (FunctionId::from_raw((index as u32).into()), identity))
    }
}

impl Index<FunctionId> for HirFunctionIdentities {
    type Output = HirFunctionIdentity;

    fn index(&self, id: FunctionId) -> &Self::Output {
        &self.identities[local_index(id)]
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

#[cfg(test)]
mod tests;
