use super::*;

pub(super) struct SelectedConst {
    pub(super) definition: usize,
    pub(super) layer: Option<crate::imports::ImportLookupLayer>,
}

impl Lowerer {
    pub(super) fn find_qualified_const_definition(
        &self,
        declarations: &[PendingConst<'_>],
        qualifier: crate::NominalTarget,
        name: &str,
        file: usize,
    ) -> Option<SelectedConst> {
        let direct = match qualifier {
            crate::NominalTarget::Object(object) => self.find_const_definition(
                declarations,
                hir::PropertyOwner::Object(object),
                name,
                file,
                false,
            ),
            _ => None,
        };
        direct.or_else(|| {
            let companion = self.companion_object(qualifier.owner())?;
            self.find_const_definition(
                declarations,
                hir::PropertyOwner::Object(companion),
                name,
                file,
                false,
            )
        })
    }

    pub(super) fn find_const_definition(
        &self,
        declarations: &[PendingConst<'_>],
        owner: hir::PropertyOwner,
        name: &str,
        file: usize,
        fallback_to_top_level: bool,
    ) -> Option<SelectedConst> {
        let find = |wanted_owner| {
            if wanted_owner == hir::PropertyOwner::TopLevel {
                let (layer, result) = self.lookup_value_origin_with_layer(name);
                match result {
                    crate::imports::lookup::LookupResult::Unique(
                        crate::imports::lookup::values::ValueOrigin::CurrentUnit(binding),
                    ) => {
                        let crate::imports::CurrentUnitTarget::SourceProperty(source) =
                            self.imports.binding(binding).target
                        else {
                            return None;
                        };
                        let definition = declarations.iter().position(|candidate| matches!(candidate.import_source, crate::imports::PropertyImportSource::CurrentUnit(id) if id == source))?;
                        return Some(SelectedConst { definition, layer });
                    }
                    crate::imports::lookup::LookupResult::Missing
                    | crate::imports::lookup::LookupResult::Inaccessible(_)
                    | crate::imports::lookup::LookupResult::Unique(
                        crate::imports::lookup::values::ValueOrigin::Core(
                            crate::imports::lookup::values::ValueTarget::Property(_),
                        ),
                    ) => {}
                    _ => return None,
                }
                // Embedded prelude constants have no current-unit import ids and
                // are evaluated before their ordinary prelude properties exist.
                let candidates = declarations
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| {
                        (!self.source_is_current_cone(candidate.file)
                            && candidate.owner == wanted_owner
                            && candidate.declaration.name.text == name
                            && self.access_domain_allows(&candidate.access.lookup.0))
                        .then_some(index)
                    })
                    .collect::<Vec<_>>();
                return match candidates.as_slice() {
                    [one] => Some(SelectedConst {
                        definition: *one,
                        layer: None,
                    }),
                    _ => None,
                };
            }
            declarations
                .iter()
                .enumerate()
                .find_map(|(index, candidate)| {
                    (candidate.owner == wanted_owner
                        && candidate.declaration.name.text == name
                        && (candidate.access.declared != hir::DeclaredVisibility::Private
                            || candidate.file == file
                            || self.access_domain_allows(&candidate.access.lookup.0)))
                    .then_some(SelectedConst {
                        definition: index,
                        layer: None,
                    })
                })
        };
        find(owner).or_else(|| {
            (fallback_to_top_level && owner != hir::PropertyOwner::TopLevel)
                .then(|| find(hir::PropertyOwner::TopLevel))
                .flatten()
        })
    }

    pub(super) fn const_definition_name(&self, declaration: &PendingConst<'_>) -> String {
        match declaration.owner {
            hir::PropertyOwner::TopLevel => declaration.declaration.name.text.clone(),
            hir::PropertyOwner::Object(object) => format!(
                "{}.{}",
                crate::Owner::Object(object).describe_name(self),
                declaration.declaration.name.text
            ),
            _ => unreachable!("the const worklist contains only top-level and object values"),
        }
    }
}
