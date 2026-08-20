use super::*;
use scoop_identity::{
    CResultAdaptation, CanonicalCAbiFunctionSignature, CanonicalCAbiParameter, CanonicalCAbiReturn,
    CanonicalCAbiSignatureFingerprintRecord, CanonicalCStorageType, GeneratedBridgeUnitKey,
    IntegerBitWidth, Signedness,
};

#[test]
fn errno_bridge_snapshots_before_copying_and_keeps_the_native_signature() {
    for has_result in [false, true] {
        let module = errno_module(has_result);
        let sources = crate::c_bridge::render_c_bridge_source_set_for_module(&module).unwrap();
        assert_eq!(sources.units().len(), 2);
        let capture = sources
            .units()
            .iter()
            .find(|unit| unit.source().contains("captured_errno"))
            .unwrap()
            .source();
        let plain = sources
            .units()
            .iter()
            .find(|unit| !unit.source().contains("captured_errno"))
            .unwrap()
            .source();
        let unpack = capture.find("__builtin_memcpy(&value0").unwrap();
        let clear = capture.find("errno = 0;").unwrap();
        let call = capture.find("test_bridge_31(value0);").unwrap();
        let snapshot = capture.find("int captured_errno = errno;").unwrap();
        let returned = capture.find("return (int32_t)captured_errno;").unwrap();
        assert!(unpack < clear && clear < call && call < snapshot && snapshot < returned);
        assert_eq!(
            &capture[call + "test_bridge_31(value0);".len()..snapshot],
            "\n  "
        );
        assert!(capture.contains("#include <errno.h>"));
        assert!(!plain.contains("errno = 0;") && !plain.contains("<errno.h>"));
        if has_result {
            let copied = capture.find("__builtin_memcpy(result,").unwrap();
            assert!(snapshot < copied && copied < returned);
            assert!(capture.contains("int32_t native_result = test_bridge_31(value0);"));
            assert!(capture.contains("void *result, const void *arg0"));
        } else {
            assert!(!capture.contains("void *result") && !capture.contains("native_result"));
            assert!(capture.contains("extern void test_bridge_31(int32_t)"));
        }
    }
}

fn errno_module(has_result: bool) -> Module {
    let mut module = values_module();
    let exact_type = test_exact_type("errnoInteger");
    let storage = CanonicalCStorageType::Integer {
        exact_type,
        signedness: Signedness::Signed,
        bit_width: IntegerBitWidth::Bits32,
    };
    let signature =
        CanonicalCAbiSignatureFingerprintRecord::new(CanonicalCAbiFunctionSignature::cdecl(
            vec![CanonicalCAbiParameter::new(exact_type, storage).unwrap()],
            if has_result {
                CanonicalCAbiReturn::Value {
                    source_exact_type: exact_type,
                    storage,
                }
            } else {
                CanonicalCAbiReturn::Void
            },
        ))
        .unwrap();
    install_test_native_function_contract_with_signature(&mut module, 31, &signature);
    let ordinary = outbound_bridge_with_signature(31, &signature);
    let GeneratedBridgeUnitKey::OutboundFunction(contract, _) = *ordinary.unit_record().key()
    else {
        unreachable!()
    };
    for result in [CResultAdaptation::Direct, CResultAdaptation::CaptureErrno] {
        let entry = scoop_lir::GeneratedBridgeEntryIdentity::new(
            module.cone,
            GeneratedBridgeUnitKey::OutboundFunction(contract, result),
        )
        .unwrap();
        module.extern_functions.alloc_c(scoop_lir::CExternFunction {
            call_mode: scoop_lir::CAbiCallMode::GcLeaf,
            identity: scoop_lir::ExternFunctionIdentity {
                source_name: format!("errno{result:?}"),
                native_symbol: "test_bridge_31".into(),
                library: String::new(),
                calling_convention: scoop_lir::CallingConvention::Cdecl,
            },
            call_plan: scoop_lir::CAbiCallPlan::StorageBridge {
                entry: Box::new(entry),
                result,
            },
            signature: scoop_lir::CFunctionType {
                params: vec![scoop_lir::CType::Integer(IntegerKind::SIGNED_32)],
                return_type: if has_result {
                    scoop_lir::CReturnType::Value(Box::new(scoop_lir::CType::Integer(
                        IntegerKind::SIGNED_32,
                    )))
                } else {
                    scoop_lir::CReturnType::Void
                },
            },
        });
    }
    module
}
