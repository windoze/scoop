use std::process::Command;

#[test]
fn installed_binary_reports_the_exact_shared_capability() {
    let output = Command::new(env!("CARGO_BIN_EXE_scoopc"))
        .arg("__machine-capability")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        scoop_protocol::decode_capability_frame(&output.stdout).unwrap(),
        scoop_protocol::ScoopcProtocolCapabilityV1::current()
    );
}
