use super::*;

impl Graph<'_> {
    pub(super) fn roots(&mut self) -> Result<(), Error> {
        let path = WirePath::root().field(8);
        let mut providers = Vec::new();
        scoop_wire::allocation::try_reserve(&mut providers, self.providers.len(), &path)?;
        providers.extend(
            self.providers
                .keys()
                .copied()
                .filter(|provider| *provider != self.current.provider),
        );
        self.calls()?;
        self.current
            .public
            .external_references()
            .validate_type_site_relations(
                self.current.provider,
                self.current.identities,
                &providers,
            )
            .map_err(|error| Error::TypeUseRelations(Box::new(error)))?;
        for (_, owner) in self
            .current
            .public
            .external_references()
            .materialized_shape_dependencies(self.current.provider, self.current.identities)
            .map_err(|error| Error::TypeUseRelations(Box::new(error)))?
        {
            self.select(owner, Kind::ShapeSupport)?;
        }
        for reference in self.current.public.external_references().records() {
            if reference.type_sites().is_empty() {
                continue;
            }
            let ExternalHirTargetV1::Nominal(SourceNominalId::Concrete(owner)) = reference.target()
            else {
                return Err(Error::NonConcreteSignature);
            };
            for site in reference.type_sites().records() {
                let kind = match site {
                    HirDependencyTypeSiteV1::CallableSignature { .. }
                    | HirDependencyTypeSiteV1::ConstructorInitializerResult { .. }
                    | HirDependencyTypeSiteV1::InitializationCycleMessage { .. } => Kind::Signature,
                    HirDependencyTypeSiteV1::Expression(expression)
                        if expression.role() == HirExpressionTypeRoleV1::TypeTest =>
                    {
                        Kind::TypeTest
                    }
                    HirDependencyTypeSiteV1::Expression(_)
                    | HirDependencyTypeSiteV1::LocalValue { .. }
                    | HirDependencyTypeSiteV1::BackingStorage { .. }
                    | HirDependencyTypeSiteV1::DelegateStorage { .. }
                    | HirDependencyTypeSiteV1::FieldStorage { .. }
                    | HirDependencyTypeSiteV1::EnumVariantFieldStorage { .. } => {
                        Kind::Representation
                    }
                };
                self.select(owner, kind)?;
            }
        }
        let sources = self.providers[&self.current.provider]
            .materialization
            .sources();
        let mut roots = Vec::new();
        scoop_wire::allocation::try_reserve(&mut roots, sources.len(), &path)?;
        roots.extend_from_slice(sources);
        for owner in roots {
            self.select(owner, Kind::Representation)?;
        }
        Ok(())
    }
}
