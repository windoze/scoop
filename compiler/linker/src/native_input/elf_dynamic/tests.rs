use super::*;
use scoop_process::CommandExt;
use scoop_toolchain::ResolvedTargetProfile;

#[test]
fn real_elf_dso_versions_tls_and_malformed_version_tables() {
    let target = ResolvedTargetProfile::resolve("x86_64-unknown-linux-gnu").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m28-linux/native-dso");
    let library = path.join("libversions.so");
    let result = target
        .c_bridge_toolchain()
        .driver_command()
        .args(["-shared", "-fPIC", "-Wl,-soname,libversions.so.1"])
        .arg(format!(
            "-Wl,--version-script={}",
            fixture.join("versions.map").display()
        ))
        .arg(fixture.join("versions.c"))
        .arg("-o")
        .arg(&library)
        .scoop_output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let original = std::fs::read(library).unwrap();
    let interface = ElfDynamic::read(&original, target.id()).unwrap();
    assert_eq!(interface.soname.as_deref(), Some("libversions.so.1"));
    assert_eq!(
        interface
            .symbols
            .find("m28_value", None)
            .unwrap()
            .version
            .as_deref(),
        Some("M28_2")
    );
    assert!(
        !interface
            .symbols
            .find("m28_value", Some("M28_1"))
            .unwrap()
            .default
    );
    assert_eq!(
        interface
            .symbols
            .find("m28_tls", None)
            .unwrap()
            .definition
            .kind,
        NativeSymbolKind::ThreadLocal
    );
    assert!(
        interface
            .symbols
            .imports
            .iter()
            .any(|import| import.name == "m28_leaf")
    );
    assert!(interface.symbols.find("m28_old", None).is_none());

    let mut bytes = original.clone();
    bytes[18..20].copy_from_slice(&elf::EM_AARCH64.to_le_bytes());
    assert!(
        ElfDynamic::read(&bytes, target.id())
            .err()
            .unwrap()
            .to_string()
            .contains("ELF64 amd64")
    );

    let file: ElfFile64<'_> = ElfFile64::parse(original.as_slice()).unwrap();
    let section = file.section_by_name(".gnu.version").unwrap();
    let header = section.elf_section_header() as *const _ as usize - original.as_ptr() as usize;
    let mut bytes = original.clone();
    bytes[header + 32..header + 40].copy_from_slice(&(section.size() - 2).to_le_bytes());
    assert!(
        ElfDynamic::read(&bytes, target.id())
            .err()
            .unwrap()
            .to_string()
            .contains("versions do not match")
    );

    let source = path.join("main.c");
    std::fs::write(&source, "int main(void) { return 0; }\n").unwrap();
    let executable = path.join("main");
    let result = target
        .c_bridge_toolchain()
        .driver_command()
        .arg("-pie")
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .scoop_output()
        .unwrap();
    assert!(result.status.success());
    assert!(
        ElfDynamic::read(&std::fs::read(executable).unwrap(), target.id())
            .err()
            .unwrap()
            .to_string()
            .contains("PIE executable")
    );
}
