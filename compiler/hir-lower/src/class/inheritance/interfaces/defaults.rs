//! Defaults from independent domains must agree wherever both domains apply.

use super::*;

impl Lowerer {
    pub(super) fn check_conditional_default_conflicts(
        &mut self,
        span: ast::Span,
        host: &str,
        conformances: &[hir::InterfaceImplementation],
    ) {
        let members = conformances
            .iter()
            .map(|conformance| self.conformance_members(conformance.interface))
            .collect::<Vec<_>>();
        let mut reported = HashSet::new();
        for (index, left) in conformances.iter().enumerate() {
            for (right, right_members) in
                conformances[index + 1..].iter().zip(&members[index + 1..])
            {
                if left.condition == right.condition
                    || match (&left.condition, &right.condition) {
                        (Some(left), Some(right)) => {
                            self.conditional_domains_are_disjoint(left, right)
                        }
                        _ => false,
                    }
                {
                    continue;
                }
                for left_method in &left.methods {
                    let left_member = members[index]
                        .iter()
                        .find(|member| member.member == left_method.member)
                        .expect("a checked implementation retains its interface slot");
                    for right_method in &right.methods {
                        if left_method.target == right_method.target {
                            continue;
                        }
                        let right_member = right_members
                            .iter()
                            .find(|member| member.member == right_method.member)
                            .expect("a checked implementation retains its interface slot");
                        if !self.conditional_default_signatures_may_match(
                            &left_member.signature,
                            &right_member.signature,
                        ) {
                            continue;
                        }
                        let left_owner = self.interface_implementation_owner(left_method.target);
                        let right_owner = self.interface_implementation_owner(right_method.target);
                        // Ordinary member duplication and class override coverage
                        // already diagnose competing class implementations.
                        if !matches!(self.types[left_owner], Type::Interface(_))
                            && !matches!(self.types[right_owner], Type::Interface(_))
                        {
                            continue;
                        }
                        if matches!(self.types[left_owner], Type::Interface(_))
                            && matches!(self.types[right_owner], Type::Interface(_))
                            && (self.open_types_may_equal(left_owner, right_owner)
                                || self.is_subtype(left_owner, right_owner)
                                || self.is_subtype(right_owner, left_owner))
                        {
                            continue;
                        }
                        let name = &left_member.signature.name;
                        if reported.insert((name.clone(), left_owner, right_owner)) {
                            self.error(span, format!(
                                "{host} may inherit conflicting implementations for `{name}` from `{}` and `{}` when their conditions both hold; declare a common override that covers both conditions",
                                self.type_name(left_owner), self.type_name(right_owner),
                            ));
                        }
                    }
                }
            }
        }
    }

    fn conditional_default_signatures_may_match(
        &self,
        left: &InterfaceSignature,
        right: &InterfaceSignature,
    ) -> bool {
        left.name == right.name
            && left.accessor == right.accessor
            && left.parameters.len() == right.parameters.len()
            && left
                .parameters
                .iter()
                .zip(&right.parameters)
                .all(|(&left, &right)| self.open_types_may_equal(left, right))
    }

    fn interface_implementation_owner(&self, target: hir::InterfaceImplementationTarget) -> TypeId {
        match target {
            hir::InterfaceImplementationTarget::Method(application)
            | hir::InterfaceImplementationTarget::Abstract(application) => {
                self.method_owner_type(self.method_applications[application].owner)
            }
            hir::InterfaceImplementationTarget::Imported(callable)
            | hir::InterfaceImplementationTarget::ImportedAbstract(callable) => self
                .imported_dependency_callables[callable]
                .receiver()
                .expect("an interface implementation is a member"),
            hir::InterfaceImplementationTarget::ImportedTemplate(application)
            | hir::InterfaceImplementationTarget::ImportedAbstractTemplate(application) => {
                let hir::ImportedCallableArguments::Method { owner, .. } =
                    self.imported_generic_applications[application].arguments
                else {
                    unreachable!("an interface implementation is a member")
                };
                owner
            }
        }
    }
}
