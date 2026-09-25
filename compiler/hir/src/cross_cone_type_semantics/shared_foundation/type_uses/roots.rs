use super::*;

impl Graph<'_> {
    pub(super) fn roots(&mut self, meter: &mut BudgetMeter) -> Result<(), Error> {
        let path = WirePath::root().field(8);
        let mut providers = Vec::new();
        meter.try_reserve_collection_slots(&mut providers, self.providers.len(), &path)?;
        providers.extend(
            self.providers
                .keys()
                .copied()
                .filter(|provider| *provider != self.current.provider),
        );
        self.current
            .public
            .external_references()
            .validate_type_site_relations(
                self.current.provider,
                self.current.identities,
                &providers,
                meter,
            )
            .map_err(|error| Error::TypeUseRelations(Box::new(error)))?;
        for reference in self.current.public.external_references().records() {
            meter.charge_work(1, &path)?;
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
                self.select(owner, kind, meter)?;
                if let HirDependencyTypeSiteV1::Expression(expression) = site
                    && matches!(
                        expression.role(),
                        HirExpressionTypeRoleV1::TypeTest | HirExpressionTypeRoleV1::BoxedValue
                    )
                {
                    self.shape_operation(expression, owner, meter)?;
                }
            }
        }
        let sources = self.providers[&self.current.provider]
            .materialization
            .sources();
        let mut roots = Vec::new();
        meter.try_reserve_collection_slots(&mut roots, sources.len(), &path)?;
        roots.extend_from_slice(sources);
        for owner in roots {
            self.select(owner, Kind::Representation, meter)?;
        }
        Ok(())
    }

    fn shape_operation(
        &mut self,
        expression: &crate::HirExpressionTypeSiteV1,
        owner: PersistentTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        meter.charge_work(
            1 + u64::from(self.current.identities.identity_count().max(1).ilog2()),
            &WirePath::root().field(8),
        )?;
        let exact = self
            .current
            .identities
            .canonical_key::<_, ExactTypeKey>(expression.exact())?;
        // A structural value's constituent types do not own its box or TD.
        // The original full exact, not the nominal fanout, chooses this root.
        if exact.as_ref() == &ExactTypeKey::Nominal(owner) {
            self.select(owner, Kind::ShapeSupport, meter)?;
        }
        Ok(())
    }
}
