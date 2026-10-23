//! Complete nominal inputs retained when only method bodies are incomplete.

use crate::{
    ClassHeader, ClassMember, ClassSource, FunctionHeader, FunctionHeaderRef,
    RecoveredFunctionBody, Span, StructHeader, StructMember, StructSource,
};

mod interfaces;
pub use interfaces::*;
mod enums;
pub use enums::*;
mod objects;
pub use objects::*;

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveredNominal {
    Class(Box<RecoveredClass>),
    Struct(Box<RecoveredStruct>),
    Interface(Box<RecoveredInterface>),
    Enum(Box<RecoveredEnum>),
    Object(Box<RecoveredObject>),
}

impl RecoveredNominal {
    pub fn span(&self) -> Span {
        match self {
            Self::Class(value) => value.span,
            Self::Struct(value) => value.span,
            Self::Interface(value) => value.span,
            Self::Enum(value) => value.span,
            Self::Object(value) => value.span,
        }
    }

    pub fn recovery_mut(&mut self) -> &mut RecoveredNominalMembers {
        match self {
            Self::Class(value) => &mut value.recovery,
            Self::Struct(value) => &mut value.recovery,
            Self::Interface(value) => &mut value.recovery,
            Self::Enum(value) => &mut value.recovery,
            Self::Object(value) => &mut value.recovery,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecoveredNominalMembers {
    pub methods: Vec<RecoveredMethod>,
    pub nested: Vec<RecoveredNominal>,
    pub companion: Option<Box<RecoveredCompanionObject>>,
}

impl RecoveredNominalMembers {
    pub const EMPTY: &'static Self = &Self {
        methods: Vec::new(),
        nested: Vec::new(),
        companion: None,
    };
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredClass {
    pub header: ClassHeader,
    pub members: Vec<ClassMember>,
    pub recovery: RecoveredNominalMembers,
    pub span: Span,
}

impl RecoveredClass {
    pub fn source(&self) -> ClassSource<'_> {
        ClassSource {
            header: &self.header,
            members: &self.members,
            recovery: &self.recovery,
            span: self.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredStruct {
    pub header: StructHeader,
    pub members: Vec<StructMember>,
    pub recovery: RecoveredNominalMembers,
    pub span: Span,
}

impl RecoveredStruct {
    pub fn source(&self) -> StructSource<'_> {
        StructSource {
            header: &self.header,
            members: &self.members,
            recovery: &self.recovery,
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
