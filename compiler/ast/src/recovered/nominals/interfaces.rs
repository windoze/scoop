use crate::{
    CompanionObjectDecl, FunctionDecl, InterfaceHeader, InterfaceSource, NestedTypeDecl,
    PropertyDecl, RecoveredNominalMembers, Span,
};

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredInterface {
    pub header: InterfaceHeader,
    pub methods: Vec<FunctionDecl>,
    pub properties: Vec<PropertyDecl>,
    pub nested: Vec<NestedTypeDecl>,
    pub companion: Option<CompanionObjectDecl>,
    pub recovery: RecoveredNominalMembers,
    pub span: Span,
}

impl RecoveredInterface {
    pub fn source(&self) -> InterfaceSource<'_> {
        InterfaceSource {
            header: &self.header,
            methods: &self.methods,
            properties: &self.properties,
            nested: &self.nested,
            companion: self.companion.as_ref(),
            recovery: &self.recovery,
            span: self.span,
        }
    }
}
