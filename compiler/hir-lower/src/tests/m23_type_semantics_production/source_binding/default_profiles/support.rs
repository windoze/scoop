use super::super::nominal_parameters::support::Sources;
use super::*;
use crate::tests::m23_ordinary_core_only::support::TrustedCoreFixture;
use scoop_identity::DefinitionOriginSubject as Subject;
type Access = hir::CanonicalDefaultSourceAccessDeclarationsV1;

pub(super) struct Inputs<'a> {
    pub output: &'a hir::DependencyHirOutput,
    pub fixture: &'a Fixture,
    pub sources: &'a Sources,
    pub core: &'a TrustedCoreFixture,
    pub core_types: &'a hir::ImportedCoreFundamentalTypeProtocol,
    pub production: hir::NominalDefaultSourceProductionV1,
    pub access: Access,
    pub required: BTreeSet<Subject>,
    core_source: hir::TypeFoundationSourceAuthorityV1,
    core_access: Access,
    core_required: BTreeSet<Subject>,
}
pub(super) fn with_inputs(source: &str, run: impl FnOnce(&Inputs<'_>)) {
    super::super::super::source_dispatch::with_hir_source(source, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let production =
            hir::NominalDefaultSourceProductionV1::from_dependency_hir(output).unwrap();
        let profiles: hir::DecodedCanonicalDefaultSourceProfilesV1 =
            decode_canonical(&encode(production.profiles()).unwrap()).unwrap();
        assert_eq!(
            profiles.resolve(&mut fixture.identities).unwrap(),
            *production.profiles()
        );
        let bound = fixture.bind().unwrap();
        let mut required = super::super::default_callable_domain::support::required(
            &bound,
            production.templates(),
        );
        required.extend(super::super::default_value_domain::support::required(
            &bound,
            production.templates(),
        ));
        let access = Access::from_export_hir(&output.output().export, &required).unwrap();
        let core_source = hir::CrossConeTypeSemanticsFoundationV1::from_hir(&core.source_output)
            .unwrap()
            .source_transcript()
            .unwrap();
        let core_required = core_source
            .entries()
            .sources
            .records()
            .iter()
            .map(|r| match r.owner() {
                hir::SourceNominalId::Concrete(id) => Subject::Type(id),
                hir::SourceNominalId::GenericTemplate(id) => Subject::GenericType(id),
            })
            .collect();
        let core_access =
            Access::from_export_hir(&core.source_output.export, &core_required).unwrap();
        let imported = core.foundation.import_core_inputs(&core.interface).unwrap();
        run(&Inputs {
            output,
            fixture: &fixture,
            sources: &sources,
            core,
            core_types: imported.protocols().fundamental_types(),
            production,
            access,
            required,
            core_source,
            core_access,
            core_required,
        });
    });
}
impl Inputs<'_> {
    pub fn with_bound(
        &self,
        run: impl FnOnce(
            &hir::DefaultSourceDomainsV1<'_, '_, '_, '_>,
            &hir::BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
            &hir::BoundTypeFoundationSourcesV1<'_>,
        ),
    ) {
        let foundation = self.fixture.bind().unwrap();
        let access = foundation
            .bind_default_access_declarations(&self.access, &self.required)
            .unwrap();
        let core = self
            .core_source
            .bind_to_foundation(&self.core.source_foundation, &self.fixture.identities)
            .unwrap();
        let core_access = core
            .bind_default_access_declarations(&self.core_access, &self.core_required)
            .unwrap();
        let dependencies = [&core_access];
        let domains =
            hir::DefaultSourceDomainsV1::new(&access, &dependencies, self.core_types).unwrap();
        self.sources
            .with_bound(&foundation, self.core_types, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &self.sources.protocols)
                    .unwrap();
                run(&domains, &parameters, &foundation);
            });
    }
}
