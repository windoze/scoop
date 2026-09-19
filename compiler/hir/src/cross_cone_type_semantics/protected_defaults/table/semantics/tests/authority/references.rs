use super::*;

impl Authority<'_> {
    fn reference(
        &mut self,
        source: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, &'static str> {
        self.check(source.template, meter, path)?;
        if source.owner.key() != self.fixture.key
            || source.occurrence.definition_origin != &self.fixture.origin
        {
            return Err("wrong reference source context");
        }
        let callable = match source.occurrence.target {
            DefaultBodyReferenceTargetV1::Type(ty)
                if ty == &self.fixture.unit
                    || ty == &self.fixture.receiver
                    || ty == &self.fixture.function_type() =>
            {
                false
            }
            DefaultBodyReferenceTargetV1::Callable(
                DefaultCallableReferenceTargetViewV1::Lambda(id),
            ) if id == self.fixture.lambda && self.case.nested() => true,
            _ => return Err("unknown actual typed reference target"),
        };
        match source.receiver {
            ProtectedDefaultReferenceReceiverV1::Metadata(
                DefaultBodyReferenceMetadataV1::TemplateLocal { local, .. },
            ) => {
                assert!(
                    local.value_type() == &self.fixture.unit
                        || local.value_type() == &self.fixture.receiver
                );
                self.metadata_calls += 1;
            }
            ProtectedDefaultReferenceReceiverV1::Metadata(
                DefaultBodyReferenceMetadataV1::Capture(capture),
            ) if self.case.nested() => {
                assert_eq!(capture.value_type(), &self.fixture.unit);
                self.metadata_calls += 1;
            }
            ProtectedDefaultReferenceReceiverV1::None => self.expression_calls += 1,
            _ => return Err("unexpected reference receiver context"),
        }
        if matches!(self.case, Case::ReferenceSource | Case::GenericReject) {
            return Err("independent reference access denied");
        }
        Ok(callable)
    }
}
impl ProtectedDefaultReferenceAccessSemanticAuthority<&'static str> for Authority<'_> {
    fn replay_param_free_default_reference<'g, 'a>(
        &mut self,
        source: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        graph: &'g CheckedNominalInheritanceGraphV1<'a>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CheckedPersistentAccessDomainV1<'g, 'a>, &'static str> {
        let callable = self.reference(source, meter, path)?;
        self.concrete_calls += 1;
        if callable {
            graph
                .validate_access_domain(self.fixture.direct.as_ref().unwrap(), meter)
                .map_err(|_| "generated body source access")
        } else {
            let owner = match source.occurrence.target {
                DefaultBodyReferenceTargetV1::Type(SignatureTypeKey::Nominal(id)) => {
                    SourceNominalId::Concrete(*id)
                }
                DefaultBodyReferenceTargetV1::Type(ty) if ty == &self.fixture.function_type() => {
                    self.fixture.source.unit.source
                }
                _ => return Err("nonconcrete type in param-free replay"),
            };
            let access = graph
                .replay_nominal_access(owner, meter)
                .map_err(|_| "unknown actual nominal source")?;
            graph
                .validate_access_domain(access.lookup().domain(), meter)
                .map_err(|_| "wrong source graph domain")
        }
    }
    fn validate_generic_default_reference(
        &mut self,
        source: ProtectedDefaultReferenceSourceUseV1<'_, '_, '_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), &'static str> {
        assert!(self.case.generic());
        self.reference(source, meter, path).map(|_| ())
    }
}
