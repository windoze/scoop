use object::{Object, ObjectSection, ObjectSymbol};

use super::*;

fn tool(variable: &str) -> PathBuf {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{variable} must name the compiler for this native environment"))
}

#[test]
fn windows_c_discovery_uses_selected_headers_and_transports_real_commands() {
    let directory = tempfile::Builder::new()
        .prefix("scoop Windows 头文件 ")
        .tempdir()
        .unwrap();
    let clang = tool("SCOOP_TEST_CLANG");
    let msvcrt = tool("SCOOP_TEST_MINGW_MSVCRT")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let ucrt = tool("SCOOP_TEST_MINGW_UCRT")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    for (target, root) in [
        (scoop_target_data::WINDOWS_X86_64_MSVC_UCRT, None),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT,
            Some(msvcrt.as_path()),
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_UCRT,
            Some(ucrt.as_path()),
        ),
    ] {
        let (invocation, inputs) = crate::windows_c::probe(target, &clang, root).unwrap();
        assert!(inputs.iter().any(|path| {
            path.file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("windows.h"))
        }));
        assert!(
            inputs
                .iter()
                .any(|path| path.file_name().is_some_and(|name| name == "stdlib.h"))
        );
        let record = InvocationRecord::from_invocation(&invocation).unwrap();
        let record: InvocationRecord =
            serde_json::from_slice(&serde_json::to_vec(&record).unwrap()).unwrap();
        let restored = record.into_invocation(target).unwrap();
        assert_eq!(restored, invocation);
        let source = directory.path().join("输入 source.c");
        let object = directory.path().join("输出 object.obj");
        std::fs::write(&source, "#include <windows.h>\n#include <stdlib.h>\nunsigned long scoop_windows_probe_user(void) { return GetCurrentThreadId(); }\n").unwrap();
        let command = restored.object_compilation_command(&source, &object);
        assert!(
            !command
                .get_envs()
                .any(|(key, _)| key == "CC" || key == "CFLAGS" || key == "INCLUDE")
        );
        restored.compile_object(&source, &object).unwrap();
        let bytes = std::fs::read(object).unwrap();
        let file = object::File::parse(bytes.as_slice()).unwrap();
        assert_eq!(file.format(), object::BinaryFormat::Coff);
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 0);
        for section in [".pdata", ".xdata"] {
            assert!(file.section_by_name(section).unwrap().size() > 0);
        }
        assert!(
            file.symbols()
                .any(|symbol| symbol.is_undefined()
                    && symbol.name() == Ok("__imp_GetCurrentThreadId"))
        );
    }
    for (target, wrong_root) in [
        (scoop_target_data::WINDOWS_X86_64_MINGW_UCRT, &msvcrt),
        (scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT, &ucrt),
    ] {
        let error =
            crate::resolve_windows_c_toolchain(target, &clang, Some(wrong_root)).unwrap_err();
        assert!(
            error.to_string().contains("wrong Windows CRT headers"),
            "{error}"
        );
    }
}

#[test]
fn windows_c_explicit_microsoft_resources_compile_without_discovery_defaults() {
    let tool = find_msvc_tools::find_tool("x86_64-pc-windows-msvc", "cl.exe").unwrap();
    let includes = tool
        .env()
        .into_iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("INCLUDE"))
        .unwrap()
        .1
        .clone();
    let tools = tool.path().ancestors().nth(4).unwrap();
    let ucrt = std::env::split_paths(&includes)
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("ucrt"))
        })
        .unwrap();
    let sdk_version = ucrt
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let sdk = ucrt.ancestors().nth(3).unwrap();
    let directory = tempfile::Builder::new()
        .prefix("scoop SDK 显式 ")
        .tempdir()
        .unwrap();
    let tools = tools.to_str().unwrap();
    let sdk = sdk.to_str().unwrap();
    let config = toml::toml! { [msvc]
        vc_tools = tools
        windows_sdk = sdk
        sdk_version = sdk_version
    };
    std::fs::write(
        directory.path().join("msvc.toml"),
        toml::to_string(&config).unwrap(),
    )
    .unwrap();
    let invocation = crate::resolve_windows_c_toolchain(
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT,
        &super::windows::tool("SCOOP_TEST_CLANG"),
        Some(directory.path()),
    )
    .unwrap();
    assert_eq!(invocation.native_sysroot(), Some(directory.path()));
    assert!(
        invocation
            .windows_environment()
            .unwrap()
            .includes
            .iter()
            .any(|path| path
                .to_string_lossy()
                .eq_ignore_ascii_case(&ucrt.to_string_lossy()))
    );
}
