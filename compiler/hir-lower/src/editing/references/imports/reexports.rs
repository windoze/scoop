use super::*;

impl ReferenceBuilder {
    pub(super) fn reexports(
        &mut self,
        lowerer: &Lowerer,
        imports: &HashMap<hir::SemanticSourceLocation, ResolvedReferenceImport>,
        check: &Rc<dyn Fn() -> bool>,
    ) -> Result<(), EditingCancelled> {
        for binding in &lowerer.imports.reexports {
            if check() {
                return Err(EditingCancelled);
            }
            let Some(target) = crate::editing::identities::DeclarationIdentities::imported_target(
                lowerer,
                binding.target,
            ) else {
                continue;
            };
            for source in binding.origins.iter() {
                let origin = hir::SemanticSourceLocation {
                    source: source.source.clone(),
                    span: source.span,
                };
                let import = &imports[&origin];
                let name = import
                    .alias
                    .map(|id| self.aliases[id].origin.clone())
                    .or_else(|| import.name.clone());
                if let Some(alias) = import.alias {
                    let alias = &mut self.aliases[alias];
                    alias.exports.push(binding.identity.id());
                    let mut targets = alias.targets.to_vec();
                    targets.push(Target::ExportBinding(binding.identity.id()));
                    alias.targets = hir::NonEmptyVec::from_vec(targets)
                        .expect("an alias has a represented target");
                }
                if let Some(name) = &name {
                    self.record(
                        Target::ExportBinding(binding.identity.id()),
                        name.clone(),
                        true,
                    );
                }
                self.reexports.push(hir::EditingReexport {
                    binding: binding.identity.id(),
                    target,
                    origin,
                    name,
                    alias: import.alias,
                    routes: binding.routes.routes().to_vec(),
                });
            }
        }
        Ok(())
    }
}
