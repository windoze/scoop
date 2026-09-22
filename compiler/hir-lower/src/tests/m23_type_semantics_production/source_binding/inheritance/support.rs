use super::*;

#[derive(Clone)]
pub(super) struct Sources {
    pub properties: super::super::properties::Sources,
    pub constructors: hir::CanonicalInheritanceSourceConstructorsV1,
    pub callables: hir::CanonicalInheritanceSourceProtectedCallablesV1,
    pub nominals: hir::CanonicalNominalSourceContractsV1,
}
impl Sources {
    pub fn from_output(output: &hir::DependencyHirOutput, fixture: &mut Fixture) -> Self {
        let properties = super::super::properties::Sources::from_output(output, fixture);
        macro_rules! restore {
            ($value:expr, $decoded:ty) => {{
                let value = $value.unwrap();
                let bytes = encode(&value).unwrap();
                let decoded: $decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
                let restored = decoded
                    .resolve(&mut fixture.identities, &mut meter())
                    .unwrap();
                assert_eq!(encode(&restored).unwrap(), bytes);
                restored
            }};
        }
        Self {
            properties,
            constructors: restore!(
                hir::CanonicalInheritanceSourceConstructorsV1::from_dependency_hir(
                    output,
                    &mut meter()
                ),
                hir::DecodedCanonicalInheritanceSourceConstructorsV1
            ),
            callables: restore!(
                hir::CanonicalInheritanceSourceProtectedCallablesV1::from_dependency_hir(
                    output,
                    &mut meter()
                ),
                hir::DecodedCanonicalInheritanceSourceProtectedCallablesV1
            ),
            nominals: restore!(
                hir::CanonicalNominalSourceContractsV1::from_export_hir(
                    &output.output().export,
                    &fixture.source.entries().source_roots,
                    &mut meter()
                ),
                hir::DecodedCanonicalNominalSourceContractsV1
            ),
        }
    }
    pub fn with_bound<R>(
        &self,
        foundation: &hir::BoundTypeFoundationSourcesV1<'_>,
        budget: &mut BudgetMeter,
        run: impl FnOnce(
            &mut hir::BoundInheritanceSourcesV1<'_, '_, '_>,
            &hir::CheckedNominalInheritanceGraphV1<'_>,
        ) -> R,
    ) -> Result<R, Error> {
        let dispatch = self
            .properties
            .dispatch
            .bind(foundation, &mut meter())
            .unwrap();
        let slots = dispatch.bind_slot_sources(&mut meter()).unwrap();
        let properties = self.properties.bind(foundation, &mut meter()).unwrap();
        let protected = properties
            .bind_protected_callable_sources(&self.callables, &mut meter())
            .unwrap();
        let constructors = foundation
            .bind_inheritance_constructor_sources(
                &self.properties.dispatch.inventory,
                &self.constructors,
                &mut meter(),
            )
            .unwrap();
        let nominals = foundation
            .bind_nominal_sources(&self.nominals, &mut meter())
            .unwrap();
        let mut bound =
            protected.bind_inheritance_sources(&constructors, &nominals, &slots, budget)?;
        Ok(run(&mut bound, slots.graph()))
    }
}
