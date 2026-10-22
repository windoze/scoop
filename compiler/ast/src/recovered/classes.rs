//! Complete class inputs retained when only member bodies are incomplete.

use crate::{
    ClassHeader, ClassMember, ClassSource, FunctionHeader, FunctionHeaderRef,
    RecoveredFunctionBody, Span,
};

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredClass {
    pub header: ClassHeader,
    pub members: Vec<ClassMember>,
    pub methods: Vec<RecoveredMethod>,
    pub classes: Vec<RecoveredClass>,
    pub span: Span,
}

impl RecoveredClass {
    pub fn source(&self) -> ClassSource<'_> {
        ClassSource {
            header: &self.header,
            members: &self.members,
            recovered_methods: &self.methods,
            recovered_classes: &self.classes,
            span: self.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredMethod {
    pub header: FunctionHeader,
    pub body: Option<RecoveredFunctionBody>,
    pub span: Span,
}

impl RecoveredMethod {
    pub fn header(&self) -> FunctionHeaderRef<'_> {
        FunctionHeaderRef {
            signature: &self.header.signature,
            body_form: self.header.body_form,
            span: self.span,
        }
    }
}
