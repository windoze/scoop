use super::*;

impl Lowerer {
    pub(super) fn check_override_access_coverage(
        &mut self,
        id: FunctionId,
        decl: &MemberOverride,
        inherited: &[(FunctionId, Vec<TypeId>)],
    ) {
        let Some(mut provided) = self.functions[id].access.slot.clone() else {
            unreachable!("an overriding method always owns a slot contract")
        };
        if self.functions[id].access.declared == hir::DeclaredVisibility::Protected
            && self.functions[inherited[0].0].access.declared == hir::DeclaredVisibility::Protected
            && let Some(required) = self.functions[inherited[0].0].access.slot.clone()
        {
            // `protected override` preserves the inherited protected region;
            // anchoring it at the concrete subclass would incorrectly narrow
            // access for sibling descendants of the original declaring class.
            provided = required;
            self.functions[id].access.slot = Some(provided.clone());
        }

        for (candidate, _) in inherited {
            let Some(required) = self.functions[*candidate].access.slot.clone() else {
                continue;
            };
            if !self.access_domain_is_subset(&required.0, &provided.0) {
                let target = self.functions[*candidate].name.clone();
                self.error(
                    decl.name.span,
                    format!(
                        "visibility of `{}` does not cover inherited slot `{target}`",
                        decl.name.text
                    ),
                );
                continue;
            }
        }
    }

    pub(in crate::class::inheritance) fn check_member_access_contract(
        &mut self,
        id: FunctionId,
        decl: &MemberOverride,
        owner: Owner,
    ) {
        let access = self.functions[id].access.clone();
        let modifier = self.functions[id]
            .method
            .expect("inheritance checks receive member functions")
            .modifier;

        if access.declared == hir::DeclaredVisibility::Private
            && modifier != hir::MethodModifier::Final
        {
            self.error(
                decl.name.span,
                format!("private method `{}` must be final", decl.name.text),
            );
        }

        if let Owner::Interface(interface) = owner {
            if self.interfaces[interface].access.declared == hir::DeclaredVisibility::Public
                && !matches!(
                    access.declared,
                    hir::DeclaredVisibility::Public | hir::DeclaredVisibility::Private
                )
            {
                self.error(
                    decl.name.span,
                    format!(
                        "member `{}` of public interface `{}` must be explicitly public",
                        decl.name.text, self.interfaces[interface].name
                    ),
                );
            }
            if access.declared == hir::DeclaredVisibility::Private && decl.is_override {
                self.error(
                    decl.name.span,
                    format!(
                        "private interface method `{}` cannot be an override",
                        decl.name.text
                    ),
                );
            }
            return;
        }

        if modifier != hir::MethodModifier::Abstract {
            return;
        }
        let Owner::Class(class) = owner else {
            return;
        };
        let required = self.classes[class].access.inheritance.0.clone();
        let Some(provided) = access.slot else {
            unreachable!("an abstract method always has a slot contract")
        };
        if !self.access_domain_is_subset(&required, &provided.0) {
            self.error(
                decl.name.span,
                format!(
                    "abstract method `{}` is not visible throughout the inheritance domain of class `{}`",
                    decl.name.text, self.classes[class].name
                ),
            );
        }
    }
}
