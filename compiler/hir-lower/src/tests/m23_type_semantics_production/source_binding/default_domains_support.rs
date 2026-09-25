use super::nominal_parameters::support::{Sources, with_sources};
use super::*;
use scoop_identity::DefinitionOriginSubject as Subject;
type Access = hir::CanonicalDefaultSourceAccessDeclarationsV1;

pub(super) struct Inputs<'a> {
    pub output: &'a hir::DependencyHirOutput,
    pub fixture: &'a Fixture,
    pub sources: &'a Sources,
    pub core_types: &'a hir::ImportedCoreFundamentalTypeProtocol,
    pub templates: hir::CanonicalDefaultSourceTemplatesV1,
    pub access: Access,
    pub required: BTreeSet<Subject>,
}
pub(super) fn with_inputs(
    source: &str,
    required: impl Fn(
        &hir::BoundTypeFoundationSourcesV1<'_>,
        &hir::CanonicalDefaultSourceTemplatesV1,
    ) -> BTreeSet<Subject>,
    run: impl FnOnce(&Inputs<'_>),
) {
    with_sources(source, |output, fixture, sources, core_types| {
        let templates = super::default_origins::templates(output);
        let required = required(&fixture.bind().unwrap(), &templates);
        let access = Access::from_export_hir(&output.output().export, &required).unwrap();
        run(&Inputs {
            output,
            fixture,
            sources,
            core_types,
            templates,
            access,
            required,
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
        let domains = hir::DefaultSourceDomainsV1::new(&access, &[], self.core_types).unwrap();
        self.sources
            .with_bound(&foundation, self.core_types, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &self.sources.protocols)
                    .unwrap();
                run(&domains, &parameters, &foundation);
            });
    }
}
