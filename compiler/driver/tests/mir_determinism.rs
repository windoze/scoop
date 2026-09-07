//! MIR lowering must be deterministic: the same concrete module lowered
//! twice (in one process) yields identical entity order. Regression test
//! for the interface-slot HashMap ordering fix.

use std::path::Path;

#[test]
fn probe_concrete_enum_order_is_stable_in_process() {
    let file =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m0-smoke/hello.scoop");
    let inputs = scoopc::load_inputs(&file).unwrap();
    let parse = |text: &str| scoop_parser::parse(text).unwrap();
    let mut files = Vec::new();
    for input in &inputs {
        files.push(parse(&input.source));
    }
    let user_index = files.len() - 1;
    fn build_unit<'a>(
        files: &'a [scoop_ast::SourceFile],
        user_index: usize,
    ) -> scoop_hir_lower::CompilationUnit<'a> {
        let core_provider = scoop_hir::IntrinsicProviderId::from_raw(0);
        let user_provider = scoop_hir::IntrinsicProviderId::from_raw(1);
        scoop_hir_lower::CompilationUnit {
            cone: scoop_hir_lower::test_cone_identity(),
            core: files[..user_index]
                .iter()
                .map(|source| scoop_hir_lower::ProviderSource {
                    source,
                    provider: core_provider,
                    name: "<core>",
                    source_text: "",
                })
                .collect(),
            user: scoop_hir_lower::ProviderSource {
                source: &files[user_index],
                provider: user_provider,
                name: "<user>",
                source_text: "",
            },
        }
    }
    let policy = scoop_hir_lower::IntrinsicDeclarationPolicy::CoreOnly;
    let first =
        scoop_hir_lower::lower_compilation_unit(&build_unit(&files, user_index), policy.clone())
            .unwrap();
    let second =
        scoop_hir_lower::lower_compilation_unit(&build_unit(&files, user_index), policy).unwrap();
    let names = |m: &scoop_hir::concrete::Module| {
        m.enums
            .iter()
            .map(|(_, def)| def.name.clone())
            .collect::<Vec<_>>()
    };
    // Print both orders for diagnosis.
    let mir1 = scoop_mir_lower::lower(&first.local);
    let mir2 = scoop_mir_lower::lower(&second.local);
    let mir_enums = |m: &scoop_mir::Module| {
        m.enums
            .iter()
            .map(|(_, d)| d.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        mir_enums(&mir1),
        mir_enums(&mir2),
        "MIR enum order differs in process"
    );
    assert_eq!(names(&first.local), names(&second.local));
}
