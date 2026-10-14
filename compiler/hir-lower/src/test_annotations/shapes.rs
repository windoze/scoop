use super::*;

impl Lowerer {
    pub(super) fn valid_test_container(&self, class: hir::ClassId) -> bool {
        let declaration = &self.classes[class];
        let Some(identity) = self.nominal_identity(Owner::Class(class)).source() else {
            return false;
        };
        let identity = identity.declaration();
        declaration.owner.is_none()
            && !matches!(
                identity.scope(),
                scoop_identity::DeclarationScope::LexicalScoped { .. }
            )
            && declaration.type_params.is_empty()
            && declaration.modifier != hir::ClassModifier::Abstract
            && matches!(declaration.inheritance_family, hir::InheritanceFamily::Open)
            && declaration.is_declared()
    }

    pub(super) fn test_method_rejection(&self, function: hir::FunctionId) -> Option<&'static str> {
        let declaration = &self.functions[function];
        if declaration.method.is_none() {
            return Some("missing instance receiver");
        }
        if self.extension_receivers.contains_key(&function) {
            return Some("extension receiver is not allowed");
        }
        if !declaration.context_parameters.is_empty() {
            return Some("context parameters are not allowed");
        }
        if declaration.params.len() != 1 {
            return Some("value parameters are not allowed, including defaulted parameters");
        }
        if !matches!(declaration.genericity, hir::FunctionGenericity::Plain) {
            return Some("type parameters are not allowed");
        }
        if declaration.is_suspend {
            return Some("suspend is not allowed");
        }
        if declaration.return_ty != self.unit {
            return Some("return type must be Unit");
        }
        if declaration.attributes.safety != hir::Safety::Safe {
            return Some("unsafe calls are not allowed");
        }
        if !matches!(declaration.kind, hir::FunctionKind::User(_)) {
            return Some("a Scoop function body is required");
        }
        None
    }
}
