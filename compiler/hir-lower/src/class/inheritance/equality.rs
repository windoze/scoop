use super::*;

impl Lowerer {
    pub(super) fn check_equality_declaration(
        &mut self,
        function: FunctionId,
        declaration: &ast::FunctionDecl,
        owner: Owner,
    ) -> bool {
        if self.functions[function].modifiers.operator != Some(hir::OperatorKind::Equals) {
            return true;
        }
        if self
            .equality_core
            .is_some_and(|core| self.interface_method_entities[core.equals].function == function)
        {
            return true;
        }
        let parameters = self.signatures[&function].params.clone();
        let applicable = self.owner_interfaces(owner).into_iter().any(|interface| {
            if !self.is_equality_interface(interface) {
                return false;
            }
            let Type::Interface(application) = self.types[interface] else {
                unreachable!("an Equality application is an interface")
            };
            let other = self.interface_applications[application].arguments[0];
            matches!(parameters.as_slice(), [parameter] if self.types_equal(parameter.ty, other))
        });
        if !applicable {
            self.error(
                declaration.name.span,
                "operator `equals` requires an explicit or inherited core Equality contract for its parameter type".to_string(),
            );
        }
        applicable
    }
}
