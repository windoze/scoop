pub(super) use super::super::default_domains_support::Inputs;
use super::*;
use scoop_identity::DefinitionOriginSubject as Subject;

pub(super) fn with_inputs(source: &str, run: impl FnOnce(&Inputs<'_>)) {
    super::super::default_domains_support::with_inputs(source, required, run);
}
pub(in crate::tests::m23_type_semantics_production::source_binding) fn required(
    foundation: &hir::BoundTypeFoundationSourcesV1<'_>,
    templates: &hir::CanonicalDefaultSourceTemplatesV1,
) -> BTreeSet<Subject> {
    let mut required = BTreeSet::new();
    for template in templates.records() {
        let closure = template
            .bind_reference_occurrences(&scoop_wire::WirePath::root())
            .unwrap();
        for occurrence in closure.occurrences() {
            let subject = match occurrence.source() {
                Reference::Constructor(r) => foundation
                    .default_constructor_access_subject(r.target())
                    .unwrap(),
                Reference::Global(r) => foundation
                    .default_global_access_subject(*r.target())
                    .unwrap(),
                Reference::Singleton(r) => foundation
                    .default_indirect_access_subject(hir::DefaultSourceIndirectTargetV1::Singleton(
                        *r.target(),
                    ))
                    .unwrap(),
                Reference::Field(r) => {
                    match foundation.default_field_access_subject(r.target()).unwrap() {
                        hir::DefaultSourceFieldAccessSubjectV1::Declaration(subject) => subject,
                        hir::DefaultSourceFieldAccessSubjectV1::TupleElement { .. } => continue,
                    }
                }
                Reference::Callable(_) | Reference::Type(_) => continue,
            };
            required.insert(subject);
        }
    }
    required
}
