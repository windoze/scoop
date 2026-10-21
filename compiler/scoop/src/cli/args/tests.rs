use super::*;

#[test]
fn output_capture_only_applies_to_actual_execution() {
    for flag in ["--list", "--no-run"] {
        assert!(Cli::try_parse_from(["scoop", "test", flag, "--nocapture"]).is_err());
    }
    assert!(Cli::try_parse_from(["scoop", "test", "--nocapture", "--ignored"]).is_ok());
}

#[test]
fn compiler_jobs_apply_to_build_run_and_test_and_must_be_positive() {
    for command in ["build", "run", "test"] {
        assert!(Cli::try_parse_from(["scoop", command, "-j", "2"]).is_ok());
        assert!(Cli::try_parse_from(["scoop", command, "--jobs", "1"]).is_ok());
        assert!(Cli::try_parse_from(["scoop", command, "--jobs", "0"]).is_err());
    }
}
