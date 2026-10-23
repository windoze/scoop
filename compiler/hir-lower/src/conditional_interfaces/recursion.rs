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
            .position(|previous| {
                previous.declaration == declaration.id()
                    && previous.arguments.len() == arguments.len()
                    && previous
                        .arguments
                        .iter()
                        .zip(arguments)
                        .all(|(&previous, &argument)| self.query_type_embeds(previous, argument))
            });
        let Some(start) = previous else {
            return false;
        };
        let kind = if self.conditional_interfaces.active_queries[start].arguments == arguments {
            "cyclic"
        } else {
            "expanding"
        };
        let interface = self.instantiate_ty(declaration.interface, arguments);
        let mut path = self.conditional_interfaces.active_queries[start..].to_vec();
        path.push(hir::ConditionalInterfaceApplication {
            declaration: declaration.id(),
            owner: declaration.key.owner,
            arguments: arguments.to_vec(),
        });
        let path = path
            .iter()
            .map(|query| self.conditional_query_label(query))
            .collect::<Vec<_>>()
            .join(" -> ");
        let message = format!(
            "{kind} conditional interface proof: the implementation cannot establish its own condition ({path})"
        );
        let diagnostic = self.conditional_query_diagnostic(declaration, message);
        self.conditional_interfaces
            .cycles
            .push(ConditionalProofCycle {
                interface,
                diagnostic,
            });
        true
    }

    fn conditional_query_label(&mut self, query: &hir::ConditionalInterfaceApplication) -> String {
        let name = match self.nominal_owners.get(&query.owner) {
            Some(owner) => owner.describe_name(self),
            None => self
                .dependencies
                .as_ref()
                .and_then(|world| world.nominal_declaration(query.owner))
                .expect("a conditional query retains its owner")
                .name()
                .to_owned(),
        };
        let arguments = query
            .arguments
            .iter()
            .map(|&argument| self.type_name(argument))
            .collect::<Vec<_>>()
            .join(", ");
        let declaration = self
            .declaration_conditional_interfaces(query.owner)
            .iter()
            .find(|declaration| declaration.id() == query.declaration)
            .expect("an active conditional query retains its declaration");
        let interface = self.instantiate_ty(declaration.interface, &query.arguments);
        format!("{name}<{arguments}> : {}", self.type_name(interface))
    }

    fn conditional_query_diagnostic(
        &self,
        declaration: &hir::ConditionalInterfaceDeclaration,
        message: String,
    ) -> ast::Diagnostic {
        if let Some(owner) = self.nominal_owners.get(&declaration.key.owner) {
            let file = match owner {
                Owner::Class(id) => self.class_files[id],
                Owner::Struct(id) => self.struct_files[id],
                Owner::Enum(id) => self.enum_files[id],
                Owner::Object(id) => self.object_files[id],
                Owner::Interface(id) => self.interface_files[id],
            };
            return ast::Diagnostic::at_file(file, declaration.span, message);
        }
        let origin = self
            .dependencies
            .as_ref()
            .and_then(|world| world.nominal_declaration(declaration.key.owner))
            .expect("an imported conditional declaration retains its nominal origin");
        let mut diagnostic = ast::Diagnostic::at_file(self.current_file, declaration.span, message);
        diagnostic.source = Some(Box::new(origin.origin.origin().source().clone()));
        diagnostic
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
        for cycle in cycles {
            self.report_conditional_cycle(cycle.diagnostic);
        }
    }

    pub(crate) fn finish_conditional_interface_set(&mut self, interfaces: &[TypeId]) {
        if !self.conditional_query_is_complete() {
            return;
        }
        let cycles = std::mem::take(&mut self.conditional_interfaces.cycles);
        for cycle in cycles {
            if !interfaces.contains(&cycle.interface) {
                self.report_conditional_cycle(cycle.diagnostic);
            }
        }
    }

    fn report_conditional_cycle(&mut self, diagnostic: ast::Diagnostic) {
        let repeated = self.diagnostics.iter().any(|previous| {
            previous.source == diagnostic.source
                && (diagnostic.source.is_some() || previous.file == diagnostic.file)
                && previous.span == diagnostic.span
                && previous.message == diagnostic.message
        });
        if !repeated {
            self.diagnostics.push(diagnostic);
        }
    }

    fn conditional_query_is_complete(&self) -> bool {
        self.conditional_interfaces.relation_depth == 0
            && self.conditional_interfaces.active_queries.is_empty()
    }

    /// Structural embedding detects an expanding revisit of the same rule;
    /// descending through a nested container remains legal.
    pub(super) fn query_type_embeds(&self, smaller: TypeId, larger: TypeId) -> bool {
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
