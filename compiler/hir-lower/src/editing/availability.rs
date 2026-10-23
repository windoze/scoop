//! Declaration failures follow already resolved source type dependencies.

use super::*;
use crate::Owner;
use types::TypeTarget as Target;

#[derive(Clone, Default)]
pub(crate) struct DeclarationAvailability {
    current: Option<Target>,
    dependents: imbl::HashMap<Target, imbl::HashSet<Target>>,
    failed: imbl::HashSet<Target>,
    failed_sources: imbl::HashSet<hir::SemanticSourceLocation>,
}

pub(crate) struct DeclarationCheck {
    target: Target,
    previous: Option<Target>,
    diagnostics: usize,
}

impl DeclarationAvailability {
    fn dependency(&mut self, dependent: Target, dependency: Target) {
        if !matches!(dependency, Target::Nominal(_) | Target::Alias(_)) {
            return;
        }
        self.dependents
            .entry(dependency)
            .or_default()
            .insert(dependent);
        if self.failed.contains(&dependency) {
            self.fail(dependent);
        }
    }

    fn fail(&mut self, target: Target) {
        let mut pending = vec![target];
        while let Some(target) = pending.pop() {
            if self.failed.insert(target).is_none()
                && let Some(dependents) = self.dependents.get(&target)
            {
                pending.extend(dependents.iter().copied());
            }
        }
    }
}

impl Lowerer {
    pub(crate) fn begin_editing_declaration(&mut self, owner: Owner) -> Option<DeclarationCheck> {
        self.editing_declarations.as_ref()?;
        let target = Target::Nominal(self.nominal_identity(owner).declaration_id());
        let parent = self
            .nominal_parent(owner)
            .map(|parent| Target::Nominal(self.nominal_identity(parent).declaration_id()));
        self.begin_editing_type_check(target, parent)
    }

    pub(crate) fn begin_editing_alias(
        &mut self,
        id: crate::aliases::SourceTypeAliasId,
    ) -> Option<DeclarationCheck> {
        self.editing_declarations.as_ref()?;
        let parent = self.source_type_aliases[id]
            .owner
            .map(|owner| Target::Nominal(self.nominal_identity(owner).declaration_id()));
        self.begin_editing_type_check(Target::Alias(id), parent)
    }

    fn begin_editing_type_check(
        &mut self,
        target: Target,
        parent: Option<Target>,
    ) -> Option<DeclarationCheck> {
        let state = self
            .editing_declarations
            .as_mut()
            .expect("editing declarations");
        if let Some(parent) = parent {
            state.dependency(target, parent);
        }
        Some(DeclarationCheck {
            target,
            previous: state.current.replace(target),
            diagnostics: self.diagnostics.len(),
        })
    }

    pub(crate) fn finish_editing_declaration(&mut self, check: Option<DeclarationCheck>) {
        let Some(check) = check else {
            return;
        };
        let state = self
            .editing_declarations
            .as_mut()
            .expect("editing declarations");
        state.current = check.previous;
        self.record_editing_type_errors(check.target, check.diagnostics);
    }

    pub(crate) fn record_editing_type_errors(&mut self, target: Target, start: usize) {
        if let Some(state) = &mut self.editing_declarations
            && self.diagnostics[start..]
                .iter()
                .any(|diagnostic| diagnostic.severity == ast::DiagnosticSeverity::Error)
        {
            state.fail(target);
        }
    }

    pub(crate) fn current_editing_declaration(&self) -> Option<Target> {
        let state = self.editing_declarations.as_ref()?;
        self.type_alias_resolution_stack
            .last()
            .copied()
            .map(Target::Alias)
            .or(state.current)
    }

    pub(crate) fn observe_declaration_type_dependency(&mut self, target: Target) {
        if let Some(dependent) = self.current_editing_declaration()
            && let Some(state) = &mut self.editing_declarations
        {
            state.dependency(dependent, target);
        }
    }

    pub(crate) fn fail_editing_owner(&mut self, owner: Owner) {
        if self.editing_declarations.is_some() {
            self.fail_editing_type_declaration(Target::Nominal(
                self.nominal_identity(owner).declaration_id(),
            ));
        }
    }

    pub(crate) fn record_editing_owner_errors(&mut self, owner: Owner, start: usize) {
        if self.editing_declarations.is_some() {
            self.record_editing_type_errors(
                Target::Nominal(self.nominal_identity(owner).declaration_id()),
                start,
            );
        }
    }

    pub(crate) fn fail_editing_type_declaration(&mut self, target: Target) {
        if let Some(state) = &mut self.editing_declarations {
            state.fail(target);
        }
    }

    pub(crate) fn fail_editing_source_declaration(&mut self, span: ast::Span) {
        if self.editing_declarations.is_none() {
            return;
        }
        let origin = hir::SemanticSourceLocation {
            source: self.source_identity(self.current_file).clone(),
            span,
        };
        self.editing_declarations
            .as_mut()
            .expect("editing declarations")
            .failed_sources
            .insert(origin);
    }

    pub(crate) fn editing_type_available(&self, target: Target) -> bool {
        self.editing_declarations
            .as_ref()
            .is_none_or(|state| !state.failed.contains(&target))
    }

    pub(crate) fn editing_owner_available(&self, owner: Owner) -> bool {
        self.editing_declarations.is_none()
            || self.editing_type_available(Target::Nominal(
                self.nominal_identity(owner).declaration_id(),
            ))
    }

    pub(crate) fn require_editing_type(&mut self, target: Target, span: ast::Span) -> Option<()> {
        if self.editing_type_available(target) {
            return Some(());
        }
        self.error(
            span,
            "type declaration is unavailable because its signature failed".into(),
        );
        None
    }

    pub(crate) fn failed_editing_declarations(
        &self,
    ) -> impl Iterator<Item = hir::SemanticSourceLocation> + '_ {
        self.editing_declarations
            .as_ref()
            .into_iter()
            .flat_map(|state| state.failed.iter())
            .filter_map(|target| match target {
                Target::Nominal(id) => {
                    let owner = *self.nominal_owners.get(id)?;
                    let span = match owner {
                        Owner::Class(id) => self.classes[id].span,
                        Owner::Struct(id) => self.structs[id].span,
                        Owner::Enum(id) => self.enums[id].span,
                        Owner::Interface(id) => self.interfaces[id].span,
                        Owner::Object(id) => self.objects[id].span,
                    };
                    Some(hir::SemanticSourceLocation {
                        source: self
                            .source_identity(self.nominal_definition_file(owner)?)
                            .clone(),
                        span,
                    })
                }
                Target::Alias(id) => {
                    let alias = &self.source_type_aliases[*id];
                    Some(hir::SemanticSourceLocation {
                        source: self.editing_type_origin(*target)?.source,
                        span: alias.origin.span,
                    })
                }
                _ => None,
            })
            .chain(
                self.editing_declarations
                    .as_ref()
                    .into_iter()
                    .flat_map(|state| state.failed_sources.iter().cloned()),
            )
    }

    pub(crate) fn editing_declaration_scope_available(
        &self,
        origin: &hir::SemanticSourceLocation,
    ) -> bool {
        self.failed_editing_declarations().all(|failed| {
            failed.source != origin.source
                || failed.span.start > origin.span.start
                || failed.span.end < origin.span.end
        })
    }
}
