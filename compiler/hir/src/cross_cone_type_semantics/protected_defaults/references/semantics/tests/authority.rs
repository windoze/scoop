use super::*;

pub(super) struct Authority {
    pub key: ProtectedDefaultTemplateKeyV1,
    pub target: SourceNominalId,
    pub origin: ExportDefinitionSourceV1,
    pub profile: ProtectedDefaultWitnessSourceProfileV1,
    pub roots: CanonicalProtectedSlotRefsV1,
    pub case: Case,
    pub concrete: usize,
    pub metadata: usize,
}
impl ProtectedDefaultSourceProfileSemanticAuthority<&'static str> for Authority {
    fn default_access_profile(
        &self,
        key: ProtectedDefaultTemplateKeyV1,
        _meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<ProtectedDefaultWitnessSourceProfileV1, &'static str> {
        if key != self.key {
            return Err("unknown source default");
        }
        Ok(self.profile)
    }
}
impl ProtectedDefaultRootSlotSemanticAuthority<&'static str> for Authority {
    fn default_root_slots(
        &self,
        owner: CallableTemplateOrigin,
    ) -> Result<&CanonicalProtectedSlotRefsV1, &'static str> {
        if owner != self.key.owner() {
            return Err("wrong root-slot owner");
        }
        Ok(&self.roots)
    }
}
impl ExportDefinitionSourceSemanticAuthority<&'static str> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        self.origin.origin().source().cone()
    }
    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        if source != &self.origin || matches!(self.case, Case::Origin) {
            return Err("unproven evaluation origin");
        }
        Ok(())
    }
}
impl Authority {
    fn validate(
        &mut self,
        source_use: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), &'static str> {
        meter
            .charge_work(1, path)
            .map_err(|_| "source work budget")?;
        if source_use.owner.key() != self.key || source_use.template.key() != self.key {
            return Err("wrong typed source owner");
        }
        let DefaultBodyReferenceTargetV1::Type(SignatureTypeKey::Nominal(target)) =
            source_use.occurrence.target
        else {
            return Err("wrong typed target kind");
        };
        if SourceNominalId::Concrete(*target) != self.target
            || matches!(self.case, Case::WrongTarget)
        {
            return Err("wrong typed target source");
        }
        match source_use.receiver {
            ProtectedDefaultReferenceReceiverV1::Metadata(
                DefaultBodyReferenceMetadataV1::TemplateLocal { local, .. },
            ) => {
                assert_eq!(local.value_type(), &SignatureTypeKey::Nominal(*target));
                self.metadata += 1;
                if matches!(self.case, Case::RejectMetadata | Case::GenericReject) {
                    return Err("metadata source access denied");
                }
            }
            ProtectedDefaultReferenceReceiverV1::None => {}
            _ => return Err("wrong actual receiver context"),
        }
        Ok(())
    }
}
impl ProtectedDefaultReferenceAccessSemanticAuthority<&'static str> for Authority {
    fn replay_param_free_default_reference<'g, 'a>(
        &mut self,
        source_use: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        graph: &'g CheckedNominalInheritanceGraphV1<'a>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CheckedPersistentAccessDomainV1<'g, 'a>, &'static str> {
        self.validate(source_use, meter, path)?;
        self.concrete += 1;
        let target = graph
            .replay_nominal_access(self.target, meter)
            .map_err(|_| "target source domain")?;
        graph
            .validate_access_domain(target.lookup().domain(), meter)
            .map_err(|_| "target graph domain")
    }
    fn validate_generic_default_reference(
        &mut self,
        source_use: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), &'static str> {
        self.validate(source_use, meter, path)
    }
}
