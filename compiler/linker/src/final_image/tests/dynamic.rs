use super::*;
use crate::macho_cursor::Cursor;

#[test]
fn actual_native_dynamic_ordinal_version_and_rpath_corruption_are_rejected() {
    let fixture = support::native_fixture();
    let path = fixture.directory.path();
    let object = path.join("dynamic-build.o");
    std::fs::rename(path.join("m23_final.o"), &object).unwrap();
    let toolchain = fixture.profile.startup_toolchain();
    let result = std::process::Command::new(toolchain.compiler_driver())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .args(["-target", "arm64-apple-macos", "-dynamiclib", "-isysroot"])
        .arg(toolchain.sdk_root())
        .arg(format!(
            "-mmacosx-version-min={}",
            toolchain.profile().contract().deployment().minimum_os()
        ))
        .args([
            "-install_name",
            "@rpath/libm23_final.dylib",
            "-current_version",
            "2.0",
            "-compatibility_version",
            "1.0",
        ])
        .arg(&object)
        .arg("-o")
        .arg(path.join("libm23_final.dylib"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let linked = crate::link_program(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &fixture.library_paths,
        &path.join("dynamic-program"),
    )
    .unwrap();
    let result = std::process::Command::new(&linked.path)
        .env_clear()
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"42\n");
    let inputs = ProgramInputs::new(
        &fixture.closure,
        &fixture.runtime,
        &fixture.profile,
        &fixture.library_paths,
    )
    .unwrap();
    let startup = StartupObject::build(&inputs, &fixture.profile, path).unwrap();
    let original = std::fs::read(&linked.path).unwrap();
    let map = super::native::symbol_ranges(&inputs, &original);
    verify(&original, &inputs, &startup, &fixture.profile, &map).unwrap();
    let reject = |bytes: &[u8], expected: &str| {
        let message = verify(bytes, &inputs, &startup, &fixture.profile, &map)
            .err()
            .unwrap()
            .to_string();
        assert!(message.contains(expected), "{message}");
        assert_eq!(std::fs::read(&linked.path).unwrap(), original);
    };

    let mut bytes = original.clone();
    let rpath = inputs.providers.rpaths.iter().next().unwrap();
    let offset = find(&bytes, rpath.as_bytes());
    bytes[offset + rpath.len() - 1] = b'!';
    reject(&bytes, "final RPATH");

    let file: MachOFile64<'_> = MachOFile64::parse(original.as_slice()).unwrap();
    let mut commands = file.macho_load_commands().unwrap();
    let mut changed = false;
    while let Some(command) = commands.next().unwrap() {
        if let Some(library) = command.dylib().unwrap()
            && command.cmd() == macho::LC_LOAD_DYLIB
            && command.string(file.endian(), library.dylib.name).unwrap()
                == b"@rpath/libm23_final.dylib"
        {
            let offset = command.raw_data().as_ptr() as usize - original.as_ptr() as usize;
            let mut bytes = original.clone();
            bytes[offset + 20..offset + 24].copy_from_slice(&0u32.to_le_bytes());
            reject(&bytes, "version differs");
            changed = true;
        }
    }
    assert!(changed);
    let dyld = command(&original, macho::LC_DYLD_INFO_ONLY);
    let mut changed = false;
    for stream in [16, 24, 32] {
        let offset = word(&original, dyld + stream) as usize;
        let size = word(&original, dyld + stream + 4) as usize;
        if let Some(ordinal) = ordinal_for(&original[offset..offset + size], "_m23_final") {
            let mut bytes = original.clone();
            let old = bytes[offset + ordinal] & 15;
            assert!(matches!(old, 1 | 2));
            bytes[offset + ordinal] = 0x10 | if old == 1 { 2 } else { 1 };
            reject(&bytes, "ordinal");
            changed = true;
            break;
        }
    }
    assert!(changed);
}

fn ordinal_for(bytes: &[u8], target: &str) -> Option<usize> {
    let mut cursor = Cursor::new(bytes);
    let mut ordinal = None;
    let mut symbol = String::new();
    while !cursor.done() {
        let position = cursor.take(0).unwrap().as_ptr() as usize - bytes.as_ptr() as usize;
        let opcode = cursor.byte().unwrap() & 0xf0;
        if matches!(opcode, 0x90 | 0xa0 | 0xb0 | 0xc0) && symbol == target {
            return ordinal;
        }
        match opcode {
            0x00 => {
                ordinal = None;
                symbol.clear();
            }
            0x10 => ordinal = Some(position),
            0x20 => {
                cursor.uleb().unwrap();
                ordinal = None;
            }
            0x30 => ordinal = None,
            0x40 => symbol = cursor.name().unwrap(),
            0x50 | 0x90 | 0xb0 => continue,
            0x60 => {
                cursor.sleb().unwrap();
            }
            0x70 | 0x80 | 0xa0 => {
                cursor.uleb().unwrap();
            }
            0xc0 => {
                cursor.uleb().unwrap();
                cursor.uleb().unwrap();
            }
            _ => panic!("unexpected binding opcode {opcode:#x}"),
        }
    }
    None
}
