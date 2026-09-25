use super::super::source_dispatch::with_hir_source;
use super::*;
use hir::DefaultSourceTypeAccessDemandV1 as Demand;
use scoop_identity::{
    CoreBuiltinNominal, DeclarationName, SignatureTypeKey as Type, SourceDeclarationKey,
};
use scoop_wire::WirePath;
mod limits;
mod pointers;
mod resources;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/type-access-demands.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/type-access-combinations.scoop"
));
const CASES: &[(&str, u32)] = &[
    ("nominal", 0),
    ("applied", 0),
    ("tuple", 1),
    ("function", 1),
    ("suspended", 1),
    ("binder", 1),
    ("builtin", 1),
];
fn with_types(
    source: &str,
    cases: &[(&str, u32)],
    run: impl FnOnce(&ValidatedIdentityGraph, &[(&str, hir::DefaultSourceTemplateV1)]),
) {
    with_hir_source(source, |output, _| {
        let fixture = Fixture::from_output(output);
        let templates = cases
            .iter()
            .map(|(name, position)| {
                let function = output
                    .output()
                    .export
                    .module()
                    .functions
                    .iter()
                    .find(|(_, f)| f.name == *name)
                    .unwrap()
                    .0;
                let template = hir::DefaultSourceBodyProductionV1::from_dependency_hir(
                    output,
                    hir::ExportParameterOwner::Function(function),
                    *position,
                )
                .unwrap()
                .into_source_template()
                .unwrap();
                (*name, template)
            })
            .collect::<Vec<_>>();
        run(&fixture.identities, &templates);
    });
}
fn nominal_name(identities: &ValidatedIdentityGraph, demand: Demand<'_>) -> String {
    let key = match demand {
        Demand::Nominal(id) => {
            for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
                if id == builtin.identity_record().id() {
                    return format!("{builtin:?}");
                }
            }
            identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap()
        }
        Demand::NominalApplication { origin, arguments } => {
            let key = identities
                .canonical_key::<_, SourceDeclarationKey>(origin)
                .unwrap();
            assert_eq!(
                key.duplicate_signature().type_parameter_count() as usize,
                arguments.len()
            );
            key
        }
        _ => panic!("nominal demand"),
    };
    match key.name() {
        DeclarationName::Named(name) => name.as_str().to_owned(),
        other => panic!("nominal source name: {other:?}"),
    }
}
fn outline(
    identities: &ValidatedIdentityGraph,
    templates: &[(&str, hir::DefaultSourceTemplateV1)],
) -> String {
    let mut output = String::new();
    for (name, template) in templates {
        output.push_str(&format!("{name}\n"));
        hir::visit_default_source_type_access_demands(
            template.result(),
            &WirePath::root(),
            &mut |demand, path| -> Result<(), std::convert::Infallible> {
                let label = match demand {
                    Demand::Nominal(_) => format!("Nominal {}", nominal_name(identities, demand)),
                    Demand::NominalApplication { arguments, .. } => format!(
                        "Application {} {}",
                        nominal_name(identities, demand),
                        arguments.len()
                    ),
                    Demand::RawPointer { .. } => "RawPointer".into(),
                    Demand::NativeFunctionPointer {
                        calling_convention, ..
                    } => {
                        assert_eq!(calling_convention, scoop_identity::CallingConvention::C);
                        "NativeFunctionPointer".into()
                    }
                    Demand::Binder { depth, index } => format!("Binder {depth}:{index}"),
                };
                output.push_str(&format!("  {path} {label}\n"));
                Ok(())
            },
        )
        .unwrap();
    }
    output
}
#[test]
fn default_type_access_demands_preserve_all_signature_branches_and_occurrences() {
    for (source, cases, expected) in [
        (
            SOURCE,
            CASES,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/type-access-demands.snap"
            )),
        ),
        (
            COMBINATIONS,
            &[("Envelope.combined", 1)][..],
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/type-access-combinations.snap"
            )),
        ),
    ] {
        with_types(source, cases, |fixture, templates| {
            assert_eq!(outline(fixture, templates), expected);
        });
    }
}
