//! Enum headers, variants and independently completed members.

use super::*;

/// `enum E<T> { ... }` (spec 4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub header: EnumHeader,
    pub variants: Vec<VariantDecl>,
    pub methods: Vec<FunctionDecl>,
    pub properties: Vec<PropertyDecl>,
    pub nested: Vec<NestedTypeDecl>,
    pub companion: Option<CompanionObjectDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumHeader {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub name: Ident,
    pub type_params: Vec<TypeParamDecl>,
    /// Implemented interfaces (spec 4.4.3).
    pub interfaces: Vec<TypeRef>,
    pub where_clause: Option<WhereClause>,
    pub span: Span,
}

impl std::ops::Deref for EnumDecl {
    type Target = EnumHeader;
    fn deref(&self) -> &Self::Target {
        &self.header
    }
}

impl std::ops::DerefMut for EnumDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.header
    }
}

impl EnumDecl {
    pub fn source(&self) -> EnumSource<'_> {
        EnumSource {
            header: &self.header,
            variants: &self.variants,
            methods: &self.methods,
            properties: &self.properties,
            nested: &self.nested,
            companion: self.companion.as_ref(),
            recovery: crate::RecoveredNominalMembers::EMPTY,
            span: self.span,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EnumSource<'a> {
    pub header: &'a EnumHeader,
    pub variants: &'a [VariantDecl],
    pub methods: &'a [FunctionDecl],
    pub properties: &'a [PropertyDecl],
    pub nested: &'a [NestedTypeDecl],
    pub companion: Option<&'a CompanionObjectDecl>,
    pub recovery: &'a crate::RecoveredNominalMembers,
    pub span: Span,
}

impl std::ops::Deref for EnumSource<'_> {
    type Target = EnumHeader;
    fn deref(&self) -> &Self::Target {
        self.header
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariantDecl {
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub kind: VariantDeclKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VariantDeclKind {
    /// `Variant = integer_constant` in an integral enum.
    Constant(Box<crate::Expr>),
    /// `SimpleVariant`
    Unit,
    /// `VariantWithValue(Int, String)` — unnamed fields.
    Positional(Vec<crate::PositionalVariantFieldDecl>),
    /// `Variant { f1: Int, f2: String }` — block-style named fields.
    Named(Vec<VariantFieldDecl>),
    /// `Variant(val f1: Int, val f2: String = "...")` —
    /// constructor-style named fields (spec 4.2); defaults are
    /// constant expressions in M4 (DESIGN.md 5.4).
    Constructor(Vec<VariantFieldDecl>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariantFieldDecl {
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub ty: TypeRef,
    /// Constructor-style variants use the full source parameter protocol;
    /// block-style named variants always use [`ParameterSyntax::Required`].
    pub syntax: ParameterSyntax,
    pub span: Span,
}
