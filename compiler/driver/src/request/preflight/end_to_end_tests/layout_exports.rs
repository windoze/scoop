use super::*;
use scoop_hir as hir;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WireDecode, WireEncode, decode_canonical, encode};

mod assertions;
mod core;
mod dependencies;
mod private_types;
mod rejections;
mod shared_abis;
mod shared_descriptors;
mod shared_dispatch;
mod shared_initialization;
mod shared_layouts;
mod shared_ordinary;
mod shared_shapes;
mod source_contracts;
mod source_uses;
mod support;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

#[test]
fn actual_source_mir_and_lir_assemble_complete_layout_exports() {
    let target = resolved_target().expect("layout production requires the supported host target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let bytes = std::fs::read(core.artifact().path()).unwrap();
    for name in ["standalone", "combined", "private-support"] {
        let path = crate::workspace_root().join("tests/fixtures/m23-lir-export-assembly");
        let source = std::fs::read_to_string(path.join(format!("{name}.scoop"))).unwrap();
        let mut expected = None;
        support::with_production(
            sysroot.path(),
            &target,
            &bytes,
            &source,
            |input, dependencies| {
                let result =
                    scoop_lir_lower::lower_layout_abi_exports(input, dependencies, &mut meter())
                        .unwrap_or_else(|error| {
                            let missing = input
                                .lir
                                .module()
                                .meta
                                .layouts
                                .iter()
                                .filter_map(|(_, layout)| {
                                    let exact = layout.identity.layout_record().key().exact_type();
                                    input
                                        .bridge
                                        .types()
                                        .get(exact)
                                        .is_none()
                                        .then_some((&layout.name, exact))
                                })
                                .collect::<Vec<_>>();
                            let callable = match &error {
                                scoop_lir_lower::LayoutAbiExportLoweringError::Callable { target, .. } => input.mir.materialization().callable_roots().iter()
                                    .find(|root| root.implementation() == target.callable_owner())
                                    .map(|root| &input.mir.module().functions[root.function()].name),
                                _ => None,
                            };
                            panic!("{name}: {error}; callable: {callable:?}; physical layouts absent from MIR exports: {missing:?}")
                        });
                assertions::actual(input, &result);
                shared_layouts::check(input, dependencies, &result);
                shared_abis::check(input, dependencies, &result);
                shared_dispatch::check(input, dependencies, &result);
                shared_descriptors::check(input, &result);
                shared_shapes::check(input, &result);
                rejections::check(input, dependencies);
                if name == "private-support" {
                    private_types::check_support(input, dependencies, &result);
                }
                expected = Some(assertions::bytes(&result));
                let dump = assertions::dump(input, &result);
                if let Some(directory) = std::env::var_os("SCOOP_LIR_EXPORT_SNAPSHOT_DIR") {
                    std::fs::create_dir_all(&directory).unwrap();
                    std::fs::write(Path::new(&directory).join(format!("{name}.snap")), dump)
                        .unwrap();
                } else {
                    assert_eq!(
                        dump,
                        std::fs::read_to_string(path.join(format!("{name}.snap"))).unwrap()
                    );
                }
            },
        );
        support::with_production(
            sysroot.path(),
            &target,
            &bytes,
            &format!(
                "{}\nprivate fun unrelated(): Boolean = true\n{source}",
                std::fs::read_to_string(path.join("private-local.scoop")).unwrap()
            ),
            |input, dependencies| {
                let result =
                    scoop_lir_lower::lower_layout_abi_exports(input, dependencies, &mut meter())
                        .unwrap();
                private_types::check(input, &result);
                shared_layouts::check(input, dependencies, &result);
                shared_abis::check(input, dependencies, &result);
                shared_dispatch::check(input, dependencies, &result);
                shared_descriptors::check(input, &result);
                shared_shapes::check(input, &result);
                assert_eq!(assertions::bytes(&result), expected.unwrap());
            },
        );
    }
}
