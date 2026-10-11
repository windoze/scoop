use super::*;

impl Lowerer {
    pub(in crate::types) fn resolve_type_name_binding(
        &mut self,
        binding: TypeLookupTarget,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
        host: Option<TypeId>,
        subject: Option<TypeId>,
    ) -> Option<ResolvedTypeName> {
        match binding {
            TypeLookupTarget::Current(TopLevelTypeTarget::Annotation(id)) => {
                Some(ResolvedTypeName::Annotation(id))
            }
            TypeLookupTarget::Current(TopLevelTypeTarget::Nominal(target)) => Some(
                ResolvedTypeName::Nominal(self.nominal_identity(target.owner()).declaration_id()),
            ),
            TypeLookupTarget::Current(TopLevelTypeTarget::Alias(alias)) => {
                let alias = self.source_alias_template(alias, name)?;
                self.resolve_alias_type_name(alias, arguments, host, subject)
            }
            TypeLookupTarget::Dependency(binding) => {
                if let hir::ImportedTarget::Annotation(id) = binding.target() {
                    Some(ResolvedTypeName::Annotation(id.persistent()))
                } else if let Some(owner) = binding.target().source_nominal() {
                    Some(ResolvedTypeName::Nominal(owner))
                } else if matches!(binding.target(), hir::ImportedTarget::TypeAlias(_)) {
                    let alias = self.imported_alias_template(&binding, name)?;
                    self.resolve_alias_type_name(alias, arguments, host, subject)
                } else {
                    self.resolve_imported_dependency_type_target(
                        &binding,
                        name,
                        !arguments.is_empty(),
                    )
                    .map(ResolvedTypeName::Applied)
                }
            }
        }
    }

    pub(crate) fn resolve_nested_type_name(
        &mut self,
        owner: hir::SourceNominalId,
        name: &ast::Ident,
        arguments: &[ast::TypeRef],
        host: Option<TypeId>,
        subject: Option<TypeId>,
    ) -> Result<Option<ResolvedTypeName>, ()> {
        if let Some(owner) = self.nominal_owners.get(&owner).copied() {
            if let Some(alias) = self
                .nested_aliases_by_owner
                .get(&(owner, name.text.clone()))
                .copied()
            {
                let alias = self.source_alias_template(alias, name).ok_or(())?;
                return self
                    .resolve_alias_type_name(alias, arguments, host, subject)
                    .map(Some)
                    .ok_or(());
            }
            if let Some(id) = self
                .nested_annotations_by_owner
                .get(&(owner, name.text.clone()))
            {
                return Ok(Some(ResolvedTypeName::Annotation(*id)));
            }
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
                    arguments,
                    host,
                    subject,
                )
                .map(Some)
                .ok_or(()),
            _ => {
                self.error(name.span, format!("ambiguous nested type `{}`", name.text));
                Err(())
            }
        }
    }

    fn resolve_alias_type_name(
        &mut self,
        mut alias: crate::aliases::AliasTemplate,
        arguments: &[ast::TypeRef],
        host: Option<TypeId>,
        subject: Option<TypeId>,
    ) -> Option<ResolvedTypeName> {
        alias.host = host;
        let ty = if let Some(subject) = subject.filter(|_| alias.parameterized()) {
            self.infer_alias_target(&alias, &[], Some(subject), alias.name.span)?
                .0
        } else {
            self.apply_alias_template(&alias, &alias.name, arguments, host)?
        };
        Some(ResolvedTypeName::Applied(ty))
    }
}
