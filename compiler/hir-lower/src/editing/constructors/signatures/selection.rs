use super::*;
use hir::{
    EditingConstructorDeclaration as Declaration, EditingUnavailable as U, EditingValue as V,
};

impl DeclarationAnalysis {
    pub fn constructor_declaration_origin(
        &self,
        declaration: Declaration,
    ) -> Option<hir::SemanticSourceLocation> {
        match declaration {
            Declaration::Constructor(callee) => self.lowerer.editing_callee_origin(callee),
            Declaration::Parameter { callee, index } => {
                self.lowerer.editing_callee_parameter_origin(callee, index)
            }
        }
    }

    pub fn constructor_declaration_at(
        &mut self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<Option<Declaration>>, EditingCancelled> {
        let check = std::rc::Rc::new(is_cancelled);
        if check() {
            return Err(EditingCancelled);
        }
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        let cancellation = check.clone();
        if let Some(body) = self.source_body_at(source, span, move || cancellation())? {
            let cancellation = check.clone();
            if let V::Unavailable(reason) =
                self.source_body_facts(body.id, move || cancellation())?
            {
                return Ok(V::Unavailable(reason));
            }
        }
        let origin = hir::SemanticSourceLocation {
            source: source.clone(),
            span,
        };
        if let Some((callee, index)) = self.lowerer.editing_parameter_at_origin(&origin)
            && callee.callable().is_none()
        {
            return Ok(V::Known(Some(Declaration::Parameter { callee, index })));
        }
        for (callee, _) in self.lowerer.editing_source_constructors() {
            if check() {
                return Err(EditingCancelled);
            }
            if self.lowerer.editing_callee_origin(callee).as_ref() == Some(&origin)
                && self.declaration_scope_available(&origin)
            {
                return Ok(V::Known(Some(Declaration::Constructor(callee))));
            }
        }
        let mut selected = None;
        for binding in self.import_bindings(source, span) {
            if check() {
                return Err(EditingCancelled);
            }
            let Some(callee) = self.lowerer.editing_import_variant(binding) else {
                continue;
            };
            if selected.is_some_and(|previous| previous != callee) {
                return Ok(V::Known(None));
            }
            selected = Some(callee);
        }
        Ok(V::Known(selected.map(Declaration::Constructor)))
    }
}
