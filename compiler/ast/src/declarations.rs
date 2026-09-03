use crate::{Block, CallArgument, Expr, Ident, Span, TypeRef};

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub declarations: Vec<Decl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Global(GlobalDecl),
    Function(FunctionDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Class(ClassDecl),
    Interface(InterfaceDecl),
}

/// A top-level storage declaration. Unlike block-local bindings, a global
/// always has an explicit type and may omit its initializer only when it is
/// imported with `@Extern`.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalDecl {
    pub annotations: Vec<Annotation>,
    pub mutable: bool,
    pub name: Ident,
    pub ty: TypeRef,
    pub init: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    /// Default: cannot be inherited.
    Final,
    Open,
    Abstract,
}

/// Effective modality of a member function (spec 9.1). Plain class
/// methods are final; an override is open unless explicitly final.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodModifier {
    Final,
    Open,
    Abstract,
}

/// `class Name(props) : Base(args), I1, I2 { members }` (spec 9.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub annotations: Vec<Annotation>,
    pub modifier: ClassModifier,
    pub name: Ident,
    /// Generic host parameters (`class Name<T, U> ...`).
    pub type_params: Vec<TypeParamDecl>,
    /// Source primary constructor. Omission is distinct from `()` because a
    /// class that declares secondary constructors may have no primary.
    pub constructor: ClassConstructorDecl,
    /// Supertypes in source order. Nominal kind is resolved by HIR; parser
    /// deliberately does not equate parentheses with "base class".
    pub supertypes: Vec<SupertypeSpec>,
    pub where_clause: Option<WhereClause>,
    /// Class body items in source order so property initializers and `init`
    /// blocks keep their observable interleaving.
    pub members: Vec<ClassMember>,
    pub span: Span,
}

impl ClassDecl {
    pub fn functions(&self) -> impl Iterator<Item = &FunctionDecl> {
        self.members.iter().filter_map(|member| match member {
            ClassMember::Function(function) => Some(function),
            ClassMember::StoredProperty(_)
            | ClassMember::InitBlock(_)
            | ClassMember::SecondaryConstructor(_) => None,
        })
    }

    pub fn secondary_constructors(&self) -> impl Iterator<Item = &SecondaryConstructorDecl> {
        self.members.iter().filter_map(|member| match member {
            ClassMember::SecondaryConstructor(constructor) => Some(constructor),
            ClassMember::StoredProperty(_)
            | ClassMember::InitBlock(_)
            | ClassMember::Function(_) => None,
        })
    }
}

/// The source form of a class primary constructor. Keeping omission distinct
/// from an explicit empty constructor lets HIR validate intrinsic type shapes
/// without reconstructing syntax from an empty property list.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassConstructorDecl {
    Omitted,
    Declared(Vec<PrimaryClassParameter>),
}

impl ClassConstructorDecl {
    pub fn is_omitted(&self) -> bool {
        matches!(self, Self::Omitted)
    }
}

impl std::ops::Deref for ClassConstructorDecl {
    type Target = [PrimaryClassParameter];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Omitted => &[],
            Self::Declared(properties) => properties,
        }
    }
}

impl<'a> IntoIterator for &'a ClassConstructorDecl {
    type Item = &'a PrimaryClassParameter;
    type IntoIter = std::slice::Iter<'a, PrimaryClassParameter>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl FromIterator<PrimaryClassParameter> for ClassConstructorDecl {
    fn from_iter<T: IntoIterator<Item = PrimaryClassParameter>>(iter: T) -> Self {
        Self::Declared(iter.into_iter().collect())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryClassParameter {
    pub property: PrimaryParameterProperty,
    pub name: Ident,
    pub ty: TypeRef,
    pub syntax: ParameterSyntax,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryParameterProperty {
    Plain,
    Val,
    Var,
}

impl PrimaryParameterProperty {
    pub const fn is_property(self) -> bool {
        matches!(self, Self::Val | Self::Var)
    }

    pub const fn is_mutable(self) -> bool {
        matches!(self, Self::Var)
    }
}

/// One syntactic entry after `:` on a nominal declaration. Parentheses are
/// retained even when empty; HIR resolves whether the target is a class or
/// interface and applies the corresponding legality rules.
#[derive(Debug, Clone, PartialEq)]
pub struct SupertypeSpec {
    pub ty: TypeRef,
    pub constructor_arguments: Option<Vec<CallArgument>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    StoredProperty(StoredPropertyDecl),
    InitBlock(InitBlockDecl),
    SecondaryConstructor(SecondaryConstructorDecl),
    Function(FunctionDecl),
}

impl ClassMember {
    pub fn span(&self) -> Span {
        match self {
            Self::StoredProperty(property) => property.span,
            Self::InitBlock(init) => init.span,
            Self::SecondaryConstructor(constructor) => constructor.span,
            Self::Function(function) => function.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StructMember {
    SecondaryConstructor(SecondaryConstructorDecl),
    Function(Box<FunctionDecl>),
}

impl StructMember {
    pub fn span(&self) -> Span {
        match self {
            Self::SecondaryConstructor(constructor) => constructor.span,
            Self::Function(function) => function.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredPropertyDecl {
    pub mutable: bool,
    pub name: Ident,
    pub ty: TypeRef,
    pub initializer: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InitBlockDecl {
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SecondaryConstructorDecl {
    pub params: Vec<Param>,
    pub delegation: Option<ConstructorDelegation>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstructorDelegation {
    This {
        target_span: Span,
        arguments: Vec<CallArgument>,
        span: Span,
    },
    Super {
        target_span: Span,
        arguments: Vec<CallArgument>,
        span: Span,
    },
}

impl ConstructorDelegation {
    pub fn span(&self) -> Span {
        match self {
            Self::This { span, .. } | Self::Super { span, .. } => *span,
        }
    }

    pub fn arguments(&self) -> &[CallArgument] {
        match self {
            Self::This { arguments, .. } | Self::Super { arguments, .. } => arguments,
        }
    }
}

/// `interface I { fun m(x: Int): String ... }` — method signatures
/// only in M6 (no properties, no default implementations).
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub type_params: Vec<TypeParamDecl>,
    pub supertypes: Vec<SupertypeSpec>,
    pub where_clause: Option<WhereClause>,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    Invariant,
    In,
    Out,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeParamDecl {
    pub name: Ident,
    pub variance: Variance,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhereClause {
    pub constraints: Vec<TypeConstraint>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeConstraint {
    pub parameter: Ident,
    pub bound: TypeBound,
    pub span: Span,
}

/// `enum E<T> { ... }` (spec 4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub type_params: Vec<TypeParamDecl>,
    pub variants: Vec<VariantDecl>,
    /// Implemented interfaces (spec 4.4.3).
    pub interfaces: Vec<TypeRef>,
    pub where_clause: Option<WhereClause>,
    /// Member functions (spec 4.2).
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariantDecl {
    pub name: Ident,
    pub kind: VariantDeclKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VariantDeclKind {
    /// `SimpleVariant`
    Unit,
    /// `VariantWithValue(Int, String)` — unnamed fields.
    Positional(Vec<TypeRef>),
    /// `Variant { f1: Int, f2: String }` — block-style named fields.
    Named(Vec<VariantFieldDecl>),
    /// `Variant(val f1: Int, val f2: String = "...")` —
    /// constructor-style named fields (spec 4.2); defaults are
    /// constant expressions in M4 (DESIGN.md 5.4).
    Constructor(Vec<VariantFieldDecl>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariantFieldDecl {
    pub name: Ident,
    pub ty: TypeRef,
    /// Constructor-style variants use the full source parameter protocol;
    /// block-style named variants always use [`ParameterSyntax::Required`].
    pub syntax: ParameterSyntax,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    /// Generic type parameters (`struct Name<T, U>(...)`); empty for
    /// non-generic structs.
    pub type_params: Vec<TypeParamDecl>,
    pub fields: StructRepresentationDecl,
    /// Syntactic supertypes. HIR requires every resolved target to be an
    /// interface and rejects constructor argument lists.
    pub supertypes: Vec<SupertypeSpec>,
    pub where_clause: Option<WhereClause>,
    pub members: Vec<StructMember>,
    pub span: Span,
}

impl StructDecl {
    pub fn functions(&self) -> impl Iterator<Item = &FunctionDecl> {
        self.members.iter().filter_map(|member| match member {
            StructMember::Function(function) => Some(function.as_ref()),
            StructMember::SecondaryConstructor(_) => None,
        })
    }

    pub fn secondary_constructors(&self) -> impl Iterator<Item = &SecondaryConstructorDecl> {
        self.members.iter().filter_map(|member| match member {
            StructMember::SecondaryConstructor(constructor) => Some(constructor),
            StructMember::Function(_) => None,
        })
    }
}

/// The representation syntax of a struct declaration. Only a registry-approved
/// intrinsic type may use `Omitted`; ordinary zero-field structs use
/// `Declared(Vec::new())` and therefore remain distinguishable.
#[derive(Debug, Clone, PartialEq)]
pub enum StructRepresentationDecl {
    Omitted,
    Declared(Vec<FieldDecl>),
}

impl StructRepresentationDecl {
    pub fn is_omitted(&self) -> bool {
        matches!(self, Self::Omitted)
    }
}

impl std::ops::Deref for StructRepresentationDecl {
    type Target = [FieldDecl];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Omitted => &[],
            Self::Declared(fields) => fields,
        }
    }
}

impl<'a> IntoIterator for &'a StructRepresentationDecl {
    type Item = &'a FieldDecl;
    type IntoIter = std::slice::Iter<'a, FieldDecl>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl FromIterator<FieldDecl> for StructRepresentationDecl {
    fn from_iter<T: IntoIterator<Item = FieldDecl>>(iter: T) -> Self {
        Self::Declared(iter.into_iter().collect())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub name: Ident,
    pub ty: TypeRef,
    pub syntax: ParameterSyntax,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    /// Compiler-recognized annotations in source order (spec 9.4 / M12).
    pub annotations: Vec<Annotation>,
    /// Whether this callable uses the coroutine calling convention.
    pub is_suspend: bool,
    /// `override` (required when overriding, forbidden otherwise).
    pub is_override: bool,
    /// Present exactly when the declaration has an `operator` modifier; the
    /// modifier span is retained for HIR signature/target diagnostics.
    pub operator: Option<OperatorModifier>,
    /// Present exactly when the declaration has an `infix` modifier.
    pub infix: Option<InfixModifier>,
    /// Effective member modality. It is `Final` for top-level
    /// functions, where member modality is not applicable.
    pub modifier: MethodModifier,
    /// Receiver type of a top-level extension function (`fun T.name(...)`).
    /// `None` for ordinary top-level functions, members, and local functions.
    pub receiver_ty: Option<TypeRef>,
    pub name: Ident,
    /// Generic type parameters (`fun <T> f(...)`); empty for
    /// non-generic functions.
    pub type_params: Vec<TypeParamDecl>,
    pub params: Vec<Param>,
    /// Return type annotation; absent means `Unit`.
    pub return_ty: Option<TypeRef>,
    pub where_clause: Option<WhereClause>,
    pub body: FunctionBody,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorModifier {
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfixModifier {
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub name: Ident,
    pub args: Vec<AnnotationArg>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationArg {
    /// A named argument (`name = value`), or `None` for a positional one.
    pub name: Option<Ident>,
    pub value: AnnotationLiteral,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationLiteral {
    String(String),
    Int(i64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeRef,
    pub syntax: ParameterSyntax,
    pub span: Span,
}

/// Source calling shape of one declared value parameter. The variants keep
/// impossible combinations (for example, a required parameter carrying a
/// default expression) out of the AST.
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterSyntax {
    Required,
    Default {
        expression: Expr,
        equals_span: Span,
    },
    Vararg {
        modifier_span: Span,
        default: VarargDefaultSyntax,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarargDefaultSyntax {
    EmptyWhenOmitted,
    Expression { expression: Expr, equals_span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionBody {
    Block(Block),
    /// `fun f(...) [: T] = expr`
    Expr(Box<Expr>),
    /// Bodyless declaration. HIR decides whether the declaration kind permits
    /// the missing body (`abstract`, interface, intrinsic or extern).
    None,
}
