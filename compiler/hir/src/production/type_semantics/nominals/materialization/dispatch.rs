use super::*;

impl Closure<'_, '_> {
    pub(super) fn dispatch(&mut self, export: &ExportHir) -> Result<(), Error> {
        self.sequence(export.functions.len())?;
        for (_, function) in export.functions.iter() {
            let Some(method) = function.method else {
                continue;
            };
            if matches!(method.dispatch, MethodDispatch::Direct) {
                continue;
            }
            let identity = export
                .type_identities
                .get(method.owner)
                .ok_or(Error::MissingExactIdentity)?;
            let Some(exact) = identity.exact() else {
                // An unbound generic owner has no nominal machine root.
                continue;
            };
            inheritance::source_resources::work(self.meter, self.roots.len())?;
            let Some(&owner) = self.roots.get(&exact.id()) else {
                continue;
            };
            if function.is_suspend
                || !matches!(function.genericity, FunctionGenericity::Plain)
                || matches!(function.kind, FunctionKind::Extern(_))
            {
                self.block(owner)?;
                continue;
            }
            self.sequence(function.params.len().saturating_add(1))?;
            for ty in function
                .params
                .iter()
                .map(|parameter| parameter.ty)
                .chain(std::iter::once(function.return_ty))
            {
                let exact = export
                    .type_identities
                    .get(ty)
                    .and_then(HirTypeIdentity::exact)
                    .ok_or(Error::MissingExactIdentity)?;
                self.require(owner, exact.id())?;
            }
        }
        Ok(())
    }
}
