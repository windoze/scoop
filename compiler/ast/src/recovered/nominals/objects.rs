//! Singleton inputs retained when only member method bodies are incomplete.

use crate::{ClassMember, RecoveredNominalMembers, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredObject {
    pub header: crate::ObjectHeader,
    pub members: Vec<ClassMember>,
    pub recovery: RecoveredNominalMembers,
    pub span: Span,
}

impl RecoveredObject {
    pub fn source(&self) -> crate::ObjectSource<'_> {
        crate::ObjectSource {
            header: &self.header,
            members: &self.members,
            recovery: &self.recovery,
            span: self.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredCompanionObject {
    pub header: crate::CompanionObjectHeader,
    pub members: Vec<ClassMember>,
    pub recovery: RecoveredNominalMembers,
    pub span: Span,
}

impl RecoveredCompanionObject {
    pub fn source(&self) -> crate::CompanionObjectSource<'_> {
        crate::CompanionObjectSource {
            header: &self.header,
            members: &self.members,
            recovery: &self.recovery,
            span: self.span,
        }
    }
}
