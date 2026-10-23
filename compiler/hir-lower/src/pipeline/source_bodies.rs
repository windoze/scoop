use super::*;

impl Lowerer {
    #[cfg(test)]
    pub(super) fn lower_source_bodies(
        &mut self,
        declarations: &SourceDeclarations<'_>,
        type_reference: Option<FunctionId>,
    ) {
        self.lower_source_bodies_with_cancellation(
            declarations,
            type_reference,
            &cancellation::uninterrupted,
        )
        .unwrap();
    }

    pub(super) fn lower_source_bodies_with_cancellation<C>(
        &mut self,
        declarations: &SourceDeclarations<'_>,
        type_reference: Option<FunctionId>,
        check: &impl Fn() -> Result<(), C>,
    ) -> Result<(), PhaseFailure<C>> {
        PhaseFailure::check(check)?;
        let SourceDeclarations {
            pending_functions,
            pending_methods,
            pending_classes,
            pending_objects,
            ..
        } = declarations;
        // Pass 3: lower bodies. Intrinsics have no body to lower (the
        // parser guarantees it is omitted); their `kind` was set at
        // declaration time. Constructor edges were resolved after source
        // parameter interfaces so delegation shares the ordinary call
        // protocol without depending on body declaration order.
        for &(id, decl, file_index) in pending_functions {
            PhaseFailure::check(check)?;
            if self.declaration_surface.unavailable_signature(id).is_some() {
                continue;
            }
            if self.has_inferred_function(id) {
                continue;
            }
            if matches!(
                self.functions[id].kind,
                FunctionKind::Intrinsic(_) | FunctionKind::Extern(_)
            ) {
                continue;
            }
            self.current_file = file_index;
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }
        for &(id, decl, file_index, _owner) in pending_methods {
            PhaseFailure::check(check)?;
            if self.declaration_surface.unavailable_signature(id).is_some() {
                continue;
            }
            if self.has_inferred_function(id) {
                continue;
            }
            // Abstract methods are bodyless; their parameter-only body was
            // built in pass 2.5. Interface methods with bodies are ordinary
            // default implementations and lower here.
            if matches!(self.functions[id].kind, FunctionKind::Intrinsic(_))
                || decl.modifier == ast::MethodModifier::Abstract
                || matches!(decl.body, ast::FunctionBody::None)
            {
                continue;
            }
            self.current_file = file_index;
            let body = self.lower_body(id, decl);
            self.functions[id].kind = FunctionKind::User(body);
        }
        self.lower_derived_encoding_bodies();
        PhaseFailure::check(check)?;
        self.lower_derived_decoding_bodies();
        PhaseFailure::check(check)?;
        self.lower_derived_inspection_bodies(type_reference);
        PhaseFailure::check(check)?;
        self.lower_interface_forwarder_bodies();
        PhaseFailure::check(check)?;
        self.lower_property_accessor_bodies();
        PhaseFailure::check(check)?;
        self.lower_release_blocks(pending_classes, pending_objects);
        PhaseFailure::check(check)?;
        self.finish_type_annotations();
        PhaseFailure::check(check)
    }
}
