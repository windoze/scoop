use object::{ObjectSymbol, SymbolFlags, elf};
use scoop_identity::{
    CanonicalCAbiFunctionSignature, CanonicalCAbiParameter, CanonicalCAbiReturn,
    CanonicalCAbiSignatureFingerprintRecord, CanonicalCStorageType, GeneratedBridgeUnitKey,
    NativeExternalContract, NativeExternalContractRecord, NativeExternalSymbolKey,
    NativeLibraryBinding, PersistentSourceNativeExternalContractId,
    SourceNativeExternalContractKey, SourceNativeSymbol,
};
use scoop_lir::{GeneratedBridgeEntryIdentity, NativeGlobalAccess};

use super::*;
use crate::tests::c_layout::{c_opaque_pointer, c_value};

#[test]
fn linux_generated_c_requirements_cover_native_tls_and_callback_contracts() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let (module, _) = fixture(target);
        let fixture = super::slib_support::SlibObjects::new(module, directory.path());
        let builtins = fixture.builtins(&fixture.objects);
        let sites = fixture.sites(builtins, &fixture.objects);
        let requirements = fixture.requirements(sites);
        assert_eq!(requirements.selection().target().id(), target);
        assert!(requirements.requirements().iter().any(|requirement| {
            requirement.use_site().symbol() == b"__tls_get_addr"
                && matches!(
                    requirement.requirement(),
                    scoop_slib::FinalUndefinedSymbolRequirementV1::CBridgeTargetSupport { .. }
                )
        }));
    }
}

#[test]
fn linux_generated_c_functions_tls_and_callback_link_and_run() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let (module, harness) = fixture(target);
        let invocation =
            scoop_toolchain::resolve_linux_c_toolchain(module.meta.target_profile, None, None)
                .unwrap();
        let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
        let emitted = emit_c_bridge_object_set(&input, directory.path(), &invocation).unwrap();
        assert_eq!(emitted.members().len(), 5);
        let surface =
            scoop_lir::ObjectSymbolSurfaceV1::from_foundation(input.foundation()).unwrap();
        let partition =
            scoop_lir::ProducerUnitPartitionV1::from_foundation(input.foundation()).unwrap();
        let members = scoop_slib::PlannedLinkObjectMemberSetV1::new(
            input.module().meta.target_profile,
            &partition,
            vec![
                scoop_slib::CanonicalScoopLirObjectUnitSetV1::new(
                    partition.scoop_lir_definition_plans().to_vec(),
                )
                .unwrap(),
            ],
            emitted
                .members()
                .iter()
                .map(|m| {
                    scoop_slib::CanonicalGeneratedBridgeObjectUnitSetV1::new(vec![m.unit()])
                        .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let symbols = scoop_slib::PlannedStrongObjectSymbolSetV1::new(
            input.module().meta.target_profile,
            &surface,
            &members,
        )
        .unwrap();
        for member in emitted.members() {
            let bytes = std::fs::read(member.object_path()).unwrap();
            let envelope = scoop_slib::validate_generated_c_object_for_profile_v1(
                &bytes,
                invocation.profile(),
            )
            .unwrap();
            let member_id = members
                .member_for_generated_bridge_unit(member.unit())
                .unwrap();
            let definitions = scoop_slib::verify_member_strong_object_definitions_v1(
                &bytes,
                envelope.into_sections(),
                symbols.member(member_id).unwrap(),
            )
            .unwrap();
            scoop_slib::verify_member_object_relocations_v1(definitions).unwrap();
            let file = object::File::parse(bytes.as_slice()).unwrap();
            assert!(file.section_by_name(".eh_frame").is_none());
            for atom in std::iter::once(member.plan().primary_atom_authority())
                .chain(member.plan().materialized_associated_atom_authorities())
            {
                let plan = surface
                    .plans()
                    .iter()
                    .find(|plan| {
                        plan.owner().kind()
                            == scoop_lir::StrongDefinitionEntityKind::GeneratedBridgeAtom(atom.id())
                    })
                    .unwrap();
                let primary = file
                    .symbol_by_name(plan.primary_symbol().symbol().as_str())
                    .unwrap();
                let [boundary] = plan.atom_boundaries() else {
                    panic!("one bridge extent")
                };
                let start = file
                    .symbol_by_name(boundary.start().symbol().as_str())
                    .unwrap();
                let end = file
                    .symbol_by_name(boundary.end().symbol().as_str())
                    .unwrap();
                assert_eq!(start.section_index(), primary.section_index());
                assert_eq!(end.section_index(), primary.section_index());
                assert_eq!(start.address(), primary.address());
                assert_eq!(end.address(), primary.address() + primary.size());
                assert_eq!(end.size(), 0);
                for symbol in [primary, start, end] {
                    assert!(
                        matches!(symbol.flags(), SymbolFlags::Elf { st_info, st_other }
                        if st_info >> 4 == elf::STB_GLOBAL && st_other & 3 == elf::STV_HIDDEN)
                    );
                }
            }
        }
        let source = directory.path().join("harness.c");
        let program = directory.path().join("bridge-test");
        std::fs::write(&source, harness).unwrap();
        let mut link = invocation.driver_command();
        link.arg(if target == TargetProfileId::LinuxX86_64Musl {
            "-static"
        } else {
            "-pie"
        });
        let result = link
            .arg(source)
            .args(emitted.members().iter().map(|m| m.object_path()))
            .arg("-o")
            .arg(&program)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            std::process::Command::new(program)
                .status()
                .unwrap()
                .success()
        );
    }
}

fn fixture(target: TargetProfileId) -> (Module, String) {
    let mut module = for_target(values_module(), target);
    let integer = CanonicalCStorageType::Integer {
        exact_type: test_exact_type("elfBridgeI64"),
        signedness: scoop_identity::Signedness::Signed,
        bit_width: scoop_identity::IntegerBitWidth::Bits64,
    };
    let pointer = CanonicalCStorageType::DataPointer {
        exact_type: test_exact_type("elfBridgePointer"),
        pointee: scoop_identity::CDataPointee::OpaqueUnit,
        storage: scoop_identity::CPointerStorage::Direct,
    };
    let outbound_signature = signature(&[integer], integer);
    let function = native_record(
        &module,
        "native_transform",
        false,
        NativeExternalContract::c_function(
            NativeLibraryBinding::DefaultNativeNamespace,
            outbound_signature.signature().clone(),
        ),
    );
    let outbound = bridge(GeneratedBridgeUnitKey::OutboundFunction(
        function.fingerprint(),
    ));
    install_test_c_signature_record(&mut module, outbound_signature);
    module.extern_functions.alloc_c(scoop_lir::CExternFunction {
        call_mode: scoop_identity::CAbiCallMode::NativeSafe,
        identity: scoop_lir::ExternFunctionIdentity {
            source_name: "transform".into(),
            native_symbol: "native_transform".into(),
            library: String::new(),
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        },
        call_plan: scoop_lir::CAbiCallPlan::StorageBridge(Box::new(outbound.clone())),
        signature: scoop_lir::CFunctionType {
            params: vec![scoop_lir::CType::Integer(IntegerKind::SIGNED_64)],
            return_type: c_value(scoop_lir::CType::Integer(IntegerKind::SIGNED_64)),
        },
    });
    let tls = native_record(
        &module,
        "native_tls",
        true,
        NativeExternalContract::mutable_tls(NativeLibraryBinding::DefaultNativeNamespace, integer),
    );
    let read = bridge(GeneratedBridgeUnitKey::GlobalRead(tls.fingerprint()));
    let write = bridge(GeneratedBridgeUnitKey::GlobalWrite(tls.fingerprint()));
    let address = bridge(GeneratedBridgeUnitKey::GlobalAddress(tls.fingerprint()));
    let get = module
        .native_global_bridges
        .gets
        .alloc(scoop_lir::NativeGlobalGetBridge {
            identity: read.clone(),
        });
    let set = module
        .native_global_bridges
        .sets
        .alloc(scoop_lir::NativeGlobalSetBridge {
            identity: write.clone(),
        });
    let addr = module
        .native_global_bridges
        .addresses
        .alloc(scoop_lir::NativeGlobalAddressBridge {
            identity: address.clone(),
        });
    module.native_globals.alloc(scoop_lir::NativeGlobal {
        source_name: "tls".into(),
        native_symbol: "native_tls".into(),
        library: String::new(),
        c_type: scoop_lir::CType::Integer(IntegerKind::SIGNED_64),
        thread_local: true,
        access: NativeGlobalAccess::Mutable {
            get,
            set,
            address: addr,
        },
    });
    module.meta.native_externals =
        scoop_lir::NativeExternalMetadata::checked(vec![function, tls], Vec::new()).unwrap();
    let callback_signature = signature(&[integer, pointer], integer);
    let trampoline = scoop_lir::ManagedCallbackTrampolineIdentity::new(
        ConeIdentity::SINGLE_FILE,
        callback_signature.fingerprint(),
        CallbackParameterIndex::new(1),
    )
    .unwrap();
    install_test_c_signature_record(&mut module, callback_signature);
    let family = c_layout::foreign_callback_family(&mut module);
    let adapter_index = module.functions.len();
    module
        .functions
        .push(c_layout::foreign_callback_adapter("elfCallbackAdapter"));
    module
        .foreign_callback_bridges
        .alloc(scoop_lir::ForeignCallbackBridge {
            application: callback_application(0),
            family,
            adapter: managed_local_function_ref(adapter_index),
            trampoline: trampoline.clone(),
            params: vec![
                scoop_lir::CType::Integer(IntegerKind::SIGNED_64),
                c_opaque_pointer(),
            ],
            return_type: c_value(scoop_lir::CType::Integer(IntegerKind::SIGNED_64)),
            context_index: 1,
            mode: module.foreign_callback_families[family].modes.reusable(),
        });
    let harness = format!(
        r#"
#include <stdint.h>
#include <stdlib.h>
_Thread_local int64_t native_tls = 9;
int64_t native_transform(int64_t value) {{ return value * 3 + 1; }}
extern void {outbound}(void *, const void *);
extern void {read}(void *);
extern void {write}(const void *);
extern void {address}(void *);
extern int64_t {callback}(int64_t, void *);
extern const unsigned char {descriptor};
uint32_t scoop_runtime_callback_invoke(void *context, const void *signature, void *result, const void *const *args) {{
    if (context != (void *)0x1234 || signature != &{descriptor}) abort();
    *(int64_t *)result = *(const int64_t *)args[0] + 2;
    return 0;
}}
int main(void) {{
    int64_t value = 13, result = 0; void *address = 0;
    {outbound}(&result, &value); if (result != 40) return 1;
    {read}(&result); if (result != 9) return 2;
    {write}(&value); {read}(&result); if (result != 13) return 3;
    {address}(&address); if (address != &native_tls) return 4;
    if ({callback}(12, (void *)0x1234) != 14) return 5;
    return 0;
}}
"#,
        outbound = outbound.symbol(),
        read = read.symbol(),
        write = write.symbol(),
        address = address.symbol(),
        callback = trampoline.entry().symbol(),
        descriptor = trampoline.signature_descriptor_symbol()
    );
    (module, harness)
}

fn signature(
    params: &[CanonicalCStorageType],
    result: CanonicalCStorageType,
) -> CanonicalCAbiSignatureFingerprintRecord {
    CanonicalCAbiSignatureFingerprintRecord::new(CanonicalCAbiFunctionSignature::cdecl(
        params
            .iter()
            .map(|storage| CanonicalCAbiParameter::new(storage.exact_type(), *storage).unwrap())
            .collect(),
        CanonicalCAbiReturn::Value {
            source_exact_type: result.exact_type(),
            storage: result,
        },
    ))
    .unwrap()
}

fn native_record(
    module: &Module,
    name: &str,
    global: bool,
    contract: NativeExternalContract,
) -> NativeExternalContractRecord {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let name_id = CanonicalIdentifier::new(name).unwrap();
    let key = if global {
        SourceNativeExternalContractKey::property(&SourceDeclarationKey::property(site, name_id))
    } else {
        SourceNativeExternalContractKey::function(&SourceDeclarationKey::function(
            site,
            name_id,
            0,
            None,
            Vec::new(),
        ))
    }
    .unwrap();
    NativeExternalContractRecord::new(
        PersistentSourceNativeExternalContractId::from_key(&key).unwrap(),
        NativeExternalSymbolKey::for_target(
            module.meta.target_profile.wire_id(),
            &SourceNativeSymbol::new(name).unwrap(),
        )
        .unwrap(),
        contract,
    )
    .unwrap()
}

fn bridge(key: GeneratedBridgeUnitKey) -> GeneratedBridgeEntryIdentity {
    GeneratedBridgeEntryIdentity::new(ConeIdentity::SINGLE_FILE, key).unwrap()
}
