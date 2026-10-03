use super::*;

#[test]
fn imported_handles_keep_the_c_projection_without_a_provider_native_use() {
    for case in ["handle-c", "handle-c-combined"] {
        let source =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                format!("../../tests/fixtures/m23-native-boundary/{case}.scoop"),
            ))
            .unwrap();
        source_dispatch::with_hir_source(&source, |output, core| {
            let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
            let ffi = protocols.protocols().ffi();
            let handles = if case == "handle-c" {
                vec![ffi.gc_handle()]
            } else {
                vec![ffi.gc_handle(), ffi.pinned_ptr()]
            };
            for handle in handles {
                let owner = handle.persistent();
                let native_owner = hir::NativeBoundaryNominalOwner::GenericTemplate(owner);
                assert!(
                    core.output
                        .output()
                        .native_boundary_types
                        .records()
                        .iter()
                        .all(|record| record.owner() != native_owner)
                );
                let loaded = &output.output().export.loaded_struct_definitions
                    [&hir::SourceNominalId::GenericTemplate(owner)];
                let [field] = loaded
                    .declaration
                    .interface
                    .source_shape()
                    .declared_fields()
                else {
                    panic!("the handle retains its unique declared field")
                };
                let projection = hir::NativeBoundaryCAbiV1::UInt64Field {
                    field: field.field(),
                };
                assert_eq!(loaded.declaration.c_abi, projection);
                let native = output
                    .output()
                    .native_boundary_types
                    .records()
                    .iter()
                    .find(|record| record.owner() == native_owner)
                    .unwrap();
                assert_eq!(native.c_abi(), projection);
                assert!(output.output().local.structs.iter().any(|(_, definition)| {
                    matches!(&definition.representation,
                        hir::concrete::StructRepresentation::Declared {
                            c_abi: hir::concrete::StructCAbi::UInt64Field { field: actual },
                            fields, ..
                        } if *actual == field.field() && fields.len() == 1 && definition.gc_free)
                }));
            }
        });
    }
}
