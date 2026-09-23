use super::*;
use hir::concrete::{Module, TypeId, TypeKind};
use source_dispatch::with_hir_source;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-fact-providers/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-fact-providers/combined.scoop"
));

#[test]
fn fact_providers_keep_source_nominals_and_structural_support_in_their_actual_cones() {
    for (case, source) in [("standalone", STANDALONE), ("combined", COMBINED)] {
        with_hir_source(source, |output, _| {
            let public = public_interface(output);
            let production = produce_cross_cone_type_semantics(output, &public).unwrap();
            let local = output.output().local.module();
            let mut rows = Vec::new();
            for fact in production.section().exact_facts().records() {
                let ty = local
                    .exact_type_identities
                    .type_for_identity(fact.exact())
                    .unwrap();
                assert!(
                    !production
                        .dependency_facts()
                        .iter()
                        .any(|dependency| dependency.exact == fact.exact())
                );
                rows.push(format!(
                    "local {} {:?} {:?}\n",
                    type_name(local, ty),
                    fact.kind(),
                    fact.gc()
                ));
            }
            for dependency in production.dependency_facts() {
                let ty = local
                    .exact_type_identities
                    .type_for_identity(dependency.exact)
                    .unwrap();
                assert_eq!(dependency.provider, ConeIdentity::CORE);
                let expected = match &local.core_protocols {
                    hir::concrete::ConcreteCoreProtocols::Imported(protocols) => {
                        let roles = protocols.fundamental_types();
                        match local.types[ty].kind {
                            TypeKind::Integer(kind) => roles.integer(kind).persistent(),
                            TypeKind::Boolean => roles.boolean().persistent(),
                            TypeKind::String => roles.string().persistent(),
                            TypeKind::Unit => scoop_identity::CoreBuiltinNominal::Unit
                                .identity_record()
                                .id(),
                            _ => panic!("unexpected dependency in the fixture"),
                        }
                    }
                    _ => panic!("ordinary sources import their fundamental roles"),
                };
                assert_eq!(
                    local.exact_type_identities[ty].key(),
                    &scoop_identity::ExactTypeKey::Nominal(expected)
                );
                rows.push(format!(
                    "dependency {} provider=core\n",
                    type_name(local, ty)
                ));
            }
            rows.sort();
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../tests/fixtures/m23-fact-providers/{case}.snap"
            ));
            if std::env::var_os("SCOOP_UPDATE_FACT_PROVIDER_SNAPSHOTS").is_some() {
                std::fs::write(&path, rows.concat()).unwrap();
            }
            assert_eq!(rows.concat(), std::fs::read_to_string(path).unwrap());
        });
    }
}

fn type_name(local: &Module, ty: TypeId) -> String {
    match &local.types[ty].kind {
        TypeKind::Unit => "Unit".into(),
        TypeKind::Any => "Any".into(),
        TypeKind::Integer(kind) => kind.canonical_name().into(),
        TypeKind::Boolean => "Boolean".into(),
        TypeKind::String => "String".into(),
        TypeKind::Struct(id) => local.structs[*id].name.clone(),
        TypeKind::Enum(id) => local.enums[*id].name.clone(),
        TypeKind::Class(id) => local.classes[*id].name.clone(),
        TypeKind::Interface(id) => local.interfaces[*id].name.clone(),
        TypeKind::Tuple(elements) => format!(
            "({})",
            elements
                .iter()
                .map(|id| type_name(local, *id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeKind::Function(_) | TypeKind::FunPtr(_) | TypeKind::Ptr(_) => {
            panic!("fixture has no callable or pointer type")
        }
    }
}
