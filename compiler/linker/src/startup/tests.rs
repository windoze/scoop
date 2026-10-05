use super::*;
use object::{Object, ObjectSection, ObjectSymbol};
use scoop_toolchain::{FinalLinkOptions, LinkMode, ResolvedTargetProfile};
use std::process::Command;

#[test]
fn elf_startup_passes_the_exact_image_order_and_root_to_the_runtime_entry() {
    for (triple, mode) in [
        ("x86_64-unknown-linux-gnu", LinkMode::Dynamic),
        ("x86_64-unknown-linux-musl", LinkMode::Static),
        ("x86_64-unknown-linux-musl", LinkMode::Dynamic),
    ] {
        let target = ResolvedTargetProfile::resolve(triple).unwrap();
        let profile = target
            .final_link_with(&FinalLinkOptions {
                mode: Some(mode),
                ..Default::default()
            })
            .unwrap();
        let ValidatedFinalLinkProfile::Linux(linux) = &profile else {
            panic!("expected ELF profile")
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        let startup = StartupObject::build_for_symbols(
            &["scoop$test$image$a".into(), "scoop$test$image$b".into()],
            "scoop$test$root",
            &profile,
            path,
        )
        .unwrap();
        let file: object::read::elf::ElfFile64<'_> =
            object::read::elf::ElfFile64::parse(startup.bytes.as_slice()).unwrap();
        let array = file
            .symbols()
            .find(|symbol| symbol.name().ok() == Some("scoop_program_images"))
            .unwrap();
        assert_eq!(array.size(), 16);
        assert_eq!(
            file.section_by_index(array.section_index().unwrap())
                .unwrap()
                .name()
                .unwrap(),
            ".data.rel.ro.scoop.startup"
        );
        let startup_path = path.join("startup.o");
        std::fs::write(&startup_path, &startup.bytes).unwrap();
        let source = path.join("runtime-entry.c");
        std::fs::write(&source, SOURCE).unwrap();
        let object = path.join("runtime-entry.o");
        checked(
            profile
                .startup_toolchain()
                .object_compilation_command(&source, &object)
                .arg("-funwind-tables"),
        );
        let program = path.join("program");
        let mut command = linux
            .command(path, &program, &path.join("program.map"))
            .unwrap();
        command.args([startup_path, object]);
        linux.append_system_libraries(&mut command);
        checked(&mut command);
        linux
            .check_image(&std::fs::read(&program).unwrap())
            .unwrap();
        checked(&mut Command::new(&program));
    }
}

fn checked(command: &mut Command) {
    let result = command.scoop_output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}: {}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

const SOURCE: &str = r#"
#include <stdint.h>
const uint64_t image_a __asm__("scoop$test$image$a") = 11;
const uint64_t image_b __asm__("scoop$test$image$b") = 22;
const uint64_t root_entry __asm__("scoop$test$root") = 33;
int scoop_rt_run_program(const void *const *images, uint64_t count, const void *root) {
    return count != 2 || images[0] != &image_a || images[1] != &image_b || root != &root_entry;
}
"#;
