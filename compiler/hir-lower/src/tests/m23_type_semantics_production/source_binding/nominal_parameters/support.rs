use super::super::nominal_members::support::Sources as Members;
use super::*;

#[derive(Clone)]
pub(in crate::tests::m23_type_semantics_production::source_binding) struct Sources {
    pub members: Members,
    pub constructors: hir::CanonicalNominalSourceConstructorsV1,
    pub protocols: Table,
}
impl Sources {
    pub(in crate::tests::m23_type_semantics_production::source_binding) fn from_output(
        output: &hir::OrdinaryHirOutput<'_>,
        fixture: &mut Fixture,
    ) -> Self {
        let members = Members::from_output(output, fixture);
        let required = members
            .nominals
            .records()
            .iter()
            .flat_map(|n| n.constructors().values().iter().copied())
            .collect();
        let constructors = hir::CanonicalNominalSourceConstructorsV1::from_export_hir(
            &output.output().export,
            &required,
            &mut meter(),
        )
        .unwrap();
        let bytes = encode(&constructors).unwrap();
        let decoded: hir::DecodedCanonicalNominalSourceConstructorsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let constructors = decoded
            .resolve(&mut fixture.identities, &mut meter())
            .unwrap();
        assert_eq!(encode(&constructors).unwrap(), bytes);
        let required = constructors
            .records()
            .iter()
            .map(|r| CallableTemplateOrigin::Constructor(r.declaration()))
            .chain(
                members
                    .callables
                    .records()
                    .iter()
                    .map(|r| r.declaration())
                    .filter(|o| !matches!(o, CallableTemplateOrigin::Accessor(_))),
            )
            .collect();
        let protocols =
            Table::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
        let bytes = encode(&protocols).unwrap();
        let decoded: hir::DecodedCanonicalNominalSourceParameterProtocolsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let protocols = decoded
            .resolve(&mut fixture.identities, &mut meter())
            .unwrap();
        assert_eq!(encode(&protocols).unwrap(), bytes);
        Self {
            members,
            constructors,
            protocols,
        }
    }
    pub(in crate::tests::m23_type_semantics_production::source_binding) fn with_bound<R>(
        &self,
        foundation: &hir::BoundTypeFoundationSourcesV1<'_>,
        core: &hir::ImportedCoreFundamentalTypeProtocol,
        run: impl FnOnce(
            &hir::BoundNominalMemberSourcesV1<'_, '_, '_>,
            &hir::BoundNominalConstructorSourcesV1<'_, '_, '_>,
        ) -> R,
    ) -> R {
        let nominals = foundation
            .bind_nominal_sources(&self.members.nominals, &mut meter())
            .unwrap();
        let members = nominals
            .bind_member_sources(
                &self.members.properties,
                &self.members.callables,
                core,
                &mut meter(),
            )
            .unwrap();
        let constructors = nominals
            .bind_constructor_sources(&self.constructors, &mut meter())
            .unwrap();
        run(&members, &constructors)
    }
    pub(in crate::tests::m23_type_semantics_production::source_binding) fn replacing(
        &self,
        record: Record,
    ) -> Table {
        Table::try_new(
            self.protocols
                .records()
                .iter()
                .map(|r| {
                    if r.owner() == record.owner() {
                        record.clone()
                    } else {
                        r.clone()
                    }
                })
                .collect(),
            &mut meter(),
        )
        .unwrap()
    }
}
pub(in crate::tests::m23_type_semantics_production::source_binding) fn with_sources(
    source: &str,
    run: impl FnOnce(
        &hir::OrdinaryHirOutput<'_>,
        &Fixture,
        &Sources,
        &hir::ImportedCoreFundamentalTypeProtocol,
    ),
) {
    with_hir_source(source, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        run(
            output,
            &fixture,
            &sources,
            inputs.protocols().fundamental_types(),
        );
    });
}
