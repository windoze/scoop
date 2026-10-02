use super::*;

#[test]
fn unused_extern_contracts_conflict_before_library_lookup_and_report_all_cones() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    for (case, left, right, expected) in [
        ("library", "function", "library", "library binding differs"),
        ("abi", "function", "scoop", "C/Scoop ABI differs"),
        ("effect", "scoop", "nogc", "GC effect differs"),
        (
            "parameter",
            "function",
            "parameter",
            "C parameter ABI differs",
        ),
        ("result", "function", "result", "C result ABI differs"),
        (
            "function-data",
            "function",
            "val",
            "function/data/TLS/mutability or storage differs",
        ),
        (
            "tls",
            "var",
            "tls",
            "function/data/TLS/mutability or storage differs",
        ),
        (
            "mutability",
            "var",
            "val",
            "function/data/TLS/mutability or storage differs",
        ),
    ] {
        let case_directory = directory.path().join(case);
        let mut providers = Vec::new();
        for (side, fixture) in [("left", left), ("right", right), ("also-left", left)] {
            let source = format!(
                "package nativelink.contracts.{}\n\n{}",
                side.replace('-', ""),
                native_fixture(&format!("contracts/{fixture}.scoop"))
            );
            providers.push(environment.build(&case_directory, side, "library", &source, &[]));
            std::fs::remove_dir_all(case_directory.join("sources").join(side)).unwrap();
        }
        let root = environment.build(
            &case_directory,
            "contracts",
            "executable",
            &native_fixture("contracts/root.scoop"),
            &[
                ("left", &providers[0]),
                ("right", &providers[1]),
                ("also-left", &providers[2]),
            ],
        );
        std::fs::remove_dir_all(case_directory.join("sources")).unwrap();
        reject_native(
            environment,
            &root,
            &[&providers[0], &providers[1], &providers[2]],
            &case_directory,
            &[],
            &[
                "native contract conflict for _m23_contract",
                expected,
                "dev.programlink:left",
                "dev.programlink:right",
                "dev.programlink:also-left",
                "native declarations",
            ],
        );
    }
}
