use super::*;
use crate::{ManifestParseErrorKind, SourceSelection, parse_cone_manifest};

const CONE: &str = "schema = 1\n[cone]\ngroup = 'native.test'\nname = 'inputs'\nversion = '1.0.0'\nkind = 'library'\n";

#[test]
fn native_sources_do_not_change_the_default_scoop_source_set() {
    let source = format!(
        "{CONE}\n[native]\ninclude = ['native/include']\nc_flags = ['-std=c17', '-D', 'ANSWER=42', '-I./more']\n[[native.sources]]\npath = 'native/file.c'\nwhen = {{os = 'linux'}}"
    );
    let parsed = parse_cone_manifest(&source).unwrap();
    assert_eq!(parsed.semantic().sources(), &SourceSelection::Default);
    let native = parsed.semantic().native();
    assert_eq!(native.include()[0].as_str(), "native/include");
    assert_eq!(native.sources()[0].path().as_str(), "native/file.c");
    assert!(
        native.sources()[0]
            .predicate()
            .matches(scoop_identity::TargetProfileId::LinuxX86_64Gnu)
    );
    assert!(
        !native.sources()[0]
            .predicate()
            .matches(scoop_identity::TargetProfileId::DarwinAarch64)
    );
    assert_eq!(
        native.c_flags(),
        &[
            NativeCompileFlag::Argument("-std=c17".into()),
            NativeCompileFlag::Argument("-DANSWER=42".into()),
            NativeCompileFlag::Include {
                kind: NativeIncludeFlag::Search,
                path: ConeRelativePath::new("more").unwrap()
            },
        ]
    );
}

#[test]
fn include_options_keep_their_kind_order_and_normalized_paths() {
    let source = format!(
        "{CONE}\n[native]\nc_flags = ['-iquote', './quoted', '-isystemsystem', '-idirafter', 'late', '-include', 'prefix.h', '-imacrosmacros.h']"
    );
    let parsed = parse_cone_manifest(&source).unwrap();
    let flags = parsed.semantic().native().c_flags();
    for (flag, kind, path) in [
        (&flags[0], NativeIncludeFlag::Quote, "quoted"),
        (&flags[1], NativeIncludeFlag::System, "system"),
        (&flags[2], NativeIncludeFlag::After, "late"),
        (&flags[3], NativeIncludeFlag::ForcedHeader, "prefix.h"),
        (&flags[4], NativeIncludeFlag::Macros, "macros.h"),
    ] {
        assert_eq!(
            flag,
            &NativeCompileFlag::Include {
                kind,
                path: ConeRelativePath::new(path).unwrap()
            }
        );
    }
}

#[test]
fn flags_cannot_replace_driver_owned_inputs_or_outputs() {
    for flag in [
        "-c",
        "-E",
        "-S",
        "-ooutput.o",
        "-x",
        "--target=x86_64-linux-gnu",
        "--sysroot=/sdk",
        "-arch",
        "-m32",
        "-MMD",
        "-MFdeps",
        "@options.rsp",
        "-fplugin=plugin.so",
        "--specs=other",
        "-Btools",
        "-Wl,-rpath,.",
        "-ltest",
        "-include-pch",
        "-fpreprocessed",
        "-fpch-preprocess",
    ] {
        let source = format!("{CONE}\n[native]\nc_flags = ['{flag}']");
        let error = parse_cone_manifest(&source).unwrap_err();
        assert!(
            matches!(error.kind(), ManifestParseErrorKind::InvalidNative(_)),
            "{flag}: {error}"
        );
        assert_eq!(&source[error.span().unwrap().range()], format!("'{flag}'"));
    }
}

#[test]
fn malformed_native_paths_and_tables_fail_before_source_selection() {
    for native in [
        "[native]\ninclude = ['/outside']",
        "[native]\nc_flags = ['-I', '../outside']",
        "[native]\nc_flags = ['-include']",
        "[native]\nsources = ['file.c']",
        "[[native.sources]]\npath = '../file.c'\nwhen = {os = []}",
        "[[native.sources]]\npath = 'file.c'\nwhen = {env = 'unknown'}",
        "[native]\nflags = []",
    ] {
        assert!(
            parse_cone_manifest(&format!("{CONE}\n{native}")).is_err(),
            "{native}"
        );
    }
}

#[test]
fn library_requirements_select_the_target_and_deduplicate_complete_keys() {
    use scoop_identity::{NativeLibraryKind, TargetProfileId};
    let source = format!(
        "{CONE}
[[native.libraries]]
name = 'm'
when = {{os = 'linux'}}
[[native.libraries]]
name = 'CoreFoundation'
kind = 'framework'
when = {{os = 'darwin'}}
[[native.libraries]]
name = 'm'
kind = 'default'
when = {{env = ['gnu', 'musl']}}
"
    );
    let manifest = parse_cone_manifest(&source).unwrap();
    for target in TargetProfileId::ALL {
        let libraries = manifest
            .semantic()
            .native()
            .library_requirements(target)
            .unwrap();
        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0].key().target_profile().id(), target);
        let (name, kind) = if target == TargetProfileId::DarwinAarch64 {
            ("CoreFoundation", NativeLibraryKind::Framework)
        } else {
            ("m", NativeLibraryKind::TargetDefault)
        };
        assert_eq!(libraries[0].key().library().as_str(), name);
        assert_eq!(libraries[0].key().kind(), kind);
    }
}

#[test]
fn library_syntax_is_checked_before_target_selection() {
    for declaration in [
        "name = '../m'",
        "name = ''",
        "name = 'm'\nkind = 'archive'",
        "name = 'm'\nkind = 2",
        "name = 'm'\ngroup = 'unused'",
        "kind = 'static'",
        "name = 'm'\nwhen = {os = 'unknown'}",
    ] {
        assert!(
            parse_cone_manifest(&format!("{CONE}\n[[native.libraries]]\n{declaration}")).is_err(),
            "{declaration}"
        );
    }
}
