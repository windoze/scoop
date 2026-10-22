use super::*;

impl Lowerer {
    /// A missing core exception type aborts value lowering; source failures
    /// remain local to their catch clause.
    pub(crate) fn resolve_catch_type(
        &mut self,
        source: &ast::TypeRef,
        span: Span,
        mut previous: impl Iterator<Item = TypeId>,
    ) -> Result<Option<TypeId>, ()> {
        let Some(ty) = self.resolve_type_ref(source) else {
            return Ok(None);
        };
        let throwable = self.throwable_ty(source.span).ok_or(())?;
        if !self.is_subtype(ty, throwable) {
            let found = self.type_name(ty);
            self.error(
                source.span,
                format!("catch parameter type {found} is not a subtype of Throwable"),
            );
            return Ok(None);
        }
        if previous.any(|earlier| self.is_subtype(ty, earlier)) {
            let found = self.type_name(ty);
            self.error(
                span,
                format!("unreachable catch block: {found} is already covered by an earlier catch"),
            );
            return Ok(None);
        }
        Ok(Some(ty))
    }

    pub(crate) fn with_catch_scope<T>(
        &mut self,
        name: &ast::Ident,
        local: hir::LocalId,
        query: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.push_scope();
        self.scopes.declare(name.text.clone(), local);
        let result = query(self);
        self.pop_scope();
        result
    }
}
