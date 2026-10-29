use super::*;
use object::{Object, ObjectSection};

#[test]
fn native_debug_file_retains_lines_after_strip_and_uses_output_basename() {
    let target = scoop_toolchain::select_target(scoop_toolchain::host_target_triple().unwrap())
        .unwrap()
        .target();
    let tools = NativeDebugTools::resolve(target).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.c");
    std::fs::write(
        &source,
        "int increment(int x) { return x + 1; }\nint main(void) { return increment(41) != 42; }\n",
    )
    .unwrap();
    let compiled = directory.path().join("source.o");
    run(Command::new("cc")
        .args(["-g", "-gdwarf-5", "-O0", "-fno-omit-frame-pointer", "-c"])
        .arg(&source)
        .arg("-o")
        .arg(&compiled))
    .unwrap();
    for keep_symbols in [false, true] {
        let name = if keep_symbols {
            "kept program"
        } else {
            "stripped program"
        };
        let executable = directory.path().join(name);
        run(Command::new("cc").arg(&compiled).arg("-o").arg(&executable)).unwrap();
        let debug_file = generate(&tools, &executable, keep_symbols).unwrap();
        run(&mut Command::new(&executable)).unwrap();
        let object = match debug_file.kind {
            DebugFileKind::Dsym => {
                run(Command::new("/usr/bin/codesign")
                    .arg("--verify")
                    .arg(&executable))
                .unwrap();
                debug_file.path.join("Contents/Resources/DWARF").join(name)
            }
            DebugFileKind::Elf => {
                let bytes = std::fs::read(&executable).unwrap();
                let object = object::File::parse(bytes.as_slice()).unwrap();
                let link = object
                    .section_by_name(".gnu_debuglink")
                    .unwrap()
                    .data()
                    .unwrap();
                assert!(link.starts_with(format!("{name}.debug\0").as_bytes()));
                assert!(object.section_by_name(".eh_frame").is_some());
                debug_file.path.clone()
            }
        };
        let bytes = std::fs::read(&object).unwrap();
        let object = object::File::parse(bytes.as_slice()).unwrap();
        assert!(object.sections().any(|section| {
            section
                .name()
                .is_ok_and(|name| name == ".debug_line" || name == "__debug_line")
        }));
    }
}
