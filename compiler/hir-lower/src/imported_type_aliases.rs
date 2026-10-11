//! Transparent ordinary-dependency type aliases and typed target selection.

use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;
use crate::imported_capabilities::ImportedCapabilityRequirement;

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
        self.apply_imported_type_alias(binding, name, &[], None)
    }

    pub(crate) fn apply_imported_type_alias(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
        host: Option<hir::TypeId>,
    ) -> Option<hir::TypeId> {
        let template = self.imported_alias_template(binding, name)?;
        self.apply_alias_template(&template, name, arguments, host)
    }

    pub(crate) fn imported_alias_template(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        name: &ast::Ident,
    ) -> Option<crate::aliases::AliasTemplate> {
        let hir::ImportedTarget::TypeAlias(alias) = binding.target() else {
            return None;
        };
        if let Some(template) = self.imported_alias_templates.get(&alias.persistent()) {
            return Some(template.clone());
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
        let parameters = candidate
            .interface()
            .type_parameters()
            .binders()
            .iter()
            .collect::<Vec<_>>();
        let signatures = (0..parameters.len())
            .map(|index| scoop_identity::SignatureTypeKey::Binder {
                depth: 0,
                index: index as u32,
            })
            .collect::<Vec<_>>();
        let (parameters, bindings) =
            match self.prepare_imported_type_parameters(&parameters, &signatures, name.span) {
                Ok(result) => result,
                Err(error) => {
                    self.error(
                        name.span,
                        format!("invalid imported typealias parameters: {error}"),
                    );
                    return None;
                }
            };
        let target =
            match self.imported_signature_type_with_bindings(candidate.expansion(), &bindings) {
                Ok(target) => target,
                Err(error) => {
                    self.error(
                        name.span,
                        format!("invalid imported typealias target: {error:?}"),
                    );
                    return None;
                }
            };
        let template = crate::aliases::AliasTemplate {
            origin: crate::aliases::AliasOrigin::Imported(Box::new(binding.clone())),
            name: name.clone(),
            host: None,
            arguments: None,
            parameters,
            outer_parameter_count: candidate.interface().owner_parameter_count() as usize,
            target,
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
        self.imported_alias_templates
            .insert(alias.persistent(), template.clone());
        Some(template)
    }
}
