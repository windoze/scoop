use super::*;
use object::{Object, ObjectSection};

fn compiler(variable: &str) -> PathBuf {
    std::env::var_os(variable).map(PathBuf::from).unwrap()
}

fn run(command: &mut std::process::Command) {
    let output = command.scoop_output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn windows_native_drivers_compile_c_cpp_and_consume_real_cpp_archives() {
    let directory = tempfile::Builder::new()
        .prefix("scoop native 源码 ")
        .tempdir()
        .unwrap();
    let clang = compiler("SCOOP_TEST_CLANG");
    let cl = find_msvc_tools::find_tool("x86_64-pc-windows-msvc", "cl.exe").unwrap();
    let msvcrt = compiler("SCOOP_TEST_MINGW_MSVCRT");
    let ucrt = compiler("SCOOP_TEST_MINGW_UCRT");
    for (index, (target, prefix, selected)) in [
        (scoop_target_data::WINDOWS_X86_64_MSVC_UCRT, None, None),
        (
            scoop_target_data::WINDOWS_X86_64_MSVC_UCRT,
            None,
            Some(Path::new("cl")),
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT,
            msvcrt.parent().and_then(Path::parent),
            None,
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_UCRT,
            ucrt.parent().and_then(Path::parent),
            None,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let c = crate::resolve_windows_c_toolchain(target, &clang, prefix).unwrap();
        let root = directory.path().join(index.to_string());
        std::fs::create_dir_all(root.join("include")).unwrap();
        let macro_flag = if index < 2 { "/D" } else { "-D" };
        std::fs::write(
            root.join("Cone.toml"),
            format!(
                r#"schema = 1
[cone]
group = "test"
name = "native"
version = "1.0.0"
kind = "library"
[native]
cxx = true
include = ["include"]
c_flags = ["{macro_flag}", "EXTRA=5"]
[[native.sources]]
path = "answer.c"
[[native.sources]]
path = "wrapper.cpp"
"#
            ),
        )
        .unwrap();
        let header = root.join("include/值.h");
        std::fs::write(&header, "#define VALUE 37\n").unwrap();
        std::fs::write(root.join("answer.c"), "#include <scoop_rt.h>\n#include <值.h>\n_Static_assert(sizeof(long) == 4, \"LLP64\");\nint native_c_answer(void) { return VALUE + EXTRA; }\n").unwrap();
        std::fs::write(root.join("wrapper.cpp"), WRAPPER).unwrap();
        let manifest =
            crate::load_cone_manifest(&ManifestRootLocator::cone_directory(&root)).unwrap();
        let native = NativeToolchain::resolve_with(&c, &manifest, selected, None).unwrap();
        let public = crate::development_runtime_root().join("include");
        let prepared = native
            .prepare(&manifest, OptimizationMode::Debug, &public)
            .unwrap();
        assert_eq!(prepared.units().len(), 2);
        assert!(
            prepared
                .observed_paths
                .iter()
                .any(|p| p.canonicalize().ok() == header.canonicalize().ok())
        );
        let mut objects = Vec::new();
        for (number, unit) in prepared.units().iter().enumerate() {
            let object = root.join(format!("{number}.obj"));
            compile_native_source(
                unit,
                manifest.parsed().semantic().native(),
                &native,
                OptimizationMode::Debug,
                &object,
            )
            .unwrap();
            let bytes = std::fs::read(&object).unwrap();
            let file = object::File::parse(bytes.as_slice()).unwrap();
            assert!(
                file.sections()
                    .any(|section| section.name() == Ok(".debug$S"))
            );
            objects.push(object);
        }
        let provider = root.join("provider.cpp");
        let provider_object = root.join("provider.obj");
        std::fs::write(&provider, PROVIDER).unwrap();
        let mut provider_command = if index < 2 {
            crate::windows_native::command(
                &c,
                cl.path(),
                crate::native_driver::NativeDriverKind::Msvc,
                NativeSourceLanguage::Cxx,
            )
        } else {
            native.command(NativeSourceLanguage::Cxx).unwrap()
        };
        if index < 2 {
            provider_command
                .args(["/std:c++20", "/c"])
                .arg(&provider)
                .arg(crate::windows_native::joined("/Fo", &provider_object));
        } else {
            provider_command
                .args(["-std=c++20", "-c"])
                .arg(&provider)
                .arg("-o")
                .arg(&provider_object);
        }
        run(&mut provider_command);
        let archive = root.join("provider.lib");
        run(
            std::process::Command::new(clang.with_file_name("llvm-ar.exe"))
                .arg("rcs")
                .arg(&archive)
                .arg(&provider_object),
        );
        let main = root.join("main.c");
        let executable = root.join("native.exe");
        std::fs::write(
            &main,
            "#ifdef __cplusplus\nextern \"C\"\n#endif\nint native_wrapper(void); int main(void) { return native_wrapper() != 55; }\n",
        )
        .unwrap();
        let mut link = if index < 2 {
            native.command(NativeSourceLanguage::Cxx).unwrap()
        } else {
            std::process::Command::new(
                if index == 2 { &msvcrt } else { &ucrt }.with_file_name("clang++.exe"),
            )
        };
        link.current_dir(&root)
            .arg(&main)
            .args(&objects)
            .arg(&archive);
        if index < 2 {
            if index == 0 {
                link.arg("-fuse-ld=lld");
            }
            link.arg(crate::windows_native::joined("/Fe", &executable))
                .arg("/link");
            for path in &c.windows_environment().unwrap().libraries {
                link.arg(crate::windows_native::joined("/libpath:", path));
            }
        } else {
            link.args(["-static", "-fuse-ld=lld", "-o"])
                .arg(&executable);
        }
        run(&mut link);
        run(&mut std::process::Command::new(&executable));
        std::fs::write(&header, "#define VALUE 39\n").unwrap();
        let updated = native
            .prepare(&manifest, OptimizationMode::Debug, &public)
            .unwrap();
        assert_ne!(prepared.units()[0].digest(), updated.units()[0].digest());
        assert_eq!(prepared.units()[1].digest(), updated.units()[1].digest());
    }
}

const WRAPPER: &str = r#"
#include <string>
#include <stdexcept>
#include <scoop_rt.h>
std::string provider_message(int);
extern "C" int native_c_answer(void);
struct Base { virtual ~Base() = default; };
struct Derived : Base {};
extern "C" int native_wrapper(void) {
    Derived derived;
    Base *base = &derived;
    int result = native_c_answer() + int(provider_message(1).size());
    result += dynamic_cast<Derived *>(base) != nullptr;
    try { provider_message(0); }
    catch (const std::exception &error) { result += int(std::string(error.what()).size()); }
    return result;
}
"#;
const PROVIDER: &str = r#"
#include <string>
#include <stdexcept>
std::string provider_message(int value) {
    if (!value) throw std::runtime_error("caught");
    return "native";
}
"#;
