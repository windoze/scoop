use super::super::default_domains_support::Inputs;
use super::*;
use scoop_identity::DefinitionOriginSubject as Subject;

pub(super) fn with_inputs(source: &str, run: impl FnOnce(&Inputs<'_>)) {
    super::super::default_domains_support::with_inputs(source, required, run);
}

pub(super) fn named<'a>(
    inputs: &'a Inputs<'_>,
    name: &str,
    position: u32,
) -> &'a hir::DefaultSourceTemplateV1 {
    let module = inputs.output.output().export.module();
    let function = module
        .functions
        .iter()
        .find(|(_, f)| f.name == name)
        .unwrap()
        .0;
    let source = hir::DefaultSourceBodyProductionV1::from_dependency_hir(
        inputs.output,
        hir::ExportParameterOwner::Function(function),
        position,
    )
    .unwrap();
    let key = source.into_source_template().unwrap().key();
    inputs.templates.get(key).unwrap()
}

pub(in crate::tests::m23_type_semantics_production::source_binding) fn required(
    foundation: &hir::BoundTypeFoundationSourcesV1<'_>,
    templates: &hir::CanonicalDefaultSourceTemplatesV1,
) -> BTreeSet<Subject> {
    let mut result = foundation
        .source()
        .entries()
        .sources
        .records()
        .iter()
        .map(|r| match r.owner() {
            hir::SourceNominalId::Concrete(id) => Subject::Type(id),
            hir::SourceNominalId::GenericTemplate(id) => Subject::GenericType(id),
        })
        .collect();
    for template in templates.records() {
        for reference in template.references().callables() {
            add(foundation, reference.target(), &mut result);
        }
        let nested = template
            .index_nested_callables(&scoop_wire::WirePath::root())
            .unwrap();
        for occurrence in nested.occurrences() {
            if let hir::DefaultSourceNestedCallableDescriptorV1::CallableReference(reference) =
                occurrence.descriptor()
            {
                let target = match reference.target() {
                    hir::DefaultCallableReferenceTargetV1::Named(callee)
                    | hir::DefaultCallableReferenceTargetV1::Local { callee, .. }
                    | hir::DefaultCallableReferenceTargetV1::BoundExtension { callee, .. } => {
                        Target::Callable(callee.clone())
                    }
                    hir::DefaultCallableReferenceTargetV1::BoundMember { callee, .. } => {
                        match callee {
                            hir::DefaultMethodCalleeV1::Callable(callee) => {
                                Target::Callable(callee.clone())
                            }
                            hir::DefaultMethodCalleeV1::Bound(bound) => {
                                Target::Bound(bound.clone())
                            }
                            hir::DefaultMethodCalleeV1::DerivedEquality { owner_type } => {
                                Target::DerivedEquality {
                                    owner_type: owner_type.clone(),
                                }
                            }
                        }
                    }
                };
                add(foundation, &target, &mut result);
            }
        }
    }
    result
}

fn add(
    foundation: &hir::BoundTypeFoundationSourcesV1<'_>,
    target: &Target,
    required: &mut BTreeSet<Subject>,
) {
    if let hir::DefaultSourceCallableAccessSubjectV1::Declaration(subject) =
        foundation.default_callable_access_subject(target).unwrap()
    {
        required.insert(subject);
    }
}
