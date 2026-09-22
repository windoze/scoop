use super::super::nominal_parameters::support::{Sources, with_sources};
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
pub(super) fn with_inputs(source: &str, run: impl FnOnce(&Inputs<'_>)) {
    with_sources(source, |output, fixture, sources, core_types| {
        let templates = super::super::default_origins::templates(output);
        let required = required(&fixture.bind().unwrap(), &templates);
        let access =
            Access::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
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
pub(in crate::tests::m23_type_semantics_production::source_binding) fn required(
    foundation: &hir::BoundTypeFoundationSourcesV1<'_>,
    templates: &hir::CanonicalDefaultSourceTemplatesV1,
) -> BTreeSet<Subject> {
    let mut required = BTreeSet::new();
    for template in templates.records() {
        let closure = template
            .bind_reference_occurrences(&mut meter(), &scoop_wire::WirePath::root())
            .unwrap();
        for occurrence in closure.occurrences() {
            let subject = match occurrence.source() {
                Reference::Constructor(r) => foundation
                    .default_constructor_access_subject(r.target(), &mut meter())
                    .unwrap(),
                Reference::Global(r) => foundation
                    .default_global_access_subject(*r.target(), &mut meter())
                    .unwrap(),
                Reference::Singleton(r) => foundation
                    .default_indirect_access_subject(
                        hir::DefaultSourceIndirectTargetV1::Singleton(*r.target()),
                        &mut meter(),
                    )
                    .unwrap(),
                Reference::Field(r) => match foundation
                    .default_field_access_subject(r.target(), &mut meter())
                    .unwrap()
                {
                    hir::DefaultSourceFieldAccessSubjectV1::Declaration(subject) => subject,
                    hir::DefaultSourceFieldAccessSubjectV1::TupleElement { .. } => continue,
                },
                Reference::Callable(_) | Reference::Type(_) => continue,
            };
            required.insert(subject);
        }
    }
    required
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
        let domains = Domains::new(&access, &[], self.core_types, &mut meter()).unwrap();
        self.sources
            .with_bound(&foundation, self.core_types, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &self.sources.protocols, &mut meter())
                    .unwrap();
                run(&domains, &parameters, &foundation);
            });
    }
}
