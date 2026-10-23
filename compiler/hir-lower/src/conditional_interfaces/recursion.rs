use super::*;

impl Lowerer {
    pub(super) fn conditional_query_repeats(
        &mut self,
        declaration: &hir::ConditionalInterfaceDeclaration,
        arguments: &[TypeId],
    ) -> bool {
        let previous = self
            .conditional_interfaces
            .active_queries
            .iter()
            .find(|(id, previous)| {
                *id == declaration.id()
                    && previous.len() == arguments.len()
                    && previous
                        .iter()
                        .zip(arguments)
                        .all(|(&previous, &argument)| self.query_type_embeds(previous, argument))
            });
        let Some((_, previous)) = previous else {
            return false;
        };
        let kind = if previous == arguments {
            "cyclic"
        } else {
            "expanding"
        };
        let interface = self.instantiate_ty(declaration.interface, arguments);
        self.conditional_interfaces.cycles.push(ConditionalProofCycle {
            interface,
            diagnostic: ast::Diagnostic::at_file(self.current_file, declaration.span, format!(
                "{kind} conditional interface proof: the implementation cannot establish its own condition"
            )),
        });
        true
    }

    pub(crate) fn begin_conditional_relation_query(&mut self) {
        self.conditional_interfaces.relation_depth += 1;
    }

    pub(crate) fn finish_conditional_relation_query(&mut self, proved: bool) {
        self.conditional_interfaces.relation_depth -= 1;
        if !self.conditional_query_is_complete() {
            return;
        }
        let cycles = std::mem::take(&mut self.conditional_interfaces.cycles);
        if proved {
            return;
        }
        self.diagnostics
            .extend(cycles.into_iter().map(|cycle| cycle.diagnostic));
    }

    pub(crate) fn finish_conditional_interface_set(&mut self, interfaces: &[TypeId]) {
        if !self.conditional_query_is_complete() {
            return;
        }
        let cycles = std::mem::take(&mut self.conditional_interfaces.cycles);
        for cycle in cycles {
            if !interfaces.contains(&cycle.interface) {
                self.diagnostics.push(cycle.diagnostic);
            }
        }
    }

    fn conditional_query_is_complete(&self) -> bool {
        self.conditional_interfaces.relation_depth == 0
            && self.conditional_interfaces.active_queries.is_empty()
    }

    /// Structural embedding detects an expanding revisit of the same rule;
    /// descending through a nested container remains legal.
    fn query_type_embeds(&self, smaller: TypeId, larger: TypeId) -> bool {
        if self.types_equal(smaller, larger) {
            return true;
        }
        let larger_children = self.query_type_children(larger);
        if larger_children
            .iter()
            .any(|&child| self.query_type_embeds(smaller, child))
        {
            return true;
        }
        let same_constructor = match (
            self.nominal_application(smaller),
            self.nominal_application(larger),
        ) {
            (Some(smaller), Some(larger)) => smaller.template == larger.template,
            (None, None) => match (&self.types[smaller], &self.types[larger]) {
                (Type::Tuple(_), Type::Tuple(_)) | (Type::Ptr(_), Type::Ptr(_)) => true,
                (Type::Function(a), Type::Function(b)) | (Type::FunPtr(a), Type::FunPtr(b)) => {
                    self.function_types[*a].is_suspend == self.function_types[*b].is_suspend
                }
                _ => false,
            },
            _ => false,
        };
        let smaller_children = self.query_type_children(smaller);
        same_constructor
            && smaller_children.len() == larger_children.len()
            && smaller_children
                .iter()
                .zip(&larger_children)
                .all(|(&a, &b)| self.query_type_embeds(a, b))
    }

    fn query_type_children(&self, ty: TypeId) -> Vec<TypeId> {
        if let Some(application) = self.nominal_application(ty) {
            return application.arguments;
        }
        match &self.types[ty] {
            Type::Tuple(children) => children.clone(),
            Type::Ptr(element) => vec![*element],
            Type::Function(signature) | Type::FunPtr(signature) => {
                let signature = &self.function_types[*signature];
                signature
                    .parameters()
                    .copied()
                    .chain([signature.return_type])
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}
