use super::*;

impl Lowerer {
    /// Failure to prove a guard on an open application is not a negative fact.
    pub(crate) fn has_undetermined_conditional_interface(
        &mut self,
        subject: TypeId,
        target: TypeId,
    ) -> bool {
        let mut pending = vec![subject];
        let mut seen = std::collections::HashSet::new();
        while let Some(subject) = pending.pop() {
            if !seen.insert(subject) {
                continue;
            }
            let (declarations, arguments) = match self.types[subject] {
                Type::Class(application) => {
                    let application = self.class_applications[application].clone();
                    (
                        self.class_definition(application.template)
                            .conditional_interfaces
                            .clone(),
                        application.arguments,
                    )
                }
                Type::Struct(application) => {
                    let application = self.struct_applications[application].clone();
                    (
                        self.struct_definition(application.template)
                            .conditional_interfaces
                            .clone(),
                        application.arguments,
                    )
                }
                Type::Enum(application) => {
                    let application = self.enum_applications[application].clone();
                    (
                        self.enum_definition(application.template)
                            .conditional_interfaces
                            .clone(),
                        application.arguments,
                    )
                }
                _ => (Vec::new(), Vec::new()),
            };
            if arguments
                .iter()
                .any(|&argument| self.type_contains_param(argument))
            {
                for declaration in declarations {
                    // A missing companion on a fixed nominal makes this guard
                    // false even if another argument is still a parameter.
                    if declaration.parameters.iter().zip(&arguments).any(
                        |(parameter, &argument)| {
                            parameter.companion.is_some()
                                && !matches!(
                                    self.types[argument],
                                    Type::Param(_) | Type::Companion(_)
                                )
                                && self.projected_companion_type(argument).is_err()
                        },
                    ) {
                        continue;
                    }
                    let interface = self.instantiate_ty(declaration.interface, &arguments);
                    let mut closure = Vec::new();
                    self.append_interface_closure(interface, &mut closure);
                    if closure
                        .into_iter()
                        .any(|interface| self.open_types_may_equal(interface, target))
                    {
                        return true;
                    }
                }
            }
            pending.extend(self.direct_nominal_supertypes(subject));
        }
        false
    }
}
