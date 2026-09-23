use super::*;
use hir::{SourceAccessConstraintV1 as Constraint, SourceAccessDomainV1 as Domain};
use scoop_wire::{decode_canonical, encode};

#[test]
fn shared_source_domains_preserve_private_internal_protected_and_generic_defaults() {
    for case in ["standalone", "combined"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-source-access-domains");
        let source = std::fs::read_to_string(directory.join(format!("{case}.scoop"))).unwrap();
        source_dispatch::with_hir_source(&source, |output, _| {
            let export = output.output().export.module();
            if case == "combined" {
                let derived = export
                    .functions
                    .iter()
                    .find(|(_, function)| function.name == "Derived.read")
                    .unwrap()
                    .0;
                let protocol = export
                    .source_parameter_interfaces
                    .iter()
                    .find(|protocol| protocol.owner == hir::ExportParameterOwner::Function(derived))
                    .unwrap();
                let hir::ExportParameterCalling::Default { source, .. } =
                    protocol.parameters[0].calling
                else {
                    panic!("inherited default")
                };
                let source = &export.export_default_sources[source];
                assert!(!source.type_arguments.is_empty());
                let hir::LexicalDefinitionRoot::Function(provider) =
                    export.export_default_exprs[source.expression].definition_root
                else {
                    panic!("original function provider")
                };
                assert_eq!(export.functions[provider].name, "SourceBase.read");
                assert_ne!(provider, derived);
            }
            let mut identities = source_inventory::identity_closure(output);
            let mut names = BTreeMap::new();
            for (id, class) in export.classes.iter() {
                if let Some(source) = export.nominal_identities[id].source() {
                    let owner = match source {
                        hir::HirSourceNominalIdentity::Concrete(record) => {
                            hir::SourceNominalId::Concrete(record.id())
                        }
                        hir::HirSourceNominalIdentity::Generic(record) => {
                            hir::SourceNominalId::GenericTemplate(record.id())
                        }
                    };
                    names.insert(owner, class.name.clone());
                    let domain =
                        hir::AccessDomain::from_constraints([hir::AccessConstraint::SubclassesOf(
                            id,
                        )]);
                    let projected = Domain::from_export_hir(export, &domain, &mut meter()).unwrap();
                    assert_eq!(projected.constraints(), &[Constraint::SubclassesOf(owner)]);
                }
            }
            let mut rows = BTreeSet::new();
            for expression in export.export_default_exprs.iter().map(|(_, value)| value) {
                let refs = &expression.references;
                let references = refs
                    .callables
                    .iter()
                    .map(|r| ("callable", &r.witness))
                    .chain(
                        refs.constructors
                            .iter()
                            .map(|r| ("constructor", &r.witness)),
                    )
                    .chain(refs.types.iter().map(|r| ("type", &r.witness)))
                    .chain(refs.globals.iter().map(|r| ("global", &r.witness)))
                    .chain(
                        refs.singleton_values
                            .iter()
                            .map(|r| ("singleton", &r.witness)),
                    )
                    .chain(refs.fields.iter().map(|r| ("field", &r.witness)));
                for (kind, witness) in references {
                    let projected = hir::ExportDefaultAccessWitnessV1::from_export_hir(
                        export,
                        witness,
                        &mut meter(),
                    )
                    .unwrap();
                    let bytes = encode(&projected).unwrap();
                    let decoded: hir::DecodedExportDefaultAccessWitnessV1 =
                        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
                    let restored = decoded
                        .resolve(&mut identities, &mut meter(), &WirePath::root())
                        .unwrap();
                    assert_eq!(restored, projected);
                    assert_eq!(encode(&restored).unwrap(), bytes);
                    let hir::ExportParameterOwner::Function(id) = witness.owner else {
                        panic!("function fixture")
                    };
                    rows.insert(format!(
                        "{} {kind}: direct={} slot={} target={}",
                        export.functions[id].name,
                        render(projected.direct_call_domain(), &names),
                        projected
                            .slot_call_domain()
                            .map(|slot| render(slot, &names))
                            .unwrap_or_else(|| "absent".into()),
                        render(projected.target_domain(), &names)
                    ));
                }
            }
            assert!(!rows.is_empty());
            let actual = rows.into_iter().collect::<Vec<_>>().join("\n") + "\n";
            assert_eq!(
                actual,
                std::fs::read_to_string(directory.join(format!("{case}.snap"))).unwrap()
            );
        });
    }
}

#[test]
fn portable_domains_do_not_widen_protected_default_access() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-source-access-domains/errors/protected-default-private-call.scoop"
    ));
    let diagnostics =
        lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap_err();
    let failures = diagnostics.iter().filter(|d| d.message == "default expression references a callable outside the callable's complete call domain").collect::<Vec<_>>();
    assert_eq!(failures.len(), 1, "{diagnostics:?}");
    let start = source.rfind("seed()").unwrap() as u32;
    assert_eq!(failures[0].file, 1);
    assert_eq!(failures[0].span, Some(ast::Span::new(start, start + 6)));
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn render(domain: &Domain, names: &BTreeMap<hir::SourceNominalId, String>) -> String {
    if domain.is_empty() {
        return "empty".into();
    }
    if domain.is_universal() {
        return "universal".into();
    }
    domain
        .constraints()
        .iter()
        .map(|constraint| match constraint {
            Constraint::Cone(_) => "Cone".into(),
            Constraint::File(_) => "File".into(),
            Constraint::LexicalOwner(owner) => format!("Owner({})", names[owner]),
            Constraint::SubclassesOf(owner) => format!("Subclasses({})", names[owner]),
        })
        .collect::<Vec<_>>()
        .join(" & ")
}
