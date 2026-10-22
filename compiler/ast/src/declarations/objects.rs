//! Singleton headers and independently completed members.

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDecl {
    pub header: ObjectHeader,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectHeader {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub name: Ident,
    pub supertypes: Vec<SupertypeSpec>,
    pub span: Span,
}

impl std::ops::Deref for ObjectDecl {
    type Target = ObjectHeader;
    fn deref(&self) -> &Self::Target {
        &self.header
    }
}

impl std::ops::DerefMut for ObjectDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.header
    }
}

impl ObjectDecl {
    pub fn source(&self) -> ObjectSource<'_> {
        ObjectSource {
            header: &self.header,
            members: &self.members,
            recovery: crate::RecoveredNominalMembers::EMPTY,
            span: self.span,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectSource<'a> {
    pub header: &'a ObjectHeader,
    pub members: &'a [ClassMember],
    pub recovery: &'a crate::RecoveredNominalMembers,
    pub span: Span,
}

impl std::ops::Deref for ObjectSource<'_> {
    type Target = ObjectHeader;
    fn deref(&self) -> &Self::Target {
        self.header
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionObjectDecl {
    pub header: CompanionObjectHeader,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionObjectHeader {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub name: CompanionNameSyntax,
    pub supertypes: Vec<SupertypeSpec>,
    pub span: Span,
}

impl std::ops::Deref for CompanionObjectDecl {
    type Target = CompanionObjectHeader;
    fn deref(&self) -> &Self::Target {
        &self.header
    }
}

impl std::ops::DerefMut for CompanionObjectDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.header
    }
}

impl CompanionObjectDecl {
    pub fn source(&self) -> CompanionObjectSource<'_> {
        CompanionObjectSource {
            header: &self.header,
            members: &self.members,
            recovery: crate::RecoveredNominalMembers::EMPTY,
            span: self.span,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CompanionObjectSource<'a> {
    pub header: &'a CompanionObjectHeader,
    pub members: &'a [ClassMember],
    pub recovery: &'a crate::RecoveredNominalMembers,
    pub span: Span,
}

impl std::ops::Deref for CompanionObjectSource<'_> {
    type Target = CompanionObjectHeader;
    fn deref(&self) -> &Self::Target {
        self.header
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompanionNameSyntax {
    Default { span: Span },
    Named(Ident),
}
