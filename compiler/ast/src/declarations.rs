use crate::{Block, CallArgument, Expr, Ident, IntegerLiteralSyntax, Span, TypeRef};

#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    /// At most one package header; `RootPackage` when omitted.
    pub package: PackageSyntax,
    /// File-head imports only; declaration-order preserved.
    pub imports: Vec<ImportSyntax>,
    pub declarations: Vec<Decl>,
    pub span: Span,
}

/// The file's package declaration (spec section 12.4.1). Omission is the
/// explicit root package, never an unknown state.
#[derive(Debug, Clone, PartialEq)]
pub enum PackageSyntax {
    RootPackage,
    QualifiedPackage(QualifiedPath),
}

/// A dot-separated non-empty identifier sequence. The package / owner /
/// entity boundary is resolved by HIR typed namespaces, never by string
/// concatenation here.
#[derive(Debug, Clone, PartialEq)]
pub struct QualifiedPath {
    pub segments: Vec<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportSyntax {
    Exact {
        public: bool,
        path: QualifiedPath,
        alias: Option<Ident>,
        span: Span,
    },
    Star {
        public: bool,
        path: QualifiedPath,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Global(GlobalDecl),
    Function(FunctionDecl),
    TypeAlias(TypeAliasDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Class(ClassDecl),
    Interface(InterfaceDecl),
    Object(ObjectDecl),
}

/// A top-level, non-generic transparent type alias (spec 3.2.1).
///
/// The declaration has source identity and visibility, while its target is
/// the only type represented by the alias. Generic and nested/local aliases
/// are rejected by the parser in the M22 language subset.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAliasDecl {
    pub visibility: VisibilitySyntax,
    pub name: Ident,
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
    pub ty: TypeRef,
    pub body: PropertyBodySyntax,
    pub span: Span,
}

impl PropertyDecl {
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
    pub visibility: VisibilitySyntax,
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
            | ClassMember::SecondaryConstructor(_)
            | ClassMember::Nested(_)
            | ClassMember::Companion(_) => None,
        })
    }

    pub fn secondary_constructors(&self) -> impl Iterator<Item = &SecondaryConstructorDecl> {
        self.members.iter().filter_map(|member| match member {
            ClassMember::SecondaryConstructor(constructor) => Some(constructor),
            ClassMember::StoredProperty(_)
            | ClassMember::InitBlock(_)
            | ClassMember::Function(_)
            | ClassMember::Nested(_)
            | ClassMember::Companion(_) => None,
        })
    }
}

/// The source form of a class primary constructor. Keeping omission distinct
/// from an explicit empty constructor lets HIR validate intrinsic type shapes
/// without reconstructing syntax from an empty property list.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassConstructorDecl {
    Omitted,
    Declared(PrimaryConstructorDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryConstructorDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub parameters: Vec<PrimaryClassParameter>,
    pub span: Span,
}

impl std::ops::Deref for PrimaryConstructorDecl {
    type Target = [PrimaryClassParameter];

    fn deref(&self) -> &Self::Target {
        &self.parameters
    }
}

impl std::ops::DerefMut for PrimaryConstructorDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parameters
    }
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
            Self::Declared(constructor) => &constructor.parameters,
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
        let parameters = iter.into_iter().collect::<Vec<_>>();
        let span = parameters
            .first()
            .zip(parameters.last())
            .map_or(Span::new(0, 0), |(first, last)| {
                Span::new(first.span.start, last.span.end)
            });
        Self::Declared(PrimaryConstructorDecl {
            annotations: Vec::new(),
            visibility: VisibilitySyntax::Omitted,
            parameters,
            span,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryClassParameter {
    pub property: PrimaryParameterProperty,
    /// `None` for a plain parameter. Property parameters always retain an
    /// explicit-or-omitted member visibility node.
    pub member_visibility: Option<VisibilitySyntax>,
    pub is_override: bool,
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
    Nested(Box<NestedNominalDecl>),
    Companion(Box<CompanionObjectDecl>),
}

impl ClassMember {
    pub fn span(&self) -> Span {
        match self {
            Self::StoredProperty(property) => property.span,
            Self::InitBlock(init) => init.span,
            Self::SecondaryConstructor(constructor) => constructor.span,
            Self::Function(function) => function.span,
            Self::Nested(declaration) => declaration.span(),
            Self::Companion(companion) => companion.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StructMember {
    SecondaryConstructor(SecondaryConstructorDecl),
    Function(Box<FunctionDecl>),
    Property(Box<PropertyDecl>),
    Nested(Box<NestedNominalDecl>),
    Companion(Box<CompanionObjectDecl>),
}

impl StructMember {
    pub fn span(&self) -> Span {
        match self {
            Self::SecondaryConstructor(constructor) => constructor.span,
            Self::Function(function) => function.span,
            Self::Property(property) => property.span,
            Self::Nested(declaration) => declaration.span(),
            Self::Companion(companion) => companion.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InitBlockDecl {
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SecondaryConstructorDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
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

/// An interface keeps method defaults and accessor-level property syntax;
/// semantic obligation/default selection belongs to HIR.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub name: Ident,
    pub type_params: Vec<TypeParamDecl>,
    pub supertypes: Vec<SupertypeSpec>,
    pub where_clause: Option<WhereClause>,
    pub methods: Vec<FunctionDecl>,
    pub properties: Vec<PropertyDecl>,
    pub nested: Vec<NestedNominalDecl>,
    pub companion: Option<CompanionObjectDecl>,
    pub span: Span,
}

/// A singleton object declaration. Its type and value identities are split
/// later by HIR; the AST keeps one source declaration with ordered members.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub name: Ident,
    pub supertypes: Vec<SupertypeSpec>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionObjectDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub name: CompanionNameSyntax,
    pub supertypes: Vec<SupertypeSpec>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompanionNameSyntax {
    Default { span: Span },
    Named(Ident),
}

#[derive(Debug, Clone, PartialEq)]
pub enum NestedNominalDecl {
    Struct(Box<StructDecl>),
    Enum(Box<EnumDecl>),
    Class(Box<ClassDecl>),
    Interface(Box<InterfaceDecl>),
    Object(Box<ObjectDecl>),
}

impl NestedNominalDecl {
    pub fn span(&self) -> Span {
        match self {
            Self::Struct(declaration) => declaration.span,
            Self::Enum(declaration) => declaration.span,
            Self::Class(declaration) => declaration.span,
            Self::Interface(declaration) => declaration.span,
            Self::Object(declaration) => declaration.span,
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
    pub visibility: VisibilitySyntax,
    pub name: Ident,
    pub type_params: Vec<TypeParamDecl>,
    pub variants: Vec<VariantDecl>,
    /// Implemented interfaces (spec 4.4.3).
    pub interfaces: Vec<TypeRef>,
    pub where_clause: Option<WhereClause>,
    /// Member functions (spec 4.2).
    pub methods: Vec<FunctionDecl>,
    pub properties: Vec<PropertyDecl>,
    pub nested: Vec<NestedNominalDecl>,
    pub companion: Option<CompanionObjectDecl>,
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
    pub visibility: VisibilitySyntax,
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
            StructMember::SecondaryConstructor(_)
            | StructMember::Property(_)
            | StructMember::Nested(_)
            | StructMember::Companion(_) => None,
        })
    }

    pub fn secondary_constructors(&self) -> impl Iterator<Item = &SecondaryConstructorDecl> {
        self.members.iter().filter_map(|member| match member {
            StructMember::SecondaryConstructor(constructor) => Some(constructor),
            StructMember::Function(_)
            | StructMember::Property(_)
            | StructMember::Nested(_)
            | StructMember::Companion(_) => None,
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
    pub visibility: VisibilitySyntax,
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
    Int(IntegerLiteralSyntax),
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
