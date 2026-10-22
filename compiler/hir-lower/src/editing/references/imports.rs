use super::*;
use crate::imports::ResolvedImportExposure;

struct ResolvedReferenceImport {
    targets: Vec<Target>,
    alias: Option<hir::EditingImportAliasId>,
}

impl ReferenceBuilder {
    pub(super) fn imports(
        &mut self,
        lowerer: &mut Lowerer,
        check: &Rc<dyn Fn() -> bool>,
    ) -> Result<(), EditingCancelled> {
        let declarations = lowerer
            .imports
            .files
            .iter()
            .flat_map(|file| file.exact.iter().cloned())
            .collect::<Vec<_>>();
        let mut imports = HashMap::new();
        for import in declarations {
            if check() {
                return Err(EditingCancelled);
            }
            let origin = hir::SemanticSourceLocation {
                source: import.origin.source.clone(),
                span: import.origin.span,
            };
            let name_origin = hir::SemanticSourceLocation {
                span: import.target_span,
                ..origin.clone()
            };
            let mut complete = true;
            let mut targets = Vec::new();
            for binding in import.targets.iter() {
                if check() {
                    return Err(EditingCancelled);
                }
                if let Some(target) = self.identities.import_target(lowerer, binding) {
                    if !targets.contains(&target) {
                        targets.push(target);
                        self.record(target, name_origin.clone(), false);
                    }
                } else {
                    complete = false;
                }
            }
            if !complete {
                self.unavailable.push(name_origin);
            }
            let alias = import.alias.filter(|_| complete).map(|name| {
                let alias_origin = hir::SemanticSourceLocation {
                    span: name.span,
                    ..origin.clone()
                };
                let id = self.aliases.alloc(hir::EditingImportAlias {
                    name: name.text,
                    origin: alias_origin.clone(),
                    targets: hir::NonEmptyVec::from_vec(targets.clone())
                        .expect("a complete import retains its non-empty target set"),
                    reexported: import.exposure == ResolvedImportExposure::PublicReexport,
                });
                self.record(Target::ImportAlias(id), alias_origin, true);
                id
            });
            imports.insert(origin, ResolvedReferenceImport { targets, alias });
        }
        self.link_import_uses(&imports, check)
    }

    fn link_import_uses(
        &mut self,
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
            let mut aliases = Vec::new();
            for &index in occurrences {
                let occurrence = &mut self.occurrences[index];
                for source in &sources {
                    let Some(import) = imports.get(source) else {
                        continue;
                    };
                    if !import.targets.contains(&occurrence.target) {
                        continue;
                    }
                    if let Some(alias) = import.alias {
                        let definition = &self.aliases[alias].origin;
                        if !occurrence.aliases.contains(definition) {
                            occurrence.aliases.push(definition.clone());
                        }
                        if !aliases.contains(&alias) {
                            aliases.push(alias);
                        }
                    }
                }
            }
            for alias in aliases {
                self.record(Target::ImportAlias(alias), origin.clone(), false);
            }
        }
        Ok(())
    }
}
