use super::*;
use std::ffi::OsString;
use std::process::Command;

#[test]
fn compile_env_host_values_and_cache_updates() {
    const NAME: &str = "SCOOP_COMPILE_ENV_TEST_VALUE";
    const CASE: &str = "SCOOP_COMPILE_ENV_TEST_CASE";
    if let Ok(case) = std::env::var(CASE) {
        let inputs = CompileTimeInputs::default();
        let name = CompileTimeEnvironmentName::parse(NAME).unwrap();
        let value = inputs.environment(&name);
        if case == "invalid" {
            assert!(value.unwrap_err().contains("not valid Unicode"));
            assert!(!compiler_inputs_current(&[
                scoop_protocol::CompilerInputV1::Environment(scoop_protocol::EnvironmentInputV1 {
                    name: NAME.into(),
                    value: None,
                })
            ]));
            return;
        }
        let expected = match case.as_str() {
            "absent" => None,
            "empty" => Some(String::new()),
            "present" => Some("value".into()),
            "unicode" => Some("编译".into()),
            _ => panic!("unexpected child case"),
        };
        assert_eq!(value.unwrap(), expected);
        let recorded = inputs.compiler_inputs();
        assert_eq!(recorded.len(), 1);
        assert!(
            matches!(&recorded[0], scoop_protocol::CompilerInputV1::Environment(input) if input.value == expected)
        );
        assert!(compiler_inputs_current(&recorded));
        let stale = scoop_protocol::EnvironmentInputV1 {
            name: NAME.into(),
            value: if expected.is_some() {
                None
            } else {
                Some(String::new())
            },
        };
        assert!(!compiler_inputs_current(&[
            scoop_protocol::CompilerInputV1::Environment(stale)
        ]));
        return;
    }
    let mut cases = vec![
        ("absent", None),
        ("empty", Some(OsString::from(""))),
        ("present", Some(OsString::from("value"))),
        ("unicode", Some(OsString::from("编译"))),
    ];
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        cases.push(("invalid", Some(OsString::from_vec(vec![0xff]))));
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        cases.push(("invalid", Some(OsString::from_wide(&[0xd800]))));
    }
    for (case, value) in cases {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("compiler_inputs::tests::compile_env_host_values_and_cache_updates")
            .env(CASE, case)
            .env_remove(NAME);
        if let Some(value) = value {
            command.env(NAME, value);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{case}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
