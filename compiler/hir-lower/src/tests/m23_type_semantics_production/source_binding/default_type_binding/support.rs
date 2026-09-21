use super::super::super::source_dispatch::with_hir_source;
use super::super::nominal_parameters::support::Sources;
use super::*;
use scoop_identity::DefinitionOriginSubject as Subject;
type Access = hir::CanonicalDefaultSourceAccessDeclarationsV1;

pub(super) struct Inputs<'a> {
    pub output: &'a hir::OrdinaryHirOutput<'a>,
    pub fixture: &'a Fixture,
    pub sources: &'a Sources,
    pub core: &'a hir::ImportedHirFoundation,
    pub core_types: &'a hir::ImportedCoreFundamentalTypeProtocol,
    pub templates: hir::CanonicalDefaultSourceTemplatesV1,
    pub access: Access,
    pub required: BTreeSet<Subject>,
}
pub(super) fn with_inputs(source: &str, run: impl FnOnce(&Inputs<'_>)) {
    with_hir_source(source, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let templates = super::super::default_origins::templates(output);
        // Subjects come from sealed nominal sources and actual default references.
        let mut required: BTreeSet<_> = sources
            .members
            .nominals
            .records()
            .iter()
            .map(|r| match r.owner() {
                hir::SourceNominalId::Concrete(id) => Subject::Type(id),
                hir::SourceNominalId::GenericTemplate(id) => Subject::GenericType(id),
            })
            .collect();
        for ty in templates
            .records()
            .iter()
            .flat_map(|t| t.references().types().iter().map(|r| r.target()))
        {
            hir::visit_default_source_type_access_demands(
                ty,
                &mut meter(),
                &scoop_wire::WirePath::root(),
                &mut |demand, _, _| {
                    use hir::DefaultSourceTypeAccessDemandV1 as D;
                    use scoop_identity::SourceDeclarationKey;
                    let subject = match demand {
                        D::Nominal(id) => {
                            let key = fixture
                                .identities
                                .canonical_key::<_, SourceDeclarationKey>(id)
                                .unwrap();
                            (key.origin() != scoop_identity::ConeIdentity::CORE)
                                .then_some(Subject::Type(id))
                        }
                        D::NominalApplication { origin, .. } => {
                            let key = fixture
                                .identities
                                .canonical_key::<_, SourceDeclarationKey>(origin)
                                .unwrap();
                            (key.origin() != scoop_identity::ConeIdentity::CORE)
                                .then_some(Subject::GenericType(origin))
                        }
                        D::Binder { .. } => None,
                        _ => panic!("fixture has no pointer wrappers"),
                    };
                    if let Some(subject) = subject {
                        required.insert(subject);
                    }
                    Ok::<_, std::convert::Infallible>(())
                },
            )
            .unwrap();
        }
        let access =
            Access::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
        let decoded: hir::DecodedCanonicalDefaultSourceAccessDeclarationsV1 =
            decode_canonical(&encode(&access).unwrap(), DecodeLimits::default()).unwrap();
        let access = decoded
            .resolve(&mut fixture.identities, &mut meter())
            .unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        run(&Inputs {
            output,
            fixture: &fixture,
            sources: &sources,
            core: &core.foundation,
            core_types: inputs.protocols().fundamental_types(),
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
            &Domains<'_, '_, '_, '_>,
            &hir::BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
            &hir::BoundTypeFoundationSourcesV1<'_>,
        ),
    ) {
        let foundation = self.fixture.bind().unwrap();
        let access = foundation
            .bind_default_access_declarations(&self.access, &self.required, &mut meter())
            .unwrap();
        let domains = Domains::new(&access, &[], self.core, self.core_types, &mut meter()).unwrap();
        self.sources
            .with_bound(&foundation, self.core_types, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &self.sources.protocols, &mut meter())
                    .unwrap();
                run(&domains, &parameters, &foundation);
            });
    }
}
