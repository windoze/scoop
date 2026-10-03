use super::*;

#[test]
fn rebuilt_core_protocol_selects_its_real_string_descriptor_beside_an_ordinary_string() {
    let original = environment();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("core-source");
    copy_tree(&workspace().join("sysroot/lib/scoop.core"), &source);
    relocate_string(&source);
    std::fs::write(
        source.join("src/rebuilt-native.scoop"),
        fixture("rebuilt-core-native.scoop"),
    )
    .unwrap();
    let core = directory.path().join("rebuilt.slib");
    checked(
        Command::new(&original.compiler)
            .arg("build")
            .arg(&source)
            .arg("--out-slib")
            .arg(&core),
    );
    std::fs::remove_dir_all(source).unwrap();
    let environment = Environment {
        _directory: directory,
        core,
        compiler: original.compiler.clone(),
        linker: original.linker.clone(),
        runtime_index: original.runtime_index.clone(),
    };
    let path = environment._directory.path();
    let library = environment.build(
        path,
        "ordinary-string",
        "library",
        &fixture("shadow-string-library.scoop"),
        &[],
    );
    let root = environment.build(
        path,
        "root",
        "executable",
        &fixture("rebuilt-core-root.scoop"),
        &[("ordinary-string", &library)],
    );
    std::fs::remove_dir_all(path.join("sources")).unwrap();
    let program = path.join("program");
    let plan = environment.link(&root, &[&library], &program);
    let baseline = fixture("basic.plan");
    let alias = |text: &str| {
        text.lines()
            .find(|line| line.starts_with("alias "))
            .unwrap()
            .to_owned()
    };
    assert_ne!(
        alias(&plan),
        alias(&baseline),
        "the relocated String has a new typed TD owner"
    );
    for stress in [false, true] {
        assert_eq!(run(&program, stress), "rebuilt\nroot\n42\n42\n");
    }
}

fn relocate_string(source: &Path) {
    let path = source.join("src/types.scoop");
    let mut types = std::fs::read_to_string(&path).unwrap();
    let start = types.find("@Intrinsic(\"core_string\")").unwrap();
    let end = start + types[start..].find("@Intrinsic(\"core_array\")").unwrap();
    let declaration = types[start..end].replace(
        "public class String : ToString, Hash {",
        "public class String : ToString, Hash {\n    public fun rebuiltAnswer(): Int = 42\n",
    );
    types.replace_range(start..end, "public typealias String = relocated.String\n\n");
    std::fs::write(path, types).unwrap();
    let imports = "package relocated\n\nimport ToString\nimport Hash\nimport Long\nimport Boolean\nimport coreStringEquals\nimport coreStringHash\n\n";
    std::fs::write(
        source.join("src/relocated-string.scoop"),
        format!("{imports}{declaration}"),
    )
    .unwrap();
}
