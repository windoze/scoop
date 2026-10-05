//! Transparent ordinary-dependency type aliases and typed target selection.

use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;
use crate::imported_capabilities::ImportedCapabilityRequirement;
use crate::imported_core::ImportedSignatureTypeError;

impl Lowerer {
    pub(crate) fn resolve_imported_dependency_type_target(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        name: &ast::Ident,
        supplied_type_arguments: bool,
    ) -> Option<hir::TypeId> {
        match binding.target() {
            hir::ImportedTarget::Annotation(_) => {
                self.error(
                    name.span,
                    format!("annotation `{}` cannot be used as a value type", name.text),
                );
                None
            }
            hir::ImportedTarget::TypeAlias(_) => {
                self.resolve_imported_dependency_type_alias(binding, name, supplied_type_arguments)
            }
            hir::ImportedTarget::Type(declaration) => {
                let declaration = declaration.persistent();
                if let Ok(ty) = self.imported_signature_type(
                    &scoop_identity::SignatureTypeKey::Nominal(declaration),
                ) {
                    if supplied_type_arguments {
                        self.error(name.span, format!("type `{}` is not generic", name.text));
                        return None;
                    }
                    return Some(ty);
                }
                self.error(
                    name.span,
                    ImportedCapabilityRequirement::Layout
                        .diagnostic(&format!("dependency type `{}`", name.text)),
                );
                None
            }
            hir::ImportedTarget::GenericType(_) => {
                self.resolve_imported_nominal_type_arguments(binding, name, &[])
            }
            target => {
                self.error(
                    name.span,
                    format!(
                        "invalid dependency type binding `{}`: {target:?}",
                        name.text
                    ),
                );
                None
            }
        }
    }

    fn resolve_imported_dependency_type_alias(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        name: &ast::Ident,
        supplied_type_arguments: bool,
    ) -> Option<hir::TypeId> {
        if supplied_type_arguments {
            self.error(
                name.span,
                format!("typealias `{}` is not generic", name.text),
            );
            return None;
        }
        let candidate = match self
            .dependencies
            .as_ref()
            .expect("ordinary lowering carries a dependency selection plan")
            .type_alias_candidate(binding)
        {
            Ok(candidate) => candidate,
            Err(error) => {
                self.error(
                    name.span,
                    format!("invalid imported dependency typealias: {error}"),
                );
                return None;
            }
        };
        let ty = match self.imported_signature_type(candidate.expansion()) {
            Ok(ty) => ty,
            Err(ImportedSignatureTypeError::Generic) => {
                self.error(
                    name.span,
                    ImportedCapabilityRequirement::Generic
                        .diagnostic(&format!("dependency typealias `{}`", name.text)),
                );
                return None;
            }
            Err(ImportedSignatureTypeError::Structural) => {
                self.error(
                    name.span,
                    ImportedCapabilityRequirement::Layout
                        .diagnostic(&format!("dependency typealias `{}`", name.text)),
                );
                return None;
            }
        };
        if let Err(error) = self
            .dependencies
            .as_mut()
            .expect("ordinary lowering carries a dependency selection plan")
            .select_type_alias(candidate)
        {
            self.error(
                name.span,
                format!("failed to select imported dependency typealias: {error}"),
            );
            return None;
        }
        Some(ty)
    }
}
