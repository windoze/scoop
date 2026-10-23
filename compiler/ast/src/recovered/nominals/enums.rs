use crate::{
    CompanionObjectDecl, EnumHeader, EnumSource, FunctionDecl, InterfaceImplementationDecl,
    NestedTypeDecl, PropertyDecl, RecoveredNominalMembers, Span, VariantDecl,
};

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredEnum {
    pub header: EnumHeader,
    pub variants: Vec<VariantDecl>,
    pub methods: Vec<FunctionDecl>,
    pub properties: Vec<PropertyDecl>,
    pub nested: Vec<NestedTypeDecl>,
    pub companion: Option<CompanionObjectDecl>,
    pub interface_implementations: Vec<InterfaceImplementationDecl>,
    pub recovery: RecoveredNominalMembers,
    pub span: Span,
}

impl RecoveredEnum {
    pub fn source(&self) -> EnumSource<'_> {
        EnumSource {
            header: &self.header,
            variants: &self.variants,
            methods: &self.methods,
            properties: &self.properties,
            nested: &self.nested,
            companion: self.companion.as_ref(),
            interface_implementations: &self.interface_implementations,
            recovery: &self.recovery,
            span: self.span,
        }
    }
}
