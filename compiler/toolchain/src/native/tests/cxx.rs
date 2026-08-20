use super::*;

#[test]
fn mixed_c_and_cxx_use_independent_flags_and_immutable_headers() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(
        root.join("Cone.toml"),
        r#"schema = 1
[cone]
group = "test"
name = "native-cxx"
version = "1.0.0"
kind = "library"
[native]
cxx = true
c_flags = ["-DC_ONLY=1", "-std=c11"]
cxx_flags = ["-DCXX_ONLY=1", "-std=c++20", "-Wall", "-Wextra", "-Werror"]
[[native.sources]]
path = "native.c"
[[native.sources]]
path = "native.cpp"
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("native.c"),
        r#"
#if !C_ONLY || defined(CXX_ONLY) || defined(__cplusplus)
#error incorrect C configuration
#endif
int from_c(void) { return _Generic(0, int: 1, default: 0); }
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("native.cpp"),
        r#"
#include <scoop_rt.h>
#include <stdexcept>
#include <vector>
#include "value.h"
#if !CXX_ONLY || defined(C_ONLY)
#error incorrect C++ configuration
#endif
static_assert(sizeof(ScoopObjectHeader) == 16);
extern "C" int from_c(void);
extern "C" int answer(void) {
    try { throw std::runtime_error("native"); }
    catch (const std::runtime_error&) { return std::vector<int>{VALUE}[0] + from_c(); }
}
"#,
    )
    .unwrap();
    std::fs::write(root.join("value.h"), "#define VALUE 41\n").unwrap();
    let manifest = load_cone_manifest(&ManifestRootLocator::cone_directory(root)).unwrap();
    let compiler = NativeToolchain::resolve(target().c_bridge_toolchain(), &manifest).unwrap();
    let prepared = compiler
        .prepare(
            &manifest,
            OptimizationMode::Debug,
            &crate::development_runtime_root().join("include"),
        )
        .unwrap();
    assert_eq!(prepared.units().len(), 2);
    std::fs::write(root.join("value.h"), "#define VALUE 98\n").unwrap();
    let mut objects = Vec::new();
    for (index, unit) in prepared.units().iter().enumerate() {
        let object = root.join(format!("{index}.o"));
        compile_native_source(
            unit,
            manifest.parsed().semantic().native(),
            &compiler,
            OptimizationMode::Debug,
            &object,
        )
        .unwrap();
        objects.push(object);
    }
    let main = root.join("main.c");
    let object = root.join("main.o");
    std::fs::write(
        &main,
        "int answer(void); int main(void) { return answer() != 42; }\n",
    )
    .unwrap();
    let result = target()
        .c_bridge_toolchain()
        .object_compilation_command(&main, &object)
        .scoop_output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    objects.push(object);
    let binary = root.join("check");
    let result = compiler
        .command(NativeSourceLanguage::Cxx)
        .unwrap()
        .args(objects)
        .arg("-o")
        .arg(&binary)
        .scoop_output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        std::process::Command::new(binary)
            .scoop_output()
            .unwrap()
            .status
            .success()
    );
    let changed = compiler
        .prepare(
            &manifest,
            OptimizationMode::Debug,
            &crate::development_runtime_root().join("include"),
        )
        .unwrap();
    assert_ne!(
        scoop_wire::encode(&prepared).unwrap(),
        scoop_wire::encode(&changed).unwrap()
    );
}
