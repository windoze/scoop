use super::*;

impl DeclarationAnalysis {
    pub fn member_implementation_targets(
        &mut self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<Vec<Member>>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        let mut identities = MemberIndex::default();
        if let Some(function) = self.functions.iter().find(|function| {
            &function.name_origin.source == source && function.name_origin.span == span
        }) {
            return Ok(V::Known(
                identities
                    .current_member(&mut self.lowerer, function.function)
                    .into_iter()
                    .collect(),
            ));
        }
        if self
            .type_references
            .iter()
            .any(|reference| &reference.origin.source == source && reference.origin.span == span)
        {
            return Ok(V::Known(Vec::new()));
        }
        let property = self
            .lowerer
            .property_name_spans
            .iter()
            .find_map(|(&property, &name)| {
                (name == span
                    && self
                        .lowerer
                        .property_definition_file(property)
                        .is_some_and(|file| self.lowerer.source_identity(file) == source))
                .then_some(property)
            });
        if let Some(property) = property {
            return Ok(V::Known(
                identities
                    .current_property(&mut self.lowerer, property)
                    .into_iter()
                    .collect(),
            ));
        }
        let imports = self
            .lowerer
            .imports
            .files
            .values()
            .flat_map(|file| &file.exact)
            .filter(|import| {
                &import.origin.source == source
                    && (import.target_span == span
                        || import.alias.as_ref().is_some_and(|name| name.span == span))
            })
            .flat_map(|import| import.targets.iter().cloned())
            .collect::<Vec<_>>();
        let mut targets = Vec::new();
        for import in imports {
            if is_cancelled() {
                return Err(EditingCancelled);
            }
            let target = match self.lowerer.editing_import_member(&import) {
                Some(hir::EditingMemberTarget::Callable(callable)) => {
                    identities.selected_callable(&mut self.lowerer, callable)
                }
                Some(hir::EditingMemberTarget::Property(property)) => {
                    identities.current_property(&mut self.lowerer, property)
                }
                Some(hir::EditingMemberTarget::ImportedProperty(property)) => identities
                    .selected_property(
                        &mut self.lowerer,
                        hir::SemanticProperty::Imported(property),
                    ),
                None => None,
            };
            if let Some(target) = target
                && !targets.contains(&target)
            {
                targets.push(target);
            }
        }
        if !targets.is_empty() {
            return Ok(V::Known(targets));
        }
        let check = std::rc::Rc::new(is_cancelled);
        let functions = self
            .functions
            .iter()
            .filter(|function| {
                &function.name_origin.source == source
                    && function.span.start <= span.start
                    && span.end <= function.span.end
            })
            .map(|function| function.function)
            .collect::<Vec<_>>();
        for function in functions {
            let cancellation = check.clone();
            let (callables, properties) =
                match self.function_facts(function, move || cancellation())? {
                    V::Known(facts) => (
                        facts
                            .callables
                            .iter()
                            .filter(|reference| reference.span == span)
                            .filter_map(|reference| reference.target.callable())
                            .collect::<Vec<_>>(),
                        facts
                            .properties
                            .iter()
                            .filter(|reference| reference.span == span)
                            .map(|reference| reference.target)
                            .collect::<Vec<_>>(),
                    ),
                    V::Unavailable(reason) => return Ok(V::Unavailable(reason)),
                };
            for callable in callables {
                if let Some(target) = identities.selected_callable(&mut self.lowerer, callable)
                    && !targets.contains(&target)
                {
                    targets.push(target);
                }
            }
            for property in properties {
                if let Some(target) = identities.selected_property(&mut self.lowerer, property)
                    && !targets.contains(&target)
                {
                    targets.push(target);
                }
            }
        }
        if check() {
            return Err(EditingCancelled);
        }
        Ok(V::Known(targets))
    }
}

impl MemberIndex {
    fn selected_callable(
        &mut self,
        lowerer: &mut Lowerer,
        callable: hir::SemanticCallable,
    ) -> Option<Member> {
        match callable {
            hir::SemanticCallable::Current(function) => self.current_member(lowerer, function),
            hir::SemanticCallable::Imported(declaration) => {
                let origin = match declaration {
                    hir::DefaultCallableDeclarationV1::Function(id) => Imported::Function(id),
                    hir::DefaultCallableDeclarationV1::GenericFunction(id) => {
                        Imported::GenericFunction(id)
                    }
                    hir::DefaultCallableDeclarationV1::PropertyAccessor(id) => {
                        Imported::Accessor(id)
                    }
                    hir::DefaultCallableDeclarationV1::Generated(_) => return None,
                };
                self.imported_member(lowerer, origin)
            }
            hir::SemanticCallable::CurrentGetter(_) | hir::SemanticCallable::CurrentSetter(_) => {
                None
            }
        }
    }

    fn selected_property(
        &mut self,
        lowerer: &mut Lowerer,
        property: hir::SemanticProperty,
    ) -> Option<Member> {
        match property {
            hir::SemanticProperty::Current(property) => self.current_property(lowerer, property),
            hir::SemanticProperty::Imported(property) => {
                let declaration = lowerer
                    .dependencies
                    .as_ref()?
                    .property_declaration(property)?;
                matches!(
                    declaration.owner(),
                    hir::PublicDeclarationOwnerV1::Nominal(_)
                )
                .then_some(Member::Property(property))
            }
        }
    }
}
