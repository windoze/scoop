use super::*;

impl ReferenceBuilder {
    pub(super) fn link_import_uses(
        &mut self,
        lowerer: &Lowerer,
        imports: &HashMap<hir::SemanticSourceLocation, ResolvedReferenceImport>,
        check: &Rc<dyn Fn() -> bool>,
    ) -> Result<(), EditingCancelled> {
        let mut positions = HashMap::<_, Vec<_>>::new();
        for (index, occurrence) in self.occurrences.iter().enumerate() {
            positions
                .entry(occurrence.origin.clone())
                .or_default()
                .push(index);
        }
        for (origin, sources) in std::mem::take(&mut self.import_uses) {
            if check() {
                return Err(EditingCancelled);
            }
            let Some(occurrences) = positions.get(&origin) else {
                continue;
            };
            let mut selected = Vec::new();
            for source in sources {
                match source {
                    hir::EditingImportSource::Declaration(source) => {
                        let Some(import) = imports.get(&source) else {
                            continue;
                        };
                        for candidate in &import.targets {
                            selected.push((
                                candidate.target,
                                candidate.routes.clone(),
                                import.alias,
                                Some(source.clone()),
                            ));
                        }
                    }
                    hir::EditingImportSource::Dependency {
                        import,
                        target,
                        routes,
                    } => {
                        if let Some(target) =
                            crate::editing::identities::DeclarationIdentities::imported_target(
                                lowerer, target,
                            )
                        {
                            selected.push((target, routes, None, import));
                        }
                    }
                }
            }
            for (target, routes, alias, import) in selected {
                let mut found = false;
                for &index in occurrences {
                    let occurrence = &mut self.occurrences[index];
                    if occurrence.target != target {
                        continue;
                    }
                    found = true;
                    if let Some(alias) = alias {
                        let definition = &self.aliases[alias].origin;
                        if !occurrence.aliases.contains(definition) {
                            occurrence.aliases.push(definition.clone());
                        }
                    }
                }
                if !found {
                    continue;
                }
                let aliases = alias
                    .map(|alias| self.aliases[alias].origin.clone())
                    .into_iter()
                    .collect::<Vec<_>>();
                self.export_reference(target, &origin, &routes, &aliases);
                if let Some(alias) = alias {
                    if matches!(target, Target::Type(_)) {
                        self.alias_constructor_names(alias, occurrences);
                    }
                    self.record(Target::ImportAlias(alias), origin.clone(), false);
                }
                let exports = self
                    .reexports
                    .iter()
                    .filter(|export| {
                        Some(&export.origin) == import.as_ref()
                            && export.target == target
                            && export.routes.iter().any(|route| routes.contains(route))
                    })
                    .map(|export| export.binding)
                    .collect::<Vec<_>>();
                for binding in exports {
                    self.record(Target::ExportBinding(binding), origin.clone(), false);
                }
            }
        }
        Ok(())
    }

    fn alias_constructor_names(&mut self, alias: hir::EditingImportAliasId, occurrences: &[usize]) {
        let alias = &mut self.aliases[alias];
        let mut targets = alias.targets.to_vec();
        for &index in occurrences {
            let occurrence = &mut self.occurrences[index];
            if !matches!(
                occurrence.target,
                Target::Callable(hir::EditingCallableIdentity::Constructor(_))
            ) {
                continue;
            }
            if !occurrence.aliases.contains(&alias.origin) {
                occurrence.aliases.push(alias.origin.clone());
            }
            if !targets.contains(&occurrence.target) {
                targets.push(occurrence.target);
            }
        }
        alias.targets =
            hir::NonEmptyVec::from_vec(targets).expect("an alias retains its original type target");
    }
}
