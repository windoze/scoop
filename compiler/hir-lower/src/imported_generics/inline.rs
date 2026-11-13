use super::*;

impl Lowerer {
    pub(super) fn prepare_inline_templates(&mut self, first_use: usize) -> Result<(), String> {
        let declarations = self
            .imported_dependency_callables
            .iter()
            .skip(first_use)
            .filter_map(|(_, use_)| {
                let declaration = use_.reference().declaration();
                if use_.requires_provider_entry()
                    || !matches!(
                        declaration,
                        CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::Accessor(_)
                    )
                    || self
                        .imported_generic_templates
                        .by_declaration
                        .contains_key(&declaration)
                {
                    return None;
                }
                let declaration = self
                    .dependencies
                    .as_ref()?
                    .callable_declaration(declaration)
                    .ok()?;
                declaration
                    .callable_body()
                    .is_some_and(|body| body.type_parameters().arguments().is_empty())
                    .then_some((declaration.interface().declaration(), declaration))
            })
            .collect::<BTreeMap<_, _>>();
        for (key, declaration) in declarations {
            if matches!(key, CallableTemplateOrigin::Accessor(_))
                && requires_provider_storage(declaration.callable_body().expect("selected body"))?
            {
                for (_, use_) in self.imported_dependency_callables.iter_mut() {
                    if use_.reference().declaration() == key {
                        use_.require_provider_entry();
                    }
                }
                continue;
            }
            let origin = match key {
                CallableTemplateOrigin::Function(owner) => {
                    hir::ImportedCallableTemplateOrigin::Inline(owner)
                }
                CallableTemplateOrigin::Accessor(owner) => {
                    hir::ImportedCallableTemplateOrigin::InlineAccessor(owner)
                }
                _ => unreachable!("ordinary inline declarations were selected above"),
            };
            let prepared = self.prepare_imported_generic(declaration, origin)?;
            self.insert_imported_template(key, prepared);
        }
        Ok(())
    }
}

// Ordinary property reads in a template use the provider accessor contract.
// An accessor reading its own backing slot cannot use that substitution.
fn requires_provider_storage(body: &hir::ExportGenericCallableBodyV1) -> Result<bool, String> {
    struct Storage(bool);
    impl<'body> hir::DefaultBodyReferenceVisitorV1<'body> for Storage {
        type Error = scoop_wire::WireError;
        fn expression(
            &mut self,
            _: u32,
            _: &'body hir::DefaultExpressionV1,
            _: &scoop_wire::WirePath,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        fn reference(
            &mut self,
            reference: hir::DefaultBodyReferenceOccurrenceV1<'body>,
            _: &scoop_wire::WirePath,
        ) -> Result<(), Self::Error> {
            self.0 |= matches!(
                reference.target,
                hir::DefaultBodyReferenceTargetV1::Global(_)
            );
            Ok(())
        }
    }
    if body.statements().iter().any(|statement| {
        matches!(
            statement.kind(),
            hir::DefaultStatementKindV1::InitializationEnsure(_)
        )
    }) {
        return Ok(true);
    }
    let mut storage = Storage(false);
    body.visit_direct_references(&mut storage, &scoop_wire::WirePath::root())
        .map_err(|error| error.to_string())?;
    Ok(storage.0)
}
