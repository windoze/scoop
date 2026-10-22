//! Struct headers, representations and ordered members.

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum StructMember {
    SecondaryConstructor(SecondaryConstructorDecl),
    Function(Box<FunctionDecl>),
    Property(Box<PropertyDecl>),
    Nested(Box<NestedTypeDecl>),
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
pub struct StructDecl {
    pub header: StructHeader,
    pub members: Vec<StructMember>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructHeader {
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
    pub span: Span,
}

impl StructDecl {
    pub fn source(&self) -> StructSource<'_> {
        StructSource {
            header: &self.header,
            members: &self.members,
            recovery: crate::RecoveredNominalMembers::EMPTY,
            span: self.span,
        }
    }

    pub fn functions(&self) -> impl Iterator<Item = &FunctionDecl> {
        self.source().functions()
    }

    pub fn secondary_constructors(&self) -> impl Iterator<Item = &SecondaryConstructorDecl> {
        self.source().secondary_constructors()
    }
}

impl std::ops::Deref for StructDecl {
    type Target = StructHeader;

    fn deref(&self) -> &Self::Target {
        &self.header
    }
}

impl std::ops::DerefMut for StructDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.header
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StructSource<'a> {
    pub header: &'a StructHeader,
    pub members: &'a [StructMember],
    pub recovery: &'a crate::RecoveredNominalMembers,
    pub span: Span,
}

impl std::ops::Deref for StructSource<'_> {
    type Target = StructHeader;

    fn deref(&self) -> &Self::Target {
        self.header
    }
}

impl<'a> StructSource<'a> {
    pub fn functions(self) -> impl Iterator<Item = &'a FunctionDecl> {
        self.members.iter().filter_map(|member| match member {
            StructMember::Function(function) => Some(function.as_ref()),
            StructMember::SecondaryConstructor(_)
            | StructMember::Property(_)
            | StructMember::Nested(_)
            | StructMember::Companion(_) => None,
        })
    }

    pub fn secondary_constructors(self) -> impl Iterator<Item = &'a SecondaryConstructorDecl> {
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
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub ty: TypeRef,
    pub syntax: ParameterSyntax,
    pub span: Span,
}
