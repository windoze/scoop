use super::*;

impl Lowerer {
    pub(crate) fn validate_program_arguments(&mut self) -> Option<FunctionId> {
        const NAME: &str = "__scoopProgramArguments";
        let candidates = self
            .top_level
            .iter()
            .copied()
            .filter(|function| {
                self.source_function_declarations
                    .get(function)
                    .is_some_and(|declaration| declaration.name == NAME)
            })
            .collect::<Vec<_>>();
        let [function] = candidates.as_slice() else {
            self.current_file = self.core_diagnostic_file();
            self.error(
                Span::new(0, 0),
                format!(
                    "scoop.core must define exactly one internal function `{NAME}(): Array<String>`"
                ),
            );
            return None;
        };
        let function = *function;
        self.current_file = self.function_files[&function];
        let declaration = &self.functions[function];
        let signature = &self.signatures[&function];
        let valid_result = self
            .array_type_info(signature.return_ty)
            .is_some_and(|array| {
                array.kind == crate::types::ArrayKind::Immutable && array.element == self.string
            });
        if declaration.access.declared != hir::DeclaredVisibility::Internal
            || !matches!(declaration.genericity, hir::FunctionGenericity::Plain)
            || declaration.is_suspend
            || declaration.method.is_some()
            || !signature.params.is_empty()
            || !valid_result
            || signature.attributes.gc_effect != hir::GcEffect::Managed
            || !matches!(declaration.kind, hir::FunctionKind::User(_))
        {
            self.error(declaration.span, format!("function `{NAME}` in scoop.core must be internal, non-generic, non-suspend, managed, and have signature `() -> Array<String>`"));
            return None;
        }
        Some(function)
    }
}
