use super::*;

impl Lowerer {
    pub(super) fn lower_release_body(
        &mut self,
        class: ClassId,
        block: &ast::ReleaseBlock,
    ) -> (hir::Body, hir::DefinitionOrigin) {
        let outer_source_context = self.current_source_context;
        let outer_paths = std::mem::take(&mut self.definition_paths);
        let outer_root = self.definition_root.take();
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_owner = self.current_owner.replace(Owner::Class(class));
        let outer_this = self.current_this.take();
        let outer_return = self.current_return_ty;
        let outer_name = std::mem::take(&mut self.current_fn_name);
        let outer_parameters = std::mem::replace(
            &mut self.type_params_in_scope,
            self.classes[class].type_params.clone(),
        );
        self.current_release = Some(class);
        self.current_return_ty = self.unit;
        self.set_source_context(hir::SourceContextSubject::Nominal(
            hir::SourceContextNominal::Class(class),
        ));
        let origin = self.definition_origin(block.span);
        self.push_suspension_context(SuspensionContext::Forbidden(
            ForbiddenSuspendContext::Release,
        ));
        self.push_safety_context(hir::Safety::Safe);
        let statements = self.lower_block(&block.body);
        self.pop_safety_context();
        self.pop_suspension_context();
        let locals = std::mem::replace(&mut self.locals, outer_locals);
        self.current_release = None;
        self.current_owner = outer_owner;
        self.current_this = outer_this;
        self.current_source_context = outer_source_context;
        self.current_return_ty = outer_return;
        self.current_fn_name = outer_name;
        self.definition_paths = outer_paths;
        self.definition_root = outer_root;
        self.type_params_in_scope = outer_parameters;
        (hir::Body { locals, statements }, origin)
    }
}
