//! Transparent ordinary-dependency type aliases and their route witnesses.

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
                    self.retain_imported_alias_target_bindings(
                        binding,
                        hir::ExternalHirTargetV1::Nominal(
                            scoop_identity::NominalDeclarationOwner::Concrete(declaration),
                        ),
                    );
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
                self.error(
                    name.span,
                    ImportedCapabilityRequirement::Generic
                        .diagnostic(&format!("dependency generic type `{}`", name.text)),
                );
                None
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
        let witness_target = hir::ExternalHirTargetV1::TypeAlias(candidate.interface().alias());
        let alias_binding = candidate.binding().clone();
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
        self.retain_imported_alias_target_bindings(&alias_binding, witness_target);
        Some(ty)
    }

    pub(crate) fn retain_imported_alias_target_bindings(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        target: hir::ExternalHirTargetV1,
    ) {
        for alias in self.type_alias_resolution_stack.iter().copied() {
            self.type_alias_binding_witnesses
                .entry(alias)
                .or_default()
                .extend(binding.sources().map(|source| {
                    hir::ExternalHirBindingWitnessUse::new(
                        target,
                        hir::ExternalHirBindingWitnessRole::AliasTarget,
                        source.witness().dependency().clone(),
                    )
                }));
        }
    }
}
