use super::*;
use scoop_hir::HirDependencyTypeSiteV1;
use scoop_identity::{CoreBuiltinNominal, ExactTypeKey};

impl HirInterfaceValidationInput<'_> {
    pub(super) fn generated_type_site(
        self,
        provider: ConeIdentity,
        site: &HirDependencyTypeSiteV1,
        dependencies: &[ValidatedNominalProviderView<'_>],
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), CrossConeHirTypeSiteError> {
        let valid = match site {
            HirDependencyTypeSiteV1::ConstructorInitializerResult { exact, .. } => {
                meter.charge_work(
                    1 + u64::from(self.identities.identity_count().max(1).ilog2()),
                    path,
                )?;
                let key = self
                    .identities
                    .canonical_key::<_, ExactTypeKey>(*exact)
                    .map_err(CrossConeHirTypeSiteError::Identity)?;
                *key == ExactTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
            }
            HirDependencyTypeSiteV1::InitializationCycleMessage { exact, .. } => {
                meter.charge_work(dependencies.len() as u64 + 1, path)?;
                let source = if provider == self.current {
                    Some(self.core)
                } else {
                    dependencies
                        .iter()
                        .find(|view| view.identity == provider)
                        .map(|view| view.core)
                };
                source
                    .and_then(CoreBootstrapInterfaceSectionV1::compiler_protocol_definitions)
                    .is_some_and(|protocol| protocol.string_capability().exact_type() == *exact)
            }
            _ => return Ok(()),
        };
        if valid {
            Ok(())
        } else {
            Err(CrossConeHirTypeSiteError::GeneratedType {
                position: site.position(),
                actual: site.exact(),
            })
        }
    }
}
