use crate::{
    ClassDecl, ClassMember, EnumDecl, Ident, InterfaceDecl, Span, StructDecl, SupertypeSpec,
};

#[derive(Debug, Clone, PartialEq)]
pub enum LocalTypeDecl {
    Class(Box<ClassDecl>),
    Struct(Box<StructDecl>),
    Enum(Box<EnumDecl>),
    Interface(Box<InterfaceDecl>),
}

impl LocalTypeDecl {
    pub fn name(&self) -> &Ident {
        match self {
            Self::Class(value) => &value.name,
            Self::Struct(value) => &value.name,
            Self::Enum(value) => &value.name,
            Self::Interface(value) => &value.name,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Self::Class(value) => value.span,
            Self::Struct(value) => value.span,
            Self::Enum(value) => value.span,
            Self::Interface(value) => value.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnonymousObjectDecl {
    pub supertypes: Vec<SupertypeSpec>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}
