use super::*;

impl Lowerer {
    pub(crate) fn request_imported_closure(
        &mut self,
        parent: scoop_identity::CallableTemplateOwner,
        body: scoop_identity::PersistentGeneratedCallableId,
        role: scoop_identity::LexicalCallableRole,
        capture_bindings: Vec<hir::BindingId>,
    ) -> Result<hir::ImportedGenericCallableTemplateId, String> {
        if let Some(&id) = self.imported_generic_templates.closures.get(&body) {
            return Ok(id);
        }
        let source = self
            .dependencies
            .as_ref()
            .expect("an imported closure retains its dependency catalog")
            .callable_body(hir::DefaultCallableDeclarationV1::Generated(body))
            .ok_or("dependency closure is missing its implementation")?;
        let origin = hir::ImportedCallableTemplateOrigin::Closure {
            parent,
            body,
            capture_bindings,
        };
        let name = match role {
            scoop_identity::LexicalCallableRole::LambdaBody => "$lambda",
            scoop_identity::LexicalCallableRole::AnonymousFunctionBody => "$anonymous",
        };
        let prepared = self.prepare_imported_lexical_callable(
            origin,
            name.into(),
            PreparedImportedCallableSource::Body(source),
        )?;
        let id = self.allocate_imported_template(prepared);
        self.imported_generic_templates.closures.insert(body, id);
        Ok(id)
    }
}
