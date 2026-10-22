//! Source queries read the ordinary frozen import selections.

use super::*;
use crate::imports::lookup::values::{ValueOrigin, ValueTarget};
use crate::imports::{CurrentUnitTarget, ImportedTargetBinding};
use hir::EditingMemberTarget as Member;

impl DeclarationAnalysis {
    pub(super) fn import_bindings<'a>(
        &'a self,
        source: &'a scoop_identity::SourceIdentity,
        span: ast::Span,
    ) -> impl Iterator<Item = &'a ImportedTargetBinding> {
        self.lowerer
            .imports
            .files
            .values()
            .flat_map(|file| &file.exact)
            .filter(move |import| {
                &import.origin.source == source
                    && (import.target_span == span
                        || import.alias.as_ref().is_some_and(|name| name.span == span))
            })
            .flat_map(|import| import.targets.iter())
    }

    pub fn import_definitions(
        &self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<Vec<hir::SemanticSourceLocation>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        let mut definitions = Vec::new();
        for binding in self.import_bindings(source, span) {
            if is_cancelled() {
                return Err(EditingCancelled);
            }
            let origin = match self.lowerer.editing_import_member(binding) {
                Some(Member::Callable(callable)) => self.lowerer.editing_callable_origin(callable),
                Some(Member::Property(property)) => self
                    .lowerer
                    .editing_property_origin(hir::SemanticProperty::Current(property)),
                Some(Member::ImportedProperty(property)) => self
                    .lowerer
                    .editing_property_origin(hir::SemanticProperty::Imported(property)),
                None => self
                    .lowerer
                    .editing_import_type_target(binding)
                    .and_then(|target| self.lowerer.editing_type_origin(target)),
            };
            if let Some(origin) = origin
                && !definitions.contains(&origin)
            {
                definitions.push(origin);
            }
        }
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        Ok(definitions)
    }
}

impl Lowerer {
    pub(super) fn editing_import_member(&self, binding: &ImportedTargetBinding) -> Option<Member> {
        Some(match binding {
            ImportedTargetBinding::CurrentCone { binding, .. } => {
                match self.imports.binding(*binding).target {
                    CurrentUnitTarget::Function(function) => {
                        Member::Callable(hir::SemanticCallable::Current(function))
                    }
                    CurrentUnitTarget::Property(_) | CurrentUnitTarget::SourceProperty(_) => {
                        let ValueTarget::Property(property) =
                            self.materialized_value_target(&ValueOrigin::CurrentUnit(*binding))?
                        else {
                            return None;
                        };
                        Member::Property(property)
                    }
                    _ => return None,
                }
            }
            ImportedTargetBinding::DirectDependency(binding) => {
                use hir::{DefaultCallableDeclarationV1 as D, ImportedTarget as T};
                match binding.target() {
                    T::Function(id) => Member::Callable(hir::SemanticCallable::Imported(
                        D::Function(id.persistent()),
                    )),
                    T::GenericFunction(id) => Member::Callable(hir::SemanticCallable::Imported(
                        D::GenericFunction(id.persistent()),
                    )),
                    T::Property(id) => Member::ImportedProperty(
                        hir::PropertyDeclarationId::Property(id.persistent()),
                    ),
                    T::ExtensionProperty(id) => Member::ImportedProperty(
                        hir::PropertyDeclarationId::ExtensionProperty(id.persistent()),
                    ),
                    _ => return None,
                }
            }
        })
    }
}
