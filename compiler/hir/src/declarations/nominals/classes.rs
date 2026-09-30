use super::*;

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub modifier: ClassModifier,
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub access: NominalAccess,
    pub self_application: ClassApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
    pub representation: ClassRepresentation,
    pub fields: Vec<ClassFieldId>,
    pub properties: Vec<PropertyId>,
    pub constructors: Vec<ClassConstructorId>,
    pub base_class: Option<TypeId>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
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

#[derive(Debug, Clone)]
pub enum ClassRepresentation {
    Declared,
    Intrinsic(IntrinsicTypeKind),
}

#[derive(Debug, Clone)]
pub struct ClassField {
    pub owner: ClassId,
    pub property: PropertyId,
    pub ty: TypeId,
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
