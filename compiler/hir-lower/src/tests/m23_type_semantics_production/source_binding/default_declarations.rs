use super::default_origins::{replace, templates};
use super::nominal_parameters::support::with_sources;
use super::*;
use hir::{DefaultSourceDeclarationBindingError as Error, DefaultSourceTemplateV1 as Template};
use scoop_identity::{CallableTemplateOrigin, LocalValueSelector, SignatureTypeKey};

mod corruption;
mod data_flow;
mod dependency_binders;
mod envelope;
mod nested_identities;
mod nested_index;
mod nested_parents;
mod owner_arguments;
mod providers;
mod resources;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/declaration-binding.scoop"
));
fn source_template(output: &hir::OrdinaryHirOutput<'_>, name: &str, position: u32) -> Template {
    let export = &output.output().export;
    let id = export
        .functions
        .iter()
        .find(|(_, f)| f.name == name)
        .unwrap()
        .0;
    hir::DefaultSourceBodyProductionV1::from_ordinary_hir(
        output,
        hir::ExportParameterOwner::Function(id),
        position,
        &mut meter(),
    )
    .unwrap()
    .into_source_template(&mut meter())
    .unwrap()
}
fn key(
    output: &hir::OrdinaryHirOutput<'_>,
    name: &str,
    position: u32,
) -> hir::ProtectedDefaultTemplateKeyV1 {
    source_template(output, name, position).key()
}
#[test]
fn default_declarations_join_real_artifact_sources_for_every_parameter_owner() {
    for source in [
        "public class Empty {}",
        SOURCE,
        envelope::SOURCE,
        nested_identities::SOURCE,
        nested_parents::SOURCE,
        data_flow::SOURCE,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/origin-binding.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/parameters-binding.scoop"
        )),
    ] {
        with_sources(source, |output, fixture, sources, core| {
            let table = templates(output);
            let foundation = fixture.bind().unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                    .unwrap();
                let bound = parameters
                    .bind_default_declarations(&table, &[], &mut meter())
                    .unwrap();
                assert_eq!(bound.provider(), parameters.provider());
                assert!(std::ptr::eq(bound.origins().templates(), &table));
                assert_eq!(bound.declarations().len(), table.records().len());
                for template in table.records() {
                    let declared = bound.declaration(template.key(), &mut meter()).unwrap();
                    assert_eq!(declared.key(), template.key());
                    let parameter = declared.provider_parameter();
                    assert_eq!(parameter.position(), template.key().parameter_position());
                    assert!(std::ptr::eq(
                        parameter.parameters(),
                        declared.provider().parameters()
                    ));
                    assert_eq!(parameter.current().value_type(), template.result());
                    assert_eq!(declared.definition_root(), template.definition_root());
                    assert_eq!(
                        declared.provider_receiver(),
                        template.receiver().receiver().map(|r| r.value_type())
                    );
                    assert_eq!(
                        declared.provider_binders().binder_arity(),
                        template.type_parameters().len_u32()
                    );
                    if matches!(
                        template.key().owner(),
                        CallableTemplateOrigin::Constructor(_)
                            | CallableTemplateOrigin::VariantConstructor(_)
                    ) {
                        assert!(declared.provider_receiver().is_none());
                    }
                }
                if source == SOURCE {
                    let inherited = key(output, "ContractChild.pick", 2);
                    let contract = bound.declaration(inherited, &mut meter()).unwrap();
                    assert_ne!(contract.owner().owner(), contract.provider().owner());
                    assert_eq!(contract.owner_binders().nominal_owner_binder_arity(), 1);
                    assert_eq!(contract.provider_binders().nominal_owner_binder_arity(), 2);
                    let copy = bound
                        .declaration(key(output, "ContractBase.copy", 2), &mut meter())
                        .unwrap();
                    assert_eq!(copy.provider_binders().nominal_owner_binder_arity(), 2);
                    assert_eq!(copy.provider_binders().callable_own_binder_arity(), 1);
                    let nested = bound
                        .declaration(key(output, "ContractBase.Static.copy", 1), &mut meter())
                        .unwrap();
                    assert_eq!(nested.provider_binders().nominal_owner_binder_arity(), 0);
                }
            });
        });
    }
}
