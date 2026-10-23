//! Interface headers and independently completed members.

use super::*;

/// An interface keeps method defaults and accessor-level property syntax;
/// semantic obligation/default selection belongs to HIR.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDecl {
    pub header: InterfaceHeader,
    pub methods: Vec<FunctionDecl>,
    pub properties: Vec<PropertyDecl>,
    pub nested: Vec<NestedTypeDecl>,
    pub companion: Option<CompanionObjectDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceHeader {
    pub sealed_span: Option<Span>,
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub name: Ident,
    pub type_params: Vec<TypeParamDecl>,
    pub supertypes: Vec<SupertypeSpec>,
    pub where_clause: Option<WhereClause>,
    pub span: Span,
}

impl std::ops::Deref for InterfaceDecl {
    type Target = InterfaceHeader;
    fn deref(&self) -> &Self::Target {
        &self.header
    }
}

impl std::ops::DerefMut for InterfaceDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.header
    }
}

impl InterfaceDecl {
    pub fn source(&self) -> InterfaceSource<'_> {
        InterfaceSource {
            header: &self.header,
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
pub struct InterfaceSource<'a> {
    pub header: &'a InterfaceHeader,
    pub methods: &'a [FunctionDecl],
    pub properties: &'a [PropertyDecl],
    pub nested: &'a [NestedTypeDecl],
    pub companion: Option<&'a CompanionObjectDecl>,
    pub recovery: &'a crate::RecoveredNominalMembers,
    pub span: Span,
}

impl std::ops::Deref for InterfaceSource<'_> {
    type Target = InterfaceHeader;
    fn deref(&self) -> &Self::Target {
        self.header
    }
}
