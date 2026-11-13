use super::*;
use hir::IrTemplateExport::{Default, Full, Off};

#[test]
fn export_ir_modes_preserve_generic_and_default_execution() {
    let provider = r#"
        package export.mode
        private fun helper(value: Int): Int = value + 1
        private fun unused(value: Int): Int = value * 2
        public fun ordinary(value: Int): Int = helper(value)
        public fun <T> generic(value: T): T = value
        public fun defaulted(value: Int = 7, read: () -> Int = { value }): Int = read()
    "#;
    let consumer = r#"
        import export.mode.*
        public fun result(): Int = generic(defaulted()) + ordinary(1)
    "#;
    let mut identities = Vec::new();
    for mode in [Off, Default, Full] {
        with_provider_consumer_export_ir(provider, consumer, mode, |output, _, _, section, _| {
            assert_eq!(section.ir_template_export(), mode);
            let records = section.generic_callable_bodies().records();
            assert!(records.iter().any(|body| matches!(
                body.owner(),
                hir::DefaultCallableDeclarationV1::GenericFunction(_)
            )));
            assert!(records.iter().any(|body| matches!(
                body.owner(),
                hir::DefaultCallableDeclarationV1::Generated(_)
            )));
            let ordinary = records
                .iter()
                .filter(|body| {
                    matches!(body.owner(), hir::DefaultCallableDeclarationV1::Function(_))
                })
                .count();
            assert_eq!(
                ordinary,
                match mode {
                    Off => 0,
                    Default => 2,
                    Full => 4,
                }
            );
            identities.push(section.public_bindings().clone());
            let local = output.output().local.module();
            assert_eq!(local.imported_inline_bodies.is_empty(), mode == Off);
            assert!(
                local
                    .functions
                    .values()
                    .all(|function| function.name != "unused")
            );
            for template in local.imported_inline_bodies.values() {
                assert!(
                    local
                        .functions
                        .values()
                        .all(|function| function.materialization != template.materialization)
                );
            }
            let dependencies = select_library_callables(&output);
            scoop_mir_lower::lower_current_cone(
                &output,
                dependencies,
                scoop_mir_lower::MirOptimizationOptions::RELEASE,
                None,
                scoop_mir_lower::ProfilingInput::Off,
            )
            .expect("templates preserve calls and default argument execution");
            let mut indexed = section.clone();
            let bytes = scoop_wire::encode(&indexed.index_for_wire().unwrap()).unwrap();
            let decoded: hir::DecodedCrossConeHirInterfaceSectionV1 =
                scoop_wire::decode_canonical(&bytes).unwrap();
            assert_eq!(scoop_wire::encode(&decoded).unwrap(), bytes);
        })
        .unwrap_or_else(|errors| panic!("{mode}: {errors:?}"));
    }
    assert!(identities.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn full_exports_unused_private_nominal_initialization_without_materializing_it() {
    let provider = r#"
        package export.mode
        private class Hidden private constructor(private val value: Int) {
            private var extra: Int = value + 1
            init { extra += value }
            private fun read(): Int = extra
        }
        public struct Pair(val value: Int) {
            public constructor(): this(7) {}
        }
        public fun answer(): Int = 42
    "#;
    for mode in [Off, Default, Full] {
        with_provider_consumer_export_ir(
            provider,
            "import export.mode.answer\npublic fun result(): Int = answer()",
            mode,
            |output, _, _, section, _| {
                let records = section.generic_initializations().records();
                if mode == Full {
                    assert_eq!(records.len(), 2);
                    assert!(
                        records.iter().all(|record| matches!(
                            record.owner(),
                            hir::SourceNominalId::Concrete(_)
                        ))
                    );
                    let class = records
                        .iter()
                        .find(|record| !record.common().is_empty())
                        .unwrap();
                    assert_eq!(class.common().len(), 2);
                    let structure = records
                        .iter()
                        .find(|record| record.common().is_empty())
                        .unwrap();
                    assert_eq!(structure.constructors().len(), 2);
                } else {
                    assert!(records.is_empty());
                }
                let local = output.output().local.module();
                assert!(local.classes.values().all(|class| class.name != "Hidden"));
                assert!(
                    local
                        .functions
                        .values()
                        .all(|function| function.name != "read")
                );
            },
        )
        .unwrap_or_else(|errors| panic!("{mode}: {errors:?}"));
    }
}

#[test]
fn full_keeps_ordinary_closure_bodies_and_consumes_only_called_templates() {
    let provider = r#"
        package export.mode
        public fun make(value: Int): () -> Int {
            val first: () -> Int = { value }
            return { first() + value }
        }
        private fun unused(value: Int): () -> Int = { value * 2 }
    "#;
    with_provider_consumer_export_ir(
        provider,
        "import export.mode.make\npublic fun result(): Int = make(7)()",
        Full,
        |output, _, _, section, _| {
            assert!(section.generic_callable_bodies().records().len() >= 5);
            let local = output.output().local.module();
            assert_eq!(local.imported_inline_bodies.len(), 1);
            assert!(
                output
                    .output()
                    .export
                    .module()
                    .lambdas
                    .values()
                    .any(|lambda| matches!(
                        lambda.definition,
                        hir::LexicalFunctionDefinition::Template(_)
                    ))
            );
            let dependencies = select_library_callables(&output);
            scoop_mir_lower::lower_current_cone(
                &output,
                dependencies,
                scoop_mir_lower::MirOptimizationOptions::RELEASE,
                None,
                scoop_mir_lower::ProfilingInput::Off,
            )
            .expect("complete imported closures lower without provider Strong definitions");
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn full_keeps_context_suspend_and_finally_through_wire() {
    let provider = r#"
        package export.mode
        context(value: String)
        public fun contextual(): String = value
        public suspend fun delayed(value: Int): Int = value
        public fun guarded(value: Int): Int {
            var result = value
            try { result += 1 } finally { result += 2 }
            return result
        }
    "#;
    let consumer = r#"
        import export.mode.*
        public fun text(): String = contextual()
        public suspend fun wait(value: Int): Int = delayed(value)
        public fun result(value: Int): Int = guarded(value)
    "#;
    with_provider_consumer_export_ir(provider, consumer, Full, |output, _, _, section, _| {
        let records = section.generic_callable_bodies().records();
        assert!(
            records
                .iter()
                .any(|body| !body.context_parameters().is_empty())
        );
        assert!(
            records
                .iter()
                .any(|body| body.effects().execution() == scoop_identity::Effect::Suspend)
        );
        assert!(records.iter().any(|body| {
            body.statements()
                .iter()
                .any(|statement| matches!(statement.kind(), hir::DefaultStatementKindV1::Try(_)))
        }));
        assert_eq!(
            output.output().local.module().imported_inline_bodies.len(),
            3
        );
        assert!(
            output
                .output()
                .local
                .module()
                .imported_inline_bodies
                .values()
                .any(|body| body.is_suspend)
        );
    })
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    wire_equivalence::compare_consumers_export_ir(
        "full context/suspend/finally",
        provider,
        consumer,
        Full,
        false,
    );
}
