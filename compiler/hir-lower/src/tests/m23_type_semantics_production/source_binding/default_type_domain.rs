use super::super::source_dispatch::with_hir_source;
use super::*;
use hir::{
    CanonicalDefaultSourceAccessDeclarationsV1 as Table, DefaultSourceTypeAccessDemandV1 as Demand,
    DefaultSourceTypeDomainError as Error, DefaultSourceTypeDomainsV1 as Domains,
};
use scoop_identity::{
    CoreBuiltinNominal, DefinitionOriginSubject as Subject, SignatureTypeKey as Type,
};
use scoop_wire::WirePath;
mod dependencies;
mod rejection;
mod resources;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/type-source-domains.scoop"
));
const NAMES: &[&str] = &[
    "nominal",
    "visible",
    "applied",
    "tuple",
    "function",
    "suspended",
    "binder",
    "builtin",
    "Enclosing.combined",
    "Enclosing.Static.copy",
    "Outer.Inner.preserve",
];
type Templates = Vec<(&'static str, hir::DefaultSourceTemplateV1)>;
fn scope(name: &str) -> hir::SignatureBinderScopeV1 {
    match name {
        "binder" => hir::SignatureBinderScopeV1::for_declaration(1, None),
        "Enclosing.combined" => hir::SignatureBinderScopeV1::for_declaration(1, Some(1)),
        "Outer.Inner.preserve" => hir::SignatureBinderScopeV1::for_declaration(0, Some(1)),
        _ => hir::SignatureBinderScopeV1::for_declaration(0, None),
    }
}
fn templates(output: &hir::OrdinaryHirOutput) -> Templates {
    NAMES
        .iter()
        .map(|name| {
            let id = output
                .output()
                .export
                .module()
                .functions
                .iter()
                .find(|(_, f)| f.name == *name)
                .unwrap()
                .0;
            (
                *name,
                hir::DefaultSourceBodyProductionV1::from_ordinary_hir(
                    output,
                    hir::ExportParameterOwner::Function(id),
                    1,
                    &mut meter(),
                )
                .unwrap()
                .into_source_template(&mut meter())
                .unwrap(),
            )
        })
        .collect()
}
fn required(templates: &Templates) -> BTreeSet<Subject> {
    let builtins = [
        CoreBuiltinNominal::Unit.identity_record().id(),
        CoreBuiltinNominal::Any.identity_record().id(),
    ];
    let mut required = BTreeSet::new();
    for (_, template) in templates {
        for ty in template
            .references()
            .types()
            .iter()
            .map(|r| r.target())
            .chain([template.result()])
        {
            hir::visit_default_source_type_access_demands(
                ty,
                &mut meter(),
                &WirePath::root(),
                &mut |demand, _, _| -> Result<(), std::convert::Infallible> {
                    match demand {
                        Demand::Nominal(id) if !builtins.contains(&id) => {
                            required.insert(Subject::Type(id));
                        }
                        Demand::NominalApplication { origin, .. } => {
                            required.insert(Subject::GenericType(origin));
                        }
                        Demand::Nominal(_) | Demand::Binder { .. } => {}
                        _ => panic!("ordinary fixture has no pointer wrappers"),
                    }
                    Ok(())
                },
            )
            .unwrap();
        }
    }
    required
}
fn with_local(
    run: impl FnOnce(
        &crate::tests::m23_ordinary_core_only::support::TrustedCoreFixture,
        &Fixture,
        &Table,
        &BTreeSet<Subject>,
        &Templates,
    ),
) {
    with_hir_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let templates = templates(output);
        let required = required(&templates);
        let table =
            Table::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
        let decoded: hir::DecodedCanonicalDefaultSourceAccessDeclarationsV1 =
            decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
        let table = decoded
            .resolve(&mut fixture.identities, &mut meter())
            .unwrap();
        run(core, &fixture, &table, &required, &templates);
    });
}
fn summary(domain: &hir::DefaultSourceAccessDomainV1) -> String {
    use hir::PersistentAccessConstraintV1 as C;
    let names = domain
        .persistent()
        .constraints()
        .iter()
        .map(|c| match c {
            C::Cone(_) => "Cone",
            C::File(_) => "File",
            C::LexicalOwner(_) => "LexicalOwner",
            C::SubclassesOf(_) => "SubclassesOf",
        })
        .collect::<Vec<_>>();
    format!(
        "[{}]; generic {}",
        names.join(", "),
        domain.generic_subclasses().values().len()
    )
}
#[test]
fn default_type_source_domains_replay_actual_sealed_type_witnesses() {
    with_local(|core, fixture, table, required, templates| {
        let foundation = fixture.bind().unwrap();
        let declarations = foundation
            .bind_default_access_declarations(table, required, &mut meter())
            .unwrap();
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        let domains = Domains::new(
            &declarations,
            &[],
            inputs.protocols().fundamental_types(),
            &mut meter(),
        )
        .unwrap();
        let mut outline = String::new();
        let mut compared = 0;
        for (name, template) in templates {
            let scope = scope(name);
            for record in template.references().types() {
                compared += 1;
                let actual = domains
                    .type_source_domain(record.target(), &scope, &mut meter())
                    .unwrap();
                assert_eq!(
                    &actual,
                    record.witness().target_domain(),
                    "{name}: {:?}",
                    record.target()
                );
            }
            let actual = domains
                .type_source_domain(template.result(), &scope, &mut meter())
                .unwrap();
            outline.push_str(&format!("{name}: {}\n", summary(&actual)));
        }
        assert!(compared > 0);
        assert_eq!(
            outline,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/type-source-domains.snap"
            ))
        );
    });
}
