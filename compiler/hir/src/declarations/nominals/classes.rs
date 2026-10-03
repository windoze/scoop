use super::*;

mod queries;

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub access: NominalAccess,
    pub definition: ClassDefinition,
    /// Source initialization metadata, in the common field table's order.
    pub fields: Vec<ClassFieldId>,
    pub properties: Vec<PropertyId>,
    pub constructors: Vec<ClassConstructorId>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

/// Checked fields and parents in the original declaration's binder domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDefinition {
    pub release_policy: ReleasePolicy<ExportReleaseHookRef>,
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
    pub modifier: ClassModifier,
    pub self_application: ClassApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub representation: ClassRepresentation,
    pub fields: Vec<Field>,
    pub base_class: Option<TypeId>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedClassDefinition {
    pub declaration: std::sync::Arc<ImportedNominalDeclaration>,
    pub definition: ClassDefinition,
    pub virtual_methods: Vec<ImportedVirtualMethod>,
}

impl std::ops::Deref for ClassDecl {
    type Target = ClassDefinition;

    fn deref(&self) -> &Self::Target {
        &self.definition
    }
}

impl std::ops::DerefMut for ClassDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.definition
    }
}

/// Typed evaluation plan for one constructor edge. Source argument syntax and
/// missing/default state have already been eliminated.
#[derive(Debug, Clone)]
pub struct ConstructorArguments {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
    pub args: Vec<Expr>,
}

impl ClassDecl {
    pub fn is_declared(&self) -> bool {
        matches!(self.representation, ClassRepresentation::Declared)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassApplication {
    pub template: SourceNominalId,
    pub arguments: Vec<TypeId>,
    pub canonical_type: TypeId,
    pub representation: ClassApplicationRepresentation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassRepresentation {
    Declared,
    Intrinsic(IntrinsicTypeKind),
}

#[derive(Debug, Clone)]
pub struct ClassField {
    pub owner: ClassId,
    pub property: PropertyId,
    /// Position in the owner's common field definition; not an entity identity.
    pub definition_index: usize,
    pub source: ClassFieldSource,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassFieldSource {
    PrimaryParameter(ConstructorParamId),
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassApplicationRepresentation {
    Declared,
    Intrinsic(IntrinsicTypeRepresentation),
}
