use object::{Object, ObjectSymbol};
use std::path::PathBuf;

use super::*;

#[test]
fn windows_generated_c_bridges_have_real_boundaries_and_execute() {
    let directory = tempfile::Builder::new()
        .prefix("scoop C桥接 ")
        .tempdir()
        .unwrap();
    let clang = PathBuf::from(std::env::var_os("SCOOP_TEST_CLANG").unwrap());
    for (target, prefix_variable) in [
        (scoop_target_data::WINDOWS_X86_64_MSVC_UCRT, None),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT,
            Some("SCOOP_TEST_MINGW_MSVCRT"),
        ),
        (
            scoop_target_data::WINDOWS_X86_64_MINGW_UCRT,
            Some("SCOOP_TEST_MINGW_UCRT"),
        ),
    ] {
        let prefix = prefix_variable.map(|variable| {
            let compiler = PathBuf::from(std::env::var_os(variable).unwrap());
            compiler.parent().unwrap().parent().unwrap().to_owned()
        });
        let invocation =
            scoop_toolchain::resolve_windows_c_toolchain(target, &clang, prefix.as_deref())
                .unwrap();
        let mut module = bridge_module_for_target(2, target);
        module.output = scoop_lir::LirOutput::Library;
        let input = scoop_lir::ConeLirOutput::try_new(module, Vec::new()).unwrap();
        let surface =
            scoop_lir::ObjectSymbolSurfaceV1::from_foundation(input.foundation()).unwrap();
        let emitted = emit_c_bridge_object_set(
            &input,
            directory.path(),
            invocation.profile(),
            |source, object| {
                invocation
                    .compile_object(source, object)
                    .map_err(|error| CodegenError(error.to_string()))
            },
        )
        .unwrap();
        let mut symbols = Vec::new();
        for member in emitted.members() {
            let plan = surface
                .plans()
                .iter()
                .find(|plan| {
                    plan.owner().kind()
                        == scoop_lir::StrongDefinitionEntityKind::GeneratedBridgeAtom(
                            member.plan().primary_atom_authority().id(),
                        )
                })
                .unwrap();
            let bytes = std::fs::read(member.object_path()).unwrap();
            scoop_slib::read_generated_c_object_sections_v1(&bytes, target.id()).unwrap();
            let file = object::File::parse(bytes.as_slice()).unwrap();
            for boundary in plan.atom_boundaries() {
                let start = file
                    .symbol_by_name(boundary.start().symbol().as_str())
                    .unwrap();
                let end = file
                    .symbol_by_name(boundary.end().symbol().as_str())
                    .unwrap();
                assert_eq!(start.section_index(), end.section_index());
                assert!(end.address() > start.address());
            }
            symbols.push(plan.primary_symbol().symbol().to_string());
        }
        assert_eq!(symbols.len(), 2);
        let source = directory.path().join("bridge-user.c");
        std::fs::write(
            &source,
            format!(
                r#"
extern void first(const int *) __asm__("{}");
extern void second(const int *) __asm__("{}");
static int result;
void test_bridge_1(int value) {{ result += value; }}
void test_bridge_2(int value) {{ result += value * 2; }}
int main(void) {{ int value = 7; first(&value); second(&value); return result != 21; }}
"#,
                symbols[0], symbols[1]
            ),
        )
        .unwrap();
        let executable = directory.path().join("bridge-user.exe");
        let profile = ValidatedBackendProfile::from_selection(
            scoop_target_data::selection(target.id()).unwrap(),
            invocation.profile().contract(),
        )
        .unwrap();
        let output =
            crate::tests::windows_codegen::native_command(target.id(), profile.llvm_triple())
                .arg(&source)
                .args(emitted.members().iter().map(|member| member.object_path()))
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            std::process::Command::new(executable)
                .status()
                .unwrap()
                .success()
        );
    }
}
