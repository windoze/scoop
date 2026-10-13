use super::*;

#[test]
fn output_capture_only_applies_to_actual_execution() {
    for flag in ["--list", "--no-run"] {
        assert!(Cli::try_parse_from(["scoop", "test", flag, "--nocapture"]).is_err());
    }
    assert!(Cli::try_parse_from(["scoop", "test", "--nocapture", "--ignored"]).is_ok());
}
