use super::*;

#[test]
fn linux_cxx_drivers_link_in_the_gnu_environment_independently_of_c_family() {
    for c_driver in ["gcc", "clang"] {
        let c = crate::resolve_linux_c_toolchain(
            scoop_target_data::LINUX_X86_64_GNU,
            Some(Path::new(c_driver)),
            None,
        )
        .unwrap();
        for cxx_driver in ["g++", "clang++"] {
            let cxx = ValidatedCxxToolchain::resolve_with(&c, Some(Path::new(cxx_driver))).unwrap();
            let link = crate::ValidatedFinalLinkProfile::from_tools(
                c.clone(),
                &crate::FinalLinkOptions::default(),
                Some(cxx.clone()),
            )
            .unwrap();
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("main.cpp");
            let object = directory.path().join("main.o");
            let executable = directory.path().join("program");
            std::fs::write(&source, "#include <stdexcept>\n#include <string>\nint main() { try { throw std::runtime_error(std::string(\"cxx passed\")); } catch (const std::exception &e) { return std::string(e.what()) != \"cxx passed\"; } }\n").unwrap();
            run(cxx
                .command(&c)
                .args(["-O2", "-c"])
                .arg(source)
                .arg("-o")
                .arg(&object))
            .unwrap();
            run(link
                .command(directory.path(), &executable)
                .unwrap()
                .arg(object))
            .unwrap();
            run(&mut Command::new(executable)).unwrap();
        }
    }
}
