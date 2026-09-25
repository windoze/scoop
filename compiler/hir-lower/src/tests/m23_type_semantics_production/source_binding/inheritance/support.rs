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
                let decoded: $decoded = decode_canonical(&bytes).unwrap();
                let restored = decoded.resolve(&mut fixture.identities).unwrap();
                assert_eq!(encode(&restored).unwrap(), bytes);
                restored
            }};
        }
        Self {
            properties,
            constructors: restore!(
                hir::CanonicalInheritanceSourceConstructorsV1::from_dependency_hir(output),
                hir::DecodedCanonicalInheritanceSourceConstructorsV1
            ),
            callables: restore!(
                hir::CanonicalInheritanceSourceProtectedCallablesV1::from_dependency_hir(output),
                hir::DecodedCanonicalInheritanceSourceProtectedCallablesV1
            ),
            nominals: restore!(
                hir::CanonicalNominalSourceContractsV1::from_export_hir(
                    &output.output().export,
                    &fixture.source.entries().source_roots
                ),
                hir::DecodedCanonicalNominalSourceContractsV1
            ),
        }
    }
    pub fn with_bound<R>(
        &self,
        foundation: &hir::BoundTypeFoundationSourcesV1<'_>,

        run: impl FnOnce(
            &mut hir::BoundInheritanceSourcesV1<'_, '_, '_>,
            &hir::CheckedNominalInheritanceGraphV1<'_>,
        ) -> R,
    ) -> Result<R, Error> {
        let dispatch = self.properties.dispatch.bind(foundation).unwrap();
        let slots = dispatch.bind_slot_sources().unwrap();
        let properties = self.properties.bind(foundation).unwrap();
        let protected = properties
            .bind_protected_callable_sources(&self.callables)
            .unwrap();
        let constructors = foundation
            .bind_inheritance_constructor_sources(
                &self.properties.dispatch.inventory,
                &self.constructors,
            )
            .unwrap();
        let nominals = foundation.bind_nominal_sources(&self.nominals).unwrap();
        let mut bound = protected.bind_inheritance_sources(&constructors, &nominals, &slots)?;
        Ok(run(&mut bound, slots.graph()))
    }
}
