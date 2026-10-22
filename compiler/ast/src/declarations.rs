mod requirements;
pub use requirements::*;
mod functions;
pub use functions::*;

use crate::{
    Annotation, AnnotationClassDecl, Block, Expr, Ident, ImportSyntax, PackageSyntax, Span, TypeRef,
};

mod classes;
pub use classes::*;
mod structs;
pub use structs::*;
mod interfaces;
pub use interfaces::*;
mod enums;
pub use enums::*;
mod objects;
pub use objects::*;
mod local_types;
pub use local_types::*;

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub package: PackageSyntax,
    pub imports: Vec<ImportSyntax>,
    pub declarations: Vec<Decl>,
    pub documentation: Vec<crate::DocComment>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Global(GlobalDecl),
    Function(FunctionDecl),
    TypeAlias(TypeAliasDecl),
    AnnotationClass(AnnotationClassDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Class(ClassDecl),
    Interface(InterfaceDecl),
    Object(ObjectDecl),
}

impl Decl {
    pub fn span(&self) -> Span {
        match self {
            Self::Global(node) => node.span,
            Self::Function(node) => node.span,
            Self::TypeAlias(node) => node.span,
            Self::AnnotationClass(node) => node.span,
            Self::Struct(node) => node.span,
            Self::Enum(node) => node.span,
            Self::Class(node) => node.span,
            Self::Interface(node) => node.span,
            Self::Object(node) => node.span,
        }
    }
}

/// A transparent alias, expanded in its declaration-site type scope.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAliasDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub name: Ident,
    pub type_params: Vec<TypeParamDecl>,
    pub where_clause: Option<WhereClause>,
    pub target: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredVisibility {
    Public,
    Internal,
    Private,
    Protected,
}

/// Source visibility keeps an omitted modifier distinct until HIR
/// normalizes it to the language default (`internal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilitySyntax {
    Explicit {
        visibility: DeclaredVisibility,
        span: Span,
    },
    Omitted,
}

impl VisibilitySyntax {
    pub const fn omitted() -> Self {
        Self::Omitted
    }
}

/// A non-local logical property. `GlobalDecl` and the historical
/// `StoredPropertyDecl` names below are aliases so downstream stages can be
/// migrated feature-by-feature without maintaining a second field model.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDecl {
    pub context_parameters: Vec<crate::ContextParameter>,
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub modifier: MethodModifier,
    pub is_override: bool,
    pub mutable: bool,
    /// Present only for a top-level extension property.
    pub receiver_ty: Option<TypeRef>,
    /// Extension-property parameters; ordinary properties keep this empty.
    pub type_params: Vec<TypeParamDecl>,
    pub where_clause: Option<WhereClause>,
    pub name: Ident,
    pub ty: Option<TypeRef>,
    pub body: PropertyBodySyntax,
    pub span: Span,
}

impl PropertyDecl {
    pub fn type_span(&self) -> Span {
        self.ty.as_ref().map_or(self.name.span, |ty| ty.span)
    }

    pub fn initializer(&self) -> Option<&Expr> {
        match &self.body {
            PropertyBodySyntax::Initializer { expression, .. }
            | PropertyBodySyntax::Const(expression) => Some(expression),
            PropertyBodySyntax::OptionalOmitted
            | PropertyBodySyntax::Computed(_)
            | PropertyBodySyntax::Delegated { .. }
            | PropertyBodySyntax::Abstract
            | PropertyBodySyntax::ExternStorage => None,
        }
    }
}

pub type GlobalDecl = PropertyDecl;
pub type StoredPropertyDecl = PropertyDecl;

/// The property form is a closed sum. In particular, a missing initializer
/// cannot be confused with an extern view, a computed property, or the
/// Option-valued omitted-initializer shorthand.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyBodySyntax {
    Initializer {
        expression: Box<Expr>,
        accessors: AccessorSyntax,
    },
    OptionalOmitted,
    Computed(AccessorSyntax),
    Delegated {
        expression: Box<Expr>,
        by_span: Span,
    },
    Abstract,
    ExternStorage,
    Const(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccessorSyntax {
    pub getter: Option<GetterDecl>,
    pub setter: Option<SetterDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GetterDecl {
    pub annotations: Vec<Annotation>,
    pub body: AccessorBodySyntax,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetterDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: SetterVisibilitySyntax,
    pub parameter: SetterParameterSyntax,
    pub body: AccessorBodySyntax,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetterVisibilitySyntax {
    Explicit {
        visibility: DeclaredVisibility,
        span: Span,
    },
    Inherited,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetterParameterSyntax {
    Default { span: Span },
    Named(Ident),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccessorBodySyntax {
    Block(Block),
    Expr(Box<Expr>),
    Omitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    /// Default: cannot be inherited.
    Final,
    Open,
    Abstract,
    Sealed,
}

/// Effective modality of a member function (spec 9.1). Plain class
/// methods are final; an override is open unless explicitly final.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodModifier {
    Final,
    Open,
    Abstract,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NestedTypeDecl {
    TypeAlias(Box<TypeAliasDecl>),
    Struct(Box<StructDecl>),
    Enum(Box<EnumDecl>),
    Class(Box<ClassDecl>),
    Interface(Box<InterfaceDecl>),
    Object(Box<ObjectDecl>),
    AnnotationClass(Box<AnnotationClassDecl>),
}

impl NestedTypeDecl {
    pub fn span(&self) -> Span {
        match self {
            Self::TypeAlias(declaration) => declaration.span,
            Self::Struct(declaration) => declaration.span,
            Self::Enum(declaration) => declaration.span,
            Self::Class(declaration) => declaration.span,
            Self::Interface(declaration) => declaration.span,
            Self::Object(declaration) => declaration.span,
            Self::AnnotationClass(declaration) => declaration.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeParamDecl {
    pub name: Ident,
    pub inline_bound: Option<TypeBound>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeParamKindBound {
    Value,
    Ref,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeBound {
    Kind(TypeParamKindBound),
    Upper(TypeRef),
    ExcludedInterface(TypeRef),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhereClause {
    pub constraints: Vec<TypeConstraint>,
    pub requirements: Vec<MemberRequirements>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeConstraint {
    pub subject: TypeConstraintSubject,
    pub bound: TypeBound,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeConstraintSubject {
    Parameter(Ident),
    Companion(Ident),
}

impl TypeConstraintSubject {
    pub fn parameter(&self) -> &Ident {
        match self {
            Self::Parameter(parameter) | Self::Companion(parameter) => parameter,
        }
    }
}
