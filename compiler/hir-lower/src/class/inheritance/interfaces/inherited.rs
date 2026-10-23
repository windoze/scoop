use super::*;

impl Lowerer {
    pub(super) fn inherited_conditional_interface_domains(
        &mut self,
        owner: Owner,
    ) -> Vec<(TypeId, hir::ConditionalInterfaceApplication)> {
        let class = match owner {
            Owner::Class(class) => class,
            Owner::Object(object) => self.objects[object].backing_class,
            _ => return Vec::new(),
        };
        let Some(base) = self.classes[class].base_class else {
            return Vec::new();
        };
        let Type::Class(base) = self.types[base] else {
            unreachable!("a checked class base has a class application")
        };
        let base = self.class_applications[base].clone();
        let inherited = self
            .class_definition(base.template)
            .interface_implementations
            .clone();
        inherited
            .into_iter()
            .filter_map(|implementation| {
                let mut condition = implementation.condition?;
                condition.arguments = condition
                    .arguments
                    .into_iter()
                    .map(|argument| self.instantiate_ty(argument, &base.arguments))
                    .collect();
                if self.conditional_application_is_impossible(&condition) {
                    return None;
                }
                Some((
                    self.instantiate_ty(implementation.interface, &base.arguments),
                    condition,
                ))
            })
            .collect()
    }

    pub(super) fn check_inherited_conditional_relisting(
        &mut self,
        owner: Owner,
        unconditional: &[TypeId],
        inherited: &[(TypeId, hir::ConditionalInterfaceApplication)],
    ) -> Vec<(hir::ConditionalInterfaceId, TypeId)> {
        let mut relisted = Vec::new();
        for block in self.owner_conditional_interfaces(owner).to_vec() {
            let condition = hir::ConditionalInterfaceApplication {
                declaration: block.id(),
                owner: block.key.owner,
                arguments: self.owner_type_args(owner),
            };
            for &(interface, ref parent) in inherited {
                if !self.open_types_may_equal(block.interface, interface)
                    || self.conditional_domains_are_disjoint(&condition, parent)
                {
                    continue;
                }
                let equivalent = self.types_equal(block.interface, interface)
                    && self.with_conditional_interface_assumptions(parent, |state| {
                        state.conditional_application_holds(&condition)
                    })
                    && self.with_conditional_interface_assumptions(&condition, |state| {
                        state.conditional_application_holds(parent)
                    });
                if equivalent {
                    relisted.push((block.id(), interface));
                } else {
                    self.error(block.span, format!(
                        "condition for inherited interface `{}` must be equivalent to its original implementation condition",
                        self.type_name(interface),
                    ));
                }
            }
            for &interface in unconditional {
                if !self.open_types_may_equal(block.interface, interface) {
                    continue;
                }
                if self.types_equal(block.interface, interface)
                    && self.conditional_application_holds(&condition)
                {
                    relisted.push((block.id(), interface));
                } else if !self.conditional_application_is_impossible(&condition) {
                    self.error(block.span, format!(
                        "condition for inherited interface `{}` must cover its unconditional implementation",
                        self.type_name(interface),
                    ));
                }
            }
        }
        relisted
    }

    fn conditional_domains_are_disjoint(
        &mut self,
        left: &hir::ConditionalInterfaceApplication,
        right: &hir::ConditionalInterfaceApplication,
    ) -> bool {
        self.with_conditional_interface_assumptions(left, |state| {
            state.conditional_application_is_impossible(right)
        }) || self.with_conditional_interface_assumptions(right, |state| {
            state.conditional_application_is_impossible(left)
        })
    }
}
