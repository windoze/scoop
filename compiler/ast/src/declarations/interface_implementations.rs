//! Interface implementation blocks in an original nominal body.

use super::*;

/// `override interface I<Args> where T : Bound { ... }` contributes an
/// interface relation and members without introducing a nominal or binder.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceImplementationDecl {
    pub interface: TypeRef,
    pub where_clause: Option<WhereClause>,
    pub members: Vec<InterfaceImplementationMember>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterfaceImplementationMember {
    Function(Box<FunctionDecl>),
    Property(Box<PropertyDecl>),
}

impl InterfaceImplementationMember {
    pub fn span(&self) -> Span {
        match self {
            Self::Function(function) => function.span,
            Self::Property(property) => property.span,
        }
    }
}

impl InterfaceImplementationDecl {
    pub fn functions(&self) -> impl Iterator<Item = &FunctionDecl> {
        self.members.iter().filter_map(|member| match member {
            InterfaceImplementationMember::Function(function) => Some(function.as_ref()),
            InterfaceImplementationMember::Property(_) => None,
        })
    }

    pub fn properties(&self) -> impl Iterator<Item = &PropertyDecl> {
        self.members.iter().filter_map(|member| match member {
            InterfaceImplementationMember::Property(property) => Some(property.as_ref()),
            InterfaceImplementationMember::Function(_) => None,
        })
    }
}
