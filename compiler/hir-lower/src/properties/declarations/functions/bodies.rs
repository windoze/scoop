use super::*;

impl Lowerer {
    pub(crate) fn lower_property_accessor_bodies(&mut self) {
        self.lower_property_accessor_bodies_from(0);
    }

    pub(crate) fn lower_property_accessor_bodies_from(&mut self, start: usize) {
        let sources = self.property_accessor_sources[start..].to_vec();
        for source in sources {
            if self.property_accessor_was_lowered(source.function) {
                continue;
            }
            if self.functions[source.function]
                .method
                .is_some_and(|method| method.modifier == hir::MethodModifier::Abstract)
            {
                continue;
            }
            self.lower_property_accessor_body(&source);
        }
    }

    pub(crate) fn lower_property_accessor_body(&mut self, source: &PropertyAccessorSource) {
        self.current_file = self.function_files[&source.function];
        let previous = self.backing_field_context.take();
        self.backing_field_context = source.backing.map(|backing| BackingFieldContext {
            backing,
            capture_depth: self.capture_contexts.len(),
        });
        let mut body = if source.body_kind == PropertyAccessorBodyKind::Delegate {
            self.lower_generated_delegate_accessor(source)
        } else {
            self.lower_body(source.function, &source.declaration)
        };
        self.prepend_accessor_initialization_ensure(source, &mut body);
        self.functions[source.function].kind = FunctionKind::User(body);
        self.backing_field_context = previous;
    }

    pub(crate) fn prepend_accessor_initialization_ensure(
        &mut self,
        source: &PropertyAccessorSource,
        body: &mut hir::Body,
    ) {
        let Some(&unit) = self.runtime_accessor_units.get(&source.function) else {
            return;
        };
        let kind = match self.initialization_units[unit].kind {
            hir::InitializationUnitKind::GenericDelegatedExtension { template, .. } => {
                hir::StatementKind::GenericDelegateEnsure(
                    self.generic_delegate_reference(template, source.function),
                )
            }
            hir::InitializationUnitKind::EagerTopLevel { .. }
            | hir::InitializationUnitKind::LazySingleton { .. } => {
                hir::StatementKind::InitializationEnsure(unit)
            }
        };
        body.statements.insert(
            0,
            hir::Statement {
                kind,
                span: source.declaration.span,
            },
        );
    }
}
