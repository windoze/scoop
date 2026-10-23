use crate::*;

/// Bound signatures are available independently of queued implementations.
pub trait ImportedTemplateSignatures {
    fn callable_signature(
        &self,
        template: ImportedGenericCallableTemplateId,
    ) -> &ImportedGenericCallableSignature;

    fn constructor_signature(
        &self,
        template: ImportedConstructorTemplateId,
    ) -> &ImportedConstructorSignature;
}

impl ImportedTemplateSignatures for Module {
    fn callable_signature(
        &self,
        template: ImportedGenericCallableTemplateId,
    ) -> &ImportedGenericCallableSignature {
        &self.imported_generic_templates[template].signature
    }

    fn constructor_signature(
        &self,
        template: ImportedConstructorTemplateId,
    ) -> &ImportedConstructorSignature {
        &self.imported_constructor_templates[template].signature
    }
}
