use super::*;
use scoop_identity::WindowsSubsystem;

#[test]
fn windows_subsystem_defaults_and_explicit_choices() {
    let source = MINIMAL.replace("\"library\"", "\"executable\"");
    for (suffix, expected) in [
        ("", WindowsSubsystem::Console),
        ("\n[windows]\n", WindowsSubsystem::Console),
        (
            "\n[windows]\nsubsystem = \"console\"\n",
            WindowsSubsystem::Console,
        ),
        ("\n[windows]\nsubsystem = \"gui\"\n", WindowsSubsystem::Gui),
    ] {
        let manifest = parse_cone_manifest(&format!("{source}{suffix}")).unwrap();
        assert_eq!(manifest.semantic().windows().subsystem(), expected);
    }
}

#[test]
fn windows_subsystem_reports_bad_values_and_fields() {
    let source = MINIMAL.replace("\"library\"", "\"executable\"");
    let bad = format!("{source}\n[windows]\nsubsystem = \"GUI\"\n");
    let error = parse_cone_manifest(&bad).unwrap_err();
    assert!(matches!(
        error.kind(),
        ManifestParseErrorKind::InvalidWindows(_)
    ));
    assert_eq!(&bad[error.span().unwrap().range()], "\"GUI\"");
    for field in ["subsystem = 1", "subsystem = false", "subsytem = \"gui\""] {
        assert!(parse_cone_manifest(&format!("{source}\n[windows]\n{field}\n")).is_err());
    }
}

#[test]
fn libraries_reject_windows_executable_configuration() {
    let error = parse_cone_manifest(&format!("{MINIMAL}\n[windows]\n")).unwrap_err();
    assert!(matches!(
        error.kind(),
        ManifestParseErrorKind::InvalidWindows(_)
    ));
    assert!(error.to_string().contains("kind = \"executable\""));
}
