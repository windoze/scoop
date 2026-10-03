use super::*;

mod queries;

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub access: NominalAccess,
    pub definition: StructDefinition,
    pub constructors: Vec<StructConstructorId>,
    /// Member declarations in source order. Consumers follow this typed
    /// relation and never recover ownership by scanning `Module::functions`.
    pub methods: Vec<FunctionId>,
    pub properties: Vec<PropertyId>,
    /// Compiler-derived same-type equality declaration, when no explicit
    /// same-signature operator suppresses derivation. Applicability remains
    /// conditional on this application's field obligations.
    pub derived_equality: Option<FunctionId>,
    pub span: Span,
}

/// Checked fields and conformance types in the declaration's binder domain.
/// Applications retain the original declaration and substitute these fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDefinition {
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
    pub attributes: StructAttributes,
    pub self_application: StructApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub representation: StructRepresentation,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
}

impl StructDefinition {
    pub fn semantic_fields(&self) -> &[Field] {
        self.representation.semantic_fields()
    }
}

impl std::ops::Deref for StructDecl {
    type Target = StructDefinition;

    fn deref(&self) -> &Self::Target {
        &self.definition
    }
}

impl std::ops::DerefMut for StructDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.definition
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedStructDefinition {
    pub declaration: std::sync::Arc<ImportedNominalDeclaration>,
    pub definition: StructDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructApplication {
    pub template: SourceNominalId,
    pub arguments: Vec<TypeId>,
    pub canonical_type: TypeId,
    pub representation: StructApplicationRepresentation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructRepresentation {
    Declared(Vec<Field>),
    Intrinsic(IntrinsicTypeKind),
}

impl StructRepresentation {
    /// Source-visible fields. Intrinsic types deliberately expose no source
    /// fields even though their compiler representation is not empty.
    pub fn semantic_fields(&self) -> &[Field] {
        match self {
            Self::Declared(fields) => fields,
            Self::Intrinsic(_) => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructApplicationRepresentation {
    Declared,
    Intrinsic(IntrinsicTypeRepresentation),
}

/// Typed struct attributes. Raw annotation names and argument syntax never
/// cross the AST/HIR boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StructAttributes {
    pub no_gc: bool,
    pub c_layout: Option<HirCLayoutContract>,
    pub interior_mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirCLayoutValue {
    Natural,
    A1,
    A2,
    A4,
    A8,
    A16,
}

impl HirCLayoutValue {
    pub const fn from_bytes(value: u64) -> Option<Self> {
        match value {
            0 => Some(Self::Natural),
            1 => Some(Self::A1),
            2 => Some(Self::A2),
            4 => Some(Self::A4),
            8 => Some(Self::A8),
            16 => Some(Self::A16),
            _ => None,
        }
    }

    pub const fn bytes(self) -> Option<u8> {
        match self {
            Self::Natural => None,
            Self::A1 => Some(1),
            Self::A2 => Some(2),
            Self::A4 => Some(4),
            Self::A8 => Some(8),
            Self::A16 => Some(16),
        }
    }

    pub const fn from_integer(value: HirIntegerConstant) -> Option<Self> {
        match value {
            HirIntegerConstant::Signed64(raw) => Self::from_bytes(raw),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HirCLayoutContract {
    pub aligned: HirCLayoutValue,
    pub packed: HirCLayoutValue,
}
