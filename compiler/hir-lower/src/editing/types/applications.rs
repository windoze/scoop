use super::*;
use hir::{EditingUnavailable as U, EditingValue as V};

impl Lowerer {
    pub(crate) fn observe_type_application(&mut self, syntax: &ast::TypeRef, ty: hir::TypeId) {
        let span = match &syntax.kind {
            ast::TypeRefKind::Named(name) | ast::TypeRefKind::Generic(name, _) => name.span,
            ast::TypeRefKind::Qualified { path, .. } => {
                path.last().expect("a type path is non-empty").span
            }
            ast::TypeRefKind::AppliedMember { name, .. } => name.span,
            ast::TypeRefKind::Unit => syntax.span,
            ast::TypeRefKind::Tuple(_)
            | ast::TypeRefKind::Function(_)
            | ast::TypeRefKind::Nullable(_) => return,
        };
        self.observe_type_application_name(span, ty);
    }

    pub(crate) fn observe_type_application_name(&mut self, span: ast::Span, ty: hir::TypeId) {
        if !self.observes_types() {
            return;
        }
        let name = self.type_name(ty);
        let observation = self.editing_types.as_mut().unwrap_or_else(|| {
            &mut self
                .editing_body
                .as_mut()
                .expect("body type observation")
                .types
        });
        observation
            .applications
            .insert((self.current_file, span), (ty, name));
    }
}

impl DeclarationAnalysis {
    pub fn type_definition(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<Option<hir::SemanticSourceLocation>, ast::Diagnostic> {
        match self.lowerer.types[ty] {
            hir::Type::Param(parameter) | hir::Type::Companion(parameter) => Ok(self
                .lowerer
                .editing_type_origin(TypeTarget::Parameter(parameter))),
            _ => self
                .lowerer
                .semantic_nominal_identity(ty)
                .map(|declaration| self.lowerer.semantic_nominal_origin(declaration))
                .transpose(),
        }
    }

    pub fn source_type_application(
        &mut self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<Option<hir::TypeId>>, EditingCancelled> {
        self.type_application(source, span, is_cancelled)
            .map(|result| match result {
                V::Known(application) => V::Known(application.map(|application| application.ty)),
                V::Unavailable(reason) => V::Unavailable(reason),
            })
    }

    pub(super) fn type_application(
        &mut self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<Option<hir::EditingTypeApplication>>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        if !self
            .lowerer
            .editing_declaration_scope_available(&hir::SemanticSourceLocation {
                source: source.clone(),
                span,
            })
        {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        if let Some(application) = self.type_applications.iter().find(|application| {
            &application.origin.source == source && application.origin.span == span
        }) {
            return Ok(V::Known(Some(application.clone())));
        }
        if self
            .type_references
            .iter()
            .any(|reference| &reference.origin.source == source && reference.origin.span == span)
        {
            return Ok(V::Known(None));
        }
        let check = std::rc::Rc::new(is_cancelled);
        let cancellation = check.clone();
        if let Some(body) = self.source_body_at(source, span, move || cancellation())? {
            let body = body.id;
            let check = check.clone();
            match self.source_body_facts(body, move || check())? {
                V::Known(facts) => {
                    if let Some(application) = facts.type_applications.iter().find(|application| {
                        &application.origin.source == source && application.origin.span == span
                    }) {
                        return Ok(V::Known(Some(application.clone())));
                    }
                }
                V::Unavailable(reason) => return Ok(V::Unavailable(reason)),
            }
        }
        Ok(V::Known(None))
    }
}
