use super::*;
use crate::test_catalog::parse_cone_manifest;
use crate::{ManifestParseErrorKind, SourceSelection};

const CONE: &str = "schema = 1\n[cone]\ngroup = 'native.test'\nname = 'inputs'\nversion = '1.0.0'\nkind = 'library'\n";

#[test]
fn native_sources_do_not_change_the_default_scoop_source_set() {
    let source = format!(
        "{CONE}\n[native]\ninclude = ['native/include']\nc_flags = ['-std=c17', '-D', 'ANSWER=42', '-I./more']\n[[native.sources]]\npath = 'native/file.c'\nwhen = {{os = 'linux'}}"
    );
    let parsed = parse_cone_manifest(&source).unwrap();
    assert_eq!(parsed.semantic().sources(), &SourceSelection::Default);
    let native = parsed.semantic().native();
    assert_eq!(
        native
            .include_paths(scoop_target_data::DARWIN_AARCH64_ID)
            .next()
            .unwrap()
            .to_str()
            .unwrap(),
        "native/include"
    );
    assert_eq!(native.sources()[0].path().as_str(), "native/file.c");
    assert!(
        native.sources()[0]
            .predicate()
            .matches(scoop_target_data::LINUX_X86_64_GNU_ID)
    );
    assert!(
        !native.sources()[0]
            .predicate()
            .matches(scoop_target_data::DARWIN_AARCH64_ID)
    );
    assert_eq!(
        native.c_flags(),
        &[
            NativeCompileFlag::Argument("-std=c17".into()),
            NativeCompileFlag::Argument("-DANSWER=42".into()),
            NativeCompileFlag::Include {
                kind: NativeIncludeFlag::Search,
                path: PathBuf::from("./more")
            },
        ]
    );
}

#[test]
fn include_options_keep_their_kind_order_and_original_paths() {
    let source = format!(
        "{CONE}\n[native]\nc_flags = ['-iquote', './quoted', '-isystemsystem', '-idirafter', 'late', '-include', 'prefix.h', '-imacrosmacros.h']"
    );
    let parsed = parse_cone_manifest(&source).unwrap();
    let flags = parsed.semantic().native().c_flags();
    for (flag, kind, path) in [
        (&flags[0], NativeIncludeFlag::Quote, "./quoted"),
        (&flags[1], NativeIncludeFlag::System, "system"),
        (&flags[2], NativeIncludeFlag::After, "late"),
        (&flags[3], NativeIncludeFlag::ForcedHeader, "prefix.h"),
        (&flags[4], NativeIncludeFlag::Macros, "macros.h"),
    ] {
        assert_eq!(
            flag,
            &NativeCompileFlag::Include {
                kind,
                path: PathBuf::from(path)
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
        "[native]\ninclude = ['']",
        "[native]\nc_flags = ['-I', '']",
        "[native]\nc_flags = ['-include']",
        "[native]\nsources = ['file.c']",
        "[[native.sources]]\npath = '../*.c'\nwhen = {os = []}",
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
fn native_sources_can_select_shared_parent_inputs() {
    for (input, expected) in [
        (
            "../../vendor/./uv//src/poll.c",
            "../../vendor/uv/src/poll.c",
        ),
        ("../other/../shared.c", "../shared.c"),
        ("local/../../shared.c", "../shared.c"),
    ] {
        let parsed =
            parse_cone_manifest(&format!("{CONE}\n[[native.sources]]\npath = '{input}'")).unwrap();
        assert_eq!(
            parsed.semantic().native().sources()[0].path().as_str(),
            expected
        );
        let scoop = format!(
            "schema = 1\n[[sources]]\npath = '{input}'\n[cone]\ngroup = 'test'\nname = 'paths'\nversion = '1.0.0'\nkind = 'library'"
        );
        assert!(parse_cone_manifest(&scoop).unwrap_err().span().is_some());
    }
}

#[test]
fn library_requirements_select_the_target_and_deduplicate_complete_keys() {
    use scoop_identity::NativeLibraryKind;
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
    for target in crate::test_catalog::SOURCE_TARGETS
        .iter()
        .map(|target| target.id)
    {
        let libraries = manifest
            .semantic()
            .native()
            .library_requirements(target)
            .unwrap();
        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0].key().target_profile().id(), target);
        let (name, kind) = if target == scoop_target_data::DARWIN_AARCH64_ID {
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

#[test]
fn native_header_paths_follow_normal_filesystem_resolution() {
    let source = format!(
        "{CONE}\n[native]\ninclude = ['../shared', '/sdk/include', 'link/../headers']\nc_flags = ['-include', '../config.h', '-I/opt/include', '-iquote[headers]']"
    );
    let parsed = parse_cone_manifest(&source).unwrap();
    let native = parsed.semantic().native();
    assert_eq!(
        native
            .include_paths(scoop_target_data::DARWIN_AARCH64_ID)
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            PathBuf::from("../shared"),
            PathBuf::from("/sdk/include"),
            PathBuf::from("link/../headers")
        ]
    );
    for (flag, expected) in
        native
            .c_flags()
            .iter()
            .zip(["../config.h", "/opt/include", "[headers]"])
    {
        let NativeCompileFlag::Include { path, .. } = flag else {
            panic!("expected include path");
        };
        assert_eq!(path, &PathBuf::from(expected));
    }
    for source in [
        "[native]\ninclude = [\"\\u0000\"]",
        "[native]\nc_flags = ['-I', \"\\u0000\"]",
    ] {
        assert!(parse_cone_manifest(&format!("{CONE}\n{source}")).is_err());
    }
}
