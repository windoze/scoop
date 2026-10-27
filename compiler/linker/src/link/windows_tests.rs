use super::*;
use crate::namespace::NativeNamespace;
use crate::native_input::{NativeContent, NativeInputs};
use crate::program::image::SelectedImage;
use crate::{NativeObjectInfo, NativeSymbolBinding, NativeSymbolKind};
use object::{Object, ObjectSection};
use scoop_identity::{
    CanonicalNativeLibraryName, NativeLibraryGrouping, NativeLibraryKind, NativeLinkRequirementId,
    NativeLinkRequirementKey,
};
use scoop_toolchain::{FinalLinkOptions, WindowsFinalLinkProfile};
use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command, Output};

fn tool(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} is required for Windows linker validation"))
}

#[test]
fn windows_pe_imports_startup_unicode_and_pdb() {
    let directory = tempfile::Builder::new()
        .prefix("scoop COFF , 引数 ")
        .tempdir()
        .unwrap();
    let clang = tool("SCOOP_TEST_CLANG");
    let msvcrt = tool("SCOOP_TEST_MINGW_MSVCRT");
    let ucrt = tool("SCOOP_TEST_MINGW_UCRT");
    let cases = [
        (
            scoop_target_data::WINDOWS_X86_64_MSVC_UCRT,
            None,
            "SCOOP_TEST_UNWIND_MSVC",
            None,
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MSVC_UCRT,
            None,
            "SCOOP_TEST_UNWIND_MSVC",
            Some(tool("SCOOP_TEST_MSVC_LINK")),
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT,
            msvcrt.parent().and_then(Path::parent),
            "SCOOP_TEST_UNWIND_MSVCRT",
            None,
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_UCRT,
            ucrt.parent().and_then(Path::parent),
            "SCOOP_TEST_UNWIND_UCRT",
            None,
        ),
    ];
    for (index, (target, prefix, unwind, linker)) in cases.into_iter().enumerate() {
        let startup = scoop_toolchain::resolve_windows_c_toolchain(target, &clang, prefix).unwrap();
        let profile = ValidatedFinalLinkProfile::Windows(
            WindowsFinalLinkProfile::resolve(
                startup,
                &FinalLinkOptions {
                    sysroot: Some(directory.path().to_owned()),
                    unwind_prefix: Some(tool(unwind)),
                    linker,
                    ..Default::default()
                },
                None,
            )
            .unwrap(),
        );
        let scratch = directory.path().join(index.to_string());
        std::fs::create_dir_all(&scratch).unwrap();
        validate_objects(&profile, &scratch);
        validate_pe(&profile, &scratch);
    }
}

fn validate_objects(profile: &ValidatedFinalLinkProfile, scratch: &Path) {
    let source = scratch.join("ordinary.c");
    let object = scratch.join("ordinary.obj");
    std::fs::write(&source, include_str!("windows_tests/ordinary.c")).unwrap();
    checked(
        profile
            .startup_toolchain()
            .object_compilation_command(&source, &object)
            .args(["-fcommon", "-Wall", "-Wextra", "-Werror"]),
    );
    let info = NativeObjectInfo::read_with_toolchain(
        &std::fs::read(&object).unwrap(),
        profile.startup_toolchain().profile(),
    )
    .unwrap();
    assert_eq!(
        info.definitions["common"].binding,
        NativeSymbolBinding::Common
    );
    assert_eq!(
        info.definitions["merged"].binding,
        NativeSymbolBinding::Weak
    );
    assert_eq!(
        info.definitions["function"].kind,
        NativeSymbolKind::Function
    );
    assert_eq!(info.definitions["tls"].kind, NativeSymbolKind::ThreadLocal);
    assert!(info.definitions["readonly"].read_only);
    assert!(info.requirements.contains("missing"));
    let bitcode = scratch.join("lto.obj");
    checked(
        profile
            .startup_toolchain()
            .object_compilation_command(&source, &bitcode)
            .arg("-flto"),
    );
    assert!(
        NativeObjectInfo::read_with_toolchain(
            &std::fs::read(bitcode).unwrap(),
            profile.startup_toolchain().profile()
        )
        .unwrap_err()
        .0
        .contains("ordinary AMD64 COFF")
    );
}

fn validate_pe(profile: &ValidatedFinalLinkProfile, scratch: &Path) {
    let ValidatedFinalLinkProfile::Windows(windows) = profile else {
        unreachable!()
    };
    let provider_source = scratch.join("provider.c");
    let provider_object = scratch.join("provider.obj");
    std::fs::write(&provider_source, "__declspec(dllexport) int imported_data = 17;\n__declspec(dllexport) int imported_function(int value) { return value + imported_data; }\n").unwrap();
    checked(
        &mut profile
            .startup_toolchain()
            .object_compilation_command(&provider_source, &provider_object),
    );
    let dll = scratch.join("provider.dll");
    let library = scratch.join("provider.lib");
    let mut command = windows.command(scratch, &dll).unwrap();
    command.arg(&provider_object);
    if windows.microsoft() {
        windows.linker_arg(&mut command, "/dll");
        windows.linker_arg(&mut command, format!("/implib:{}", library.display()));
    } else {
        command.arg("-shared");
        windows.linker_arg(&mut command, format!("--out-implib={}", library.display()));
    }
    checked(&mut command);
    let static_source = scratch.join("static.c");
    let static_object = scratch.join("static.obj");
    std::fs::write(
        &static_source,
        "int static_function(int value) { return value + 3; }\n",
    )
    .unwrap();
    checked(
        &mut profile
            .startup_toolchain()
            .object_compilation_command(&static_source, &static_object),
    );
    checked(
        Command::new(tool("SCOOP_TEST_CLANG").with_file_name("llvm-ar.exe"))
            .arg("rcs")
            .arg(scratch.join("libprovider.a"))
            .arg(&static_object)
            .arg(scratch.join("ordinary.obj")),
    );
    let key = NativeLinkRequirementKey::for_target(
        profile.target().wire_id(),
        CanonicalNativeLibraryName::new("provider").unwrap(),
        NativeLibraryKind::Dynamic,
        NativeLibraryGrouping::Independent,
    );
    let id = NativeLinkRequirementId::from_key(&key).unwrap();
    let static_key = NativeLinkRequirementKey::for_target(
        profile.target().wire_id(),
        CanonicalNativeLibraryName::new("provider").unwrap(),
        NativeLibraryKind::StaticArchive,
        NativeLibraryGrouping::Independent,
    );
    let static_id = NativeLinkRequirementId::from_key(&static_key).unwrap();
    let mut native = NativeInputs::read(
        BTreeMap::from([
            (id, (key, vec!["PE import test".into()])),
            (static_id, (static_key, vec!["PE archive test".into()])),
        ]),
        &[scratch.to_owned()],
        profile,
    )
    .unwrap();
    let input = native.libraries[&id].inputs[0];
    let NativeContent::Archive(members) = &native.files[&input].content else {
        panic!("import archive expected")
    };
    assert!(members.iter().any(|member| member.import().is_some()));
    assert!(native.files[&input].objects().len() < members.len());
    let static_input = &native.files[&native.libraries[&static_id].inputs[0]];
    assert_eq!(static_input.objects().len(), 2);
    let member = static_input.candidate("static_function").unwrap().0;
    let (_, range) = native.object(member).unwrap();
    let extracted = scratch.join("extracted static.obj");
    std::fs::write(&extracted, &static_input.bytes[range]).unwrap();
    let mut namespace = NativeNamespace::read(&mut native, &[], profile).unwrap();
    for (symbol, kind) in [
        ("imported_function", NativeSymbolKind::Function),
        ("imported_data", NativeSymbolKind::Data),
        ("__imp_imported_function", NativeSymbolKind::Data),
        ("__imp_imported_data", NativeSymbolKind::Data),
    ] {
        let mut candidates = namespace
            .candidates(&native, symbol, Some(&[input]), false)
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let binding = candidates.pop().unwrap();
        assert_eq!(binding.definition().unwrap().kind, kind);
        namespace.bind(symbol.into(), binding).unwrap();
    }
    assert!(
        namespace
            .candidates(&native, "not_exported", Some(&[input]), false)
            .unwrap()
            .is_empty()
    );
    let images: Vec<_> = (0..2)
        .map(|index| SelectedImage {
            name: format!("scoop$test$image${index}"),
            coordinate: [
                b"test".to_vec(),
                format!("image{index}").into_bytes(),
                b"1".to_vec(),
            ],
            cone: [index + 1; 32],
            dependencies: Vec::new(),
            tables: Default::default(),
            traps: [
                (format!("scoop$test$bounds${index}"), b"bounds\0"),
                (format!("scoop$test$size${index}"), b"size\0"),
            ],
        })
        .collect();
    let inputs = ProgramInputs {
        objects: Vec::new(),
        definitions: BTreeMap::new(),
        requirements: BTreeSet::new(),
        requirement_origins: BTreeMap::new(),
        namespace,
        target: profile.target(),
        images: images.iter().map(|image| image.name.clone()).collect(),
        final_images: images,
        selected: Default::default(),
        root: "scoop$test$root".into(),
        string_target: "scoop$test$String".into(),
        native,
        windows_subsystem: scoop_identity::WindowsSubsystem::Console,
    };
    let startup = StartupObject::build(&inputs, profile, scratch).unwrap_or_else(|error| {
        let info = NativeObjectInfo::read_with_toolchain(
            &std::fs::read(scratch.join("startup-compiled.o")).unwrap(),
            profile.startup_toolchain().profile(),
        );
        panic!("{}: {error}; {info:?}", profile.canonical_triple());
    });
    let startup_path = scratch.join("generated startup.obj");
    std::fs::write(&startup_path, &startup.bytes).unwrap();
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime");
    let arguments = scratch.join("UTF-16 arguments.obj");
    compile_runtime(
        profile,
        &runtime.join("src/startup/windows_arguments.c"),
        &arguments,
        &runtime,
    );
    let mut runtime_objects = vec![arguments];
    for source in [
        "src/platform/image/pe.c",
        "src/image/ranges.c",
        "src/io.c",
        "src/process.c",
    ] {
        let source = runtime.join(source);
        let object = scratch
            .join(source.file_stem().unwrap())
            .with_extension("obj");
        compile_runtime(profile, &source, &object, &runtime);
        runtime_objects.push(object);
    }
    let boundary_source = scratch.join("boundary.c");
    let boundary_object = scratch.join("边界 , 'quote'.obj");
    std::fs::write(&boundary_source, include_str!("windows_tests/boundary.c")).unwrap();
    compile_runtime(profile, &boundary_source, &boundary_object, &runtime);
    let executable = scratch.join("程序.exe");
    let pdb = scratch.join("公布 程序.exe.pdb");
    let map = scratch.join("程序.exe.map");
    runtime_objects.extend([startup_path, boundary_object, extracted]);
    let traced = pe::link(
        windows,
        &inputs,
        scratch,
        &runtime_objects,
        &executable,
        Some(&pdb),
        Some(&map),
    )
    .unwrap();
    assert!(traced.iter().any(|path| {
        path.file_name()
            .is_some_and(|name| name == "ucrt.lib" || name == "libmsvcrt.a")
    }));
    let arguments = ["", "值🧪", "quoted \"value\"", "end\\"];
    let output = checked(Command::new(&executable).args(arguments));
    let binary_stdout = b"out:\n\r\x1a\0\xff";
    let binary_stderr = b"scoop: cannot decode UTF-16 startup argument 1\nscoop: cannot decode UTF-16 startup argument 1\nerr:\n\r\x1a\0\xff";
    assert_eq!(
        output.stdout,
        [binary_stdout.as_slice(), b"atexit\n"].concat()
    );
    assert_eq!(output.stderr, binary_stderr);
    for (argument, code, native_exit, trap) in [
        ("--high-exit", 0x8ace_1357, true, false),
        ("--runtime-exit", 0x8ace_1357, false, false),
        ("--runtime-trap", 1, false, true),
    ] {
        let output = Command::new(&executable)
            .args(arguments)
            .arg(argument)
            .scoop_output()
            .unwrap();
        assert_eq!(output.status.code().map(|code| code as u32), Some(code));
        assert_eq!(
            output.stdout,
            [
                binary_stdout.as_slice(),
                if native_exit { b"atexit\n" } else { b"" }
            ]
            .concat(),
        );
        assert_eq!(
            output.stderr,
            [
                binary_stderr.as_slice(),
                if trap { b"scoop: trap: test\n" } else { b"" }
            ]
            .concat(),
        );
    }
    let bytes = std::fs::read(executable).unwrap();
    let pe = object::File::parse(bytes.as_slice()).unwrap();
    assert_eq!(
        pe.pdb_info().unwrap().unwrap().path(),
        "公布 程序.exe.pdb".as_bytes()
    );
    assert!(
        std::fs::read(pdb)
            .unwrap()
            .starts_with(b"Microsoft C/C++ MSF 7.00")
    );
    assert!(map.metadata().unwrap().len() > 0);
    let maps = pe.section_by_name(".smap").unwrap();
    assert!(maps.data().unwrap().starts_with(b"SCOOPMAP"));
    assert!(
        matches!(maps.flags(), object::SectionFlags::Coff { characteristics } if characteristics & object::pe::IMAGE_SCN_MEM_WRITE == 0)
    );
    assert!(
        pe.imports()
            .unwrap()
            .iter()
            .any(|import| import.library() == b"provider.dll" && import.name() == b"imported_data")
    );
}

fn compile_runtime(
    profile: &ValidatedFinalLinkProfile,
    source: &Path,
    object: &Path,
    runtime: &Path,
) {
    checked(
        profile
            .startup_toolchain()
            .object_compilation_command(source, object)
            .args(["-Wall", "-Wextra", "-Werror", "-g", "-gcodeview"])
            .arg("-I")
            .arg(runtime.join("include"))
            .arg("-I")
            .arg(runtime.join("src")),
    );
}

fn checked(command: &mut Command) -> Output {
    let output = command.scoop_output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
