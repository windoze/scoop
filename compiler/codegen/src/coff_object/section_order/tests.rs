use std::path::PathBuf;
use std::process::Command;

use scoop_process::CommandExt;

use super::*;

#[test]
fn associated_constants_follow_their_code_in_coff_and_bigobj_and_link() {
    let directory = tempfile::tempdir().unwrap();
    let clang = PathBuf::from(std::env::var_os("SCOOP_TEST_CLANG").unwrap());
    let startup = scoop_toolchain::resolve_windows_c_toolchain(
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT,
        &clang,
        None,
    )
    .unwrap();
    let profile = scoop_toolchain::WindowsFinalLinkProfile::resolve(
        startup,
        &scoop_toolchain::FinalLinkOptions {
            linker: Some(PathBuf::from(
                std::env::var_os("SCOOP_TEST_MSVC_LINK").unwrap(),
            )),
            unwind_prefix: Some(PathBuf::from(
                std::env::var_os("SCOOP_TEST_UNWIND_MSVC").unwrap(),
            )),
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let harness = directory.path().join("main.c");
    std::fs::write(
        &harness,
        "double add_value(double); int wmain(void) { return add_value(0.5) != 2.0; }\n",
    )
    .unwrap();
    for big in [false, true] {
        let source = directory.path().join(format!("constant-{big}.s"));
        let path = source.with_extension("obj");
        let mut assembly =
            String::from(".section .rdata,\"dr\"\n.p2align 3\nbias:\n.quad 0x3ff8000000000000\n");
        if big {
            // Cross the 16-bit section number boundary with real empty BSS
            // contributions; the same code and relocation use a bigobj table.
            for index in 0..0x10000 {
                assembly.push_str(&format!(".section .bss$pad{index},\"bw\"\n"));
            }
        }
        assembly.push_str(".section .text$add_value,\"xr\",discard,add_value\n.globl add_value\nadd_value:\nmovsd bias(%rip), %xmm1\naddsd %xmm1, %xmm0\nretq\n");
        std::fs::write(&source, assembly).unwrap();
        run(Command::new(&clang)
            .args(["--target=x86_64-pc-windows-msvc", "-c"])
            .arg(&source)
            .arg("-o")
            .arg(&path));
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            object::FileKind::parse(bytes.as_slice()).unwrap(),
            if big {
                object::FileKind::CoffBig
            } else {
                object::FileKind::Coff
            }
        );
        let mut object = CoffObject::read(&bytes).unwrap();
        let (constant, _) = object.extent("bias").unwrap();
        let (code, _) = object.extent("add_value").unwrap();
        assert!(constant.0 < code.0);
        if big {
            assert!(code.0 > 0xffff);
        }
        let symbol = object.file.symbol_by_name("add_value").unwrap().index();
        let code_section = object.file.section_by_index(code).unwrap();
        let original_data = code_section.data().unwrap().to_vec();
        let original_relocations = references(&code_section);
        object
            .associate(code, &[constant], LinkageClass::ConeStrong)
            .unwrap();
        let fixed = object.finish().unwrap();
        std::fs::write(&path, &fixed).unwrap();
        let object = CoffObject::read(&fixed).unwrap();
        let (code, _) = object.extent("add_value").unwrap();
        let (constant, _) = object.extent("bias").unwrap();
        assert!(code.0 < constant.0);
        assert_eq!(
            object.file.symbol_by_name("add_value").unwrap().index(),
            symbol
        );
        let section = object.file.section_by_index(code).unwrap();
        assert_eq!(section.data().unwrap(), original_data);
        assert_eq!(references(&section), original_relocations);
        let aux = object.section_aux[&constant.0];
        let low = u16::from_le_bytes(object.symbols[aux + 12..aux + 14].try_into().unwrap());
        let high = u16::from_le_bytes(object.symbols[aux + 16..aux + 18].try_into().unwrap());
        assert_eq!(usize::from(low) | (usize::from(high) << 16), code.0);
        let executable = directory.path().join(format!("constant-{big}.exe"));
        let mut command = profile.command(directory.path(), &executable).unwrap();
        run(command.arg(&harness).arg(&path));
        run(&mut Command::new(&executable));
    }
}

fn references(section: &object::Section<'_, '_>) -> Vec<(u64, object::RelocationTarget, i64)> {
    section
        .relocations()
        .map(|(offset, relocation)| (offset, relocation.target(), relocation.addend()))
        .collect()
}

fn run(command: &mut Command) {
    let output = command.scoop_output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{output:?}");
}
