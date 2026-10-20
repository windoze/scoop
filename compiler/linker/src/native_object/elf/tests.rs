use super::*;
use std::process::Command;

fn compile(source: &str, extension: &str, extra: &[&str]) -> Vec<u8> {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join(format!("input.{extension}"));
    let output = directory.path().join("input.o");
    std::fs::write(&input, source).unwrap();
    let mut command = Command::new(if cfg!(target_os = "macos") {
        "clang"
    } else {
        "gcc"
    });
    if cfg!(target_os = "macos") {
        command.arg("--target=x86_64-unknown-linux-gnu");
    }
    let result = command
        .env("TMPDIR", directory.path())
        .args(["-fPIC", "-fno-common", "-g", "-c"])
        .args(extra)
        .arg(input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::read(output).unwrap()
}

#[test]
fn elf_native_objects_keep_tls_visibility_weakness_and_debug_relocations() {
    let bytes = compile(
        "_Thread_local long local_tls; extern _Thread_local long foreign_tls;\n\
         extern long foreign(void); const long read_only = 7; long data;\n\
         __attribute__((weak)) long weak_data;\n\
         long function(void) { return foreign() + local_tls + foreign_tls; }\n",
        "c",
        &[],
    );
    for _target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        let info = read(&bytes).unwrap();
        assert_eq!(
            info.definitions["local_tls"].kind,
            NativeSymbolKind::ThreadLocal
        );
        assert_eq!(
            info.definitions["function"].kind,
            NativeSymbolKind::Function
        );
        assert!(!info.definitions["data"].read_only);
        assert!(info.definitions["read_only"].read_only);
        assert_eq!(
            info.definitions["weak_data"].binding,
            NativeSymbolBinding::Weak
        );
        assert!(info.requirements.contains("foreign"));
        assert!(info.requirements.contains("foreign_tls"));
        assert!(info.requirements.contains("__tls_get_addr"));
    }
}

#[test]
fn elf_native_inputs_index_common_and_cpp_requirements() {
    let bytes = compile(
        "int tentative; extern void __cxa_throw(void); void f(void) { __cxa_throw(); }",
        "c",
        &["-fcommon"],
    );
    let info = read(&bytes).unwrap();
    assert_eq!(
        info.definitions["tentative"].binding,
        NativeSymbolBinding::Common
    );
    assert!(info.requirements.contains("__cxa_throw"));
}

#[test]
fn elf_reader_rejects_a_different_machine() {
    let mut bytes = compile("int value;", "c", &[]);
    bytes[18..20].copy_from_slice(&elf::EM_AARCH64.to_le_bytes());
    assert!(read(&bytes).is_err());
}

#[test]
fn elf_native_inputs_index_libc_exit_cleanup() {
    let bytes = compile(
        "extern int __cxa_atexit(void (*)(void *), void *, void *);\n\
         extern void __cxa_finalize(void *);\n\
         static void cleanup(void *p) { (void)p; }\n\
         int register_cleanup(void) { return __cxa_atexit(cleanup, 0, 0); }\n\
         void finish_cleanup(void) { __cxa_finalize(0); }\n",
        "c",
        &[],
    );
    for _target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        let info = read(&bytes).unwrap();
        assert!(info.requirements.contains("__cxa_atexit"));
        assert!(info.requirements.contains("__cxa_finalize"));
    }
}

#[test]
fn elf_weak_imports_do_not_require_optional_hooks_or_data() {
    let bytes = compile(
        "extern int required(void);\n\
         extern int optional_hook(void) __attribute__((weak));\n\
         extern int optional_data __attribute__((weak));\n\
         int caller(void) { return required() + (optional_hook ? optional_hook() : 0)\n\
           + (&optional_data ? optional_data : 0); }\n",
        "c",
        &[],
    );
    let info = read(&bytes).unwrap();
    assert!(info.requirements.contains("required"));
    assert!(!info.requirements.contains("optional_hook"));
    assert!(!info.requirements.contains("optional_data"));
    assert_eq!(info.definitions["caller"].kind, NativeSymbolKind::Function);
}
