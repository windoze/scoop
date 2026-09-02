use super::*;

#[test]
fn emits_non_empty_object_file() {
    let module = values_module();
    let output = std::env::temp_dir().join(format!("scoop_codegen_test_{}.o", std::process::id()));
    emit_object(&module, &output, host_profile()).expect("emit object");
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}
