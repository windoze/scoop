use super::*;
use object::{Object, ObjectSymbol, SymbolKind};
use scoop_lir::{RuntimeAbiSymbolV1, RuntimeSymbolContractRegistryV1};
use scoop_process::CommandExt;
use scoop_toolchain::{ValidatedRuntimeBuildProfile, resolve_linux_c_toolchain};
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn linux_runtime_sources_compile_and_define_the_complete_runtime_abi() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for target in [
        LirTargetProfile::LINUX_X86_64_GNU,
        LirTargetProfile::LINUX_X86_64_MUSL,
    ] {
        let invocation = resolve_linux_c_toolchain(target, None, None).unwrap();
        let profile = ValidatedRuntimeBuildProfile::for_target(target);
        let directory = tempfile::tempdir().unwrap();
        let unwind = workspace
            .join("sysroot/native")
            .join(profile.canonical_triple())
            .join("unwind/include");
        assert!(unwind.join("unwind.h").is_file());
        let mut definitions = BTreeMap::new();
        let mut objects = Vec::new();
        for (index, source) in profile.runtime_sources().iter().enumerate() {
            let path = directory.path().join(format!("runtime-{index}.o"));
            let mut command = invocation.object_compilation_command(&workspace.join(source), &path);
            command
                .args(profile.runtime_c_flags())
                .args(["-O2", "-Wall", "-Wextra", "-Werror", "-funwind-tables"])
                .arg("-I")
                .arg(&unwind);
            for include in profile.include_directories() {
                command
                    .arg("-I")
                    .arg(workspace.join("runtime").join(include));
            }
            let output = command.scoop_output().unwrap();
            assert!(
                output.status.success(),
                "{}: {source}: {}",
                profile.canonical_triple(),
                String::from_utf8_lossy(&output.stderr)
            );
            let bytes = std::fs::read(&path).unwrap();
            let file = object::File::parse(bytes.as_slice()).unwrap();
            for symbol in file.symbols().filter(|symbol| symbol.is_global()) {
                // ObjectSymbol::is_definition excludes ELF STT_TLS.
                if symbol.section_index().is_none() {
                    continue;
                }
                let name = symbol.name().unwrap().to_owned();
                assert!(!symbol.is_weak(), "weak runtime definition {name}");
                assert!(
                    definitions.insert(name.clone(), symbol.kind()).is_none(),
                    "duplicate runtime definition {name}"
                );
            }
            objects.push(bytes);
        }
        let registry = RuntimeSymbolContractRegistryV1::current(target).unwrap();
        for contract in registry.contracts() {
            let name = String::from_utf8(contract.object_symbol(target)).unwrap();
            let kind = match contract.symbol() {
                RuntimeAbiSymbolV1::CoreStringTypeDescriptor => {
                    assert!(!definitions.contains_key(&name));
                    continue;
                }
                RuntimeAbiSymbolV1::AllocationContext => SymbolKind::Tls,
                RuntimeAbiSymbolV1::CardTable => SymbolKind::Data,
                _ => SymbolKind::Text,
            };
            assert_eq!(definitions.get(&name), Some(&kind), "{name}");
        }
        assert_eq!(
            definitions.get("scoop_rt_run_program"),
            Some(&SymbolKind::Text)
        );
        assert!(!definitions.contains_key("main"));
        assert!(!definitions.keys().any(|name| name.starts_with("mbedtls_")));
        let runtime = RuntimeObjectSet::from_objects(
            target,
            invocation.profile(),
            RuntimeBuildConfiguration {
                input_key: sha256(b"runtime index integration"),
                compiler_digest: sha256(&std::fs::read(invocation.compiler_driver()).unwrap()),
                flags: profile
                    .runtime_c_flags()
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
            },
            objects,
        )
        .unwrap();
        let index = runtime
            .write_index(&directory.path().join("index"))
            .unwrap();
        let decoded = RuntimeObjectSet::read_index(&index, target, invocation.profile()).unwrap();
        assert_eq!(decoded.fingerprint(), runtime.fingerprint());
        assert_eq!(decoded.symbols(), runtime.symbols());
        assert_eq!(decoded.target(), target);
        let other = if target == LirTargetProfile::LINUX_X86_64_GNU {
            LirTargetProfile::LINUX_X86_64_MUSL
        } else {
            LirTargetProfile::LINUX_X86_64_GNU
        };
        assert!(RuntimeObjectSet::read_index(&index, other, invocation.profile()).is_err());
        let first = &decoded.input_paths()[1];
        std::fs::write(first, b"truncated").unwrap();
        assert!(RuntimeObjectSet::read_index(&index, target, invocation.profile()).is_err());
    }
}
