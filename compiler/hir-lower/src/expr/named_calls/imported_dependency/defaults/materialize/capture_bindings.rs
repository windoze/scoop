use super::*;
use crate::defaults::{DefaultScopeRoot, PendingDefaultLocalScope};
use scoop_identity::{CallableTemplateOwner, StructuralDefinitionPath};

impl Lowerer {
    pub(super) fn materialize_capture_binding(
        &mut self,
        capture: &hir::DefaultCaptureV1,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::BindingId, ImportedDefaultMaterializationError> {
        match capture.binding() {
            hir::DefaultCaptureBindingV1::Source => {
                self.imported_closure_capture_binding(capture.source(), context)
            }
            hir::DefaultCaptureBindingV1::Definition {
                owner,
                scope,
                selector,
                definition,
            } => {
                let definition = match definition {
                    hir::TemplateLocalDefinitionV1::Source(source) => {
                        hir::LocalValueDefinitionSite::Source(
                            self.imported_default_definition_origin(source, context)?,
                        )
                    }
                    hir::TemplateLocalDefinitionV1::Synthetic => {
                        hir::LocalValueDefinitionSite::Synthetic
                    }
                };
                Ok(self.loaded_default_local_binding(*owner, scope, selector, definition))
            }
        }
    }

    pub(super) fn loaded_default_local_binding(
        &mut self,
        root: CallableTemplateOwner,
        path: &StructuralDefinitionPath,
        selector: &LocalValueSelector,
        definition: hir::LocalValueDefinitionSite,
    ) -> hir::BindingId {
        let scope = self
            .default_local_value_scopes
            .iter()
            .find_map(|(id, scope)| {
                (matches!(scope.definition_root, DefaultScopeRoot::Resolved(owner) if owner == root)
                    && scope.definition_path == *path)
                    .then_some(id)
            });
        let scope = scope.unwrap_or_else(|| {
            self.default_local_value_scopes
                .alloc(PendingDefaultLocalScope {
                    definition_root: DefaultScopeRoot::Resolved(root),
                    definition_path: path.clone(),
                    values: Vec::new(),
                })
        });
        if let Some(value) = self.default_local_value_scopes[scope]
            .values
            .iter()
            .find(|value| value.selector == *selector)
        {
            return value.binding;
        }
        let binding = self.fresh_binding();
        self.default_local_value_scopes[scope]
            .values
            .push(hir::DefaultLocalValueDefinition {
                binding,
                selector: selector.clone(),
                definition,
            });
        binding
    }
}
