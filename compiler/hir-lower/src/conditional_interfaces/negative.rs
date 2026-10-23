use super::*;

impl Lowerer {
    /// Failure to prove a guard on an open application is not a negative fact.
    pub(crate) fn has_undetermined_conditional_interface(
        &mut self,
        subject: TypeId,
        target: TypeId,
    ) -> bool {
        if self
            .conditional_interfaces
            .negative_queries
            .iter()
            .any(|&(previous, interface)| {
                self.query_type_embeds(previous, subject)
                    && self.query_type_embeds(interface, target)
            })
        {
            return true;
        }
        self.conditional_interfaces
            .negative_queries
            .push((subject, target));
        let result = self.find_undetermined_conditional_interface(subject, target);
        self.conditional_interfaces.negative_queries.pop();
        result
    }

    fn find_undetermined_conditional_interface(&mut self, subject: TypeId, target: TypeId) -> bool {
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
                    if self
                        .conditional_parameters_are_impossible(&declaration.parameters, &arguments)
                    {
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
