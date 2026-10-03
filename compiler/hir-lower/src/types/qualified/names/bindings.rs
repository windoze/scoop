use super::*;

impl Lowerer {
    pub(in crate::types) fn resolve_type_name_binding(
        &mut self,
        binding: TypeLookupTarget,
        name: &ast::Ident,
        supplied_type_arguments: bool,
    ) -> Option<ResolvedTypeName> {
        match binding {
            TypeLookupTarget::Current(TopLevelTypeTarget::Nominal(target)) => Some(
                ResolvedTypeName::Nominal(self.nominal_identity(target.owner()).declaration_id()),
            ),
            TypeLookupTarget::Current(TopLevelTypeTarget::Alias(alias)) => self
                .resolve_type_alias_id_reference(alias, name, supplied_type_arguments)
                .map(ResolvedTypeName::Alias),
            TypeLookupTarget::Dependency(binding) => {
                if let Some(owner) = binding.target().source_nominal() {
                    Some(ResolvedTypeName::Nominal(owner))
                } else {
                    self.resolve_imported_dependency_type_target(
                        &binding,
                        name,
                        supplied_type_arguments,
                    )
                    .map(ResolvedTypeName::Alias)
                }
            }
        }
    }

    pub(super) fn resolve_nested_type_name(
        &mut self,
        owner: hir::SourceNominalId,
        name: &ast::Ident,
        supplied_type_arguments: bool,
    ) -> Result<Option<ResolvedTypeName>, ()> {
        if let Some(owner) = self.nominal_owners.get(&owner).copied() {
            return Ok(self.nested_nominal_target(owner, &name.text).map(|target| {
                ResolvedTypeName::Nominal(self.nominal_identity(target.owner()).declaration_id())
            }));
        }
        let dependencies = self
            .dependencies
            .as_ref()
            .expect("an external namespace retains its declaration provider");
        let bindings = dependencies
            .static_bindings(owner, scoop_identity::BindingNamespace::Type, &name.text)
            .to_vec();
        match bindings.as_slice() {
            [] => Ok(dependencies
                .nested_nominal(owner, &name.text)
                .map(|declaration| ResolvedTypeName::Nominal(declaration.owner()))),
            [binding] => self
                .resolve_type_name_binding(
                    TypeLookupTarget::Dependency(binding.clone()),
                    name,
                    supplied_type_arguments,
                )
                .map(Some)
                .ok_or(()),
            _ => {
                self.error(name.span, format!("ambiguous nested type `{}`", name.text));
                Err(())
            }
        }
    }
}
