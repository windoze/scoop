use super::*;
use scoop_process::CommandExt;
use scoop_toolchain::LinkMode;
use scoop_wire::sha256;

pub(super) fn find(
    key: &NativeLinkRequirementKey,
    profile: &ValidatedFinalLinkProfile,
) -> Result<LibraryResolution, LinkError> {
    match profile {
        ValidatedFinalLinkProfile::Darwin(_) => darwin(key, profile),
        ValidatedFinalLinkProfile::Linux(linux) => linux_library(key, profile, linux.mode()),
    }
}

fn darwin(
    key: &NativeLinkRequirementKey,
    profile: &ValidatedFinalLinkProfile,
) -> Result<LibraryResolution, LinkError> {
    let sdk = profile.startup_toolchain().sdk_root().map_err(error)?;
    let name = key.library().as_str();
    let library = sdk.join("usr/lib");
    let framework = sdk
        .join("System/Library/Frameworks")
        .join(format!("{name}.framework"));
    let dynamic = [
        (
            library.join(format!("lib{name}.tbd")),
            NativeFileKind::TextStub,
        ),
        (
            library.join(format!("lib{name}.dylib")),
            NativeFileKind::Dylib,
        ),
    ];
    let frameworks = [
        (
            framework.join(format!("{name}.tbd")),
            NativeFileKind::TextStub,
        ),
        (framework.join(name), NativeFileKind::Framework),
    ];
    let archive = (
        library.join(format!("lib{name}.a")),
        NativeFileKind::Archive,
    );
    let candidates: Vec<_> = match key.kind() {
        NativeLibraryKind::TargetDefault => dynamic
            .into_iter()
            .chain([archive])
            .chain(frameworks)
            .collect(),
        NativeLibraryKind::Dynamic => dynamic.into(),
        NativeLibraryKind::StaticArchive => vec![archive],
        NativeLibraryKind::Framework => frameworks.into(),
    };
    for (path, kind) in candidates {
        match std::fs::symlink_metadata(&path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(error(format!(
                    "system native candidate {}: {err}",
                    path.display()
                )));
            }
            Ok(_) => {}
        }
        return Ok(LibraryResolution {
            files: vec![read(&path, kind, profile, true)?],
            scripts: Vec::new(),
            system_alias: false,
        });
    }
    Err(missing(key, profile))
}

fn linux_library(
    key: &NativeLinkRequirementKey,
    profile: &ValidatedFinalLinkProfile,
    mode: LinkMode,
) -> Result<LibraryResolution, LinkError> {
    let static_library = mode == LinkMode::Static || key.kind() == NativeLibraryKind::StaticArchive;
    let directory = tempfile::Builder::new()
        .prefix("scoop-system-library-")
        .tempdir()
        .map_err(error)?;
    let mut command = profile.startup_toolchain().driver_command();
    command
        .current_dir(directory.path())
        .arg("-nostdlib")
        .args(if static_library {
            &["-r", "-Wl,-Bstatic"][..]
        } else {
            &["-shared", "-Wl,--no-as-needed"][..]
        })
        .arg("-Wl,-t");
    let name = key.library().as_str();
    // Exact filenames prevent GNU's -l: spelling from overriding an explicit kind.
    command
        .arg("-Xlinker")
        .arg(match key.kind() {
            NativeLibraryKind::StaticArchive => format!("-l:lib{name}.a"),
            NativeLibraryKind::Dynamic => format!("-l:lib{name}.so"),
            NativeLibraryKind::TargetDefault => format!("-l{name}"),
            NativeLibraryKind::Framework => {
                return Err(error(
                    "framework libraries are not supported by the Linux target",
                ));
            }
        })
        .arg("-o")
        .arg(directory.path().join("lookup"));
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        if diagnostic.contains("cannot find -l") {
            return Err(missing(key, profile));
        }
        return Err(error(format!(
            "system native library {name:?}: {}",
            diagnostic.trim()
        )));
    }
    let mut result = LibraryResolution {
        files: Vec::new(),
        scripts: Vec::new(),
        system_alias: false,
    };
    let mut seen = std::collections::BTreeSet::new();
    for line in String::from_utf8(output.stdout).map_err(error)?.lines() {
        let path = Path::new(line.trim());
        if !path.is_absolute() {
            continue;
        }
        let path = path.canonicalize().map_err(error)?;
        if !seen.insert(path.clone()) {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(error)?;
        let kind = if bytes.starts_with(b"!<arch>\n") {
            NativeFileKind::Archive
        } else if bytes.starts_with(b"\x7fELF") {
            if static_library {
                return Err(error(format!(
                    "static native library {name:?} resolved to a dynamic input"
                )));
            }
            NativeFileKind::SharedObject
        } else {
            std::str::from_utf8(&bytes).map_err(error)?;
            result.scripts.push((path, sha256(&bytes)));
            continue;
        };
        let file = super::read_bytes(path, bytes, kind, profile, true)?;
        if profile.id() == scoop_lir::TargetProfileId::LinuxX86_64Musl
            && file.bytes.as_ref() == b"!<arch>\n"
            && matches!(file.content, NativeContent::System)
        {
            result.system_alias = true;
        }
        result.files.push(file);
    }
    if result.files.is_empty() {
        return Err(missing(key, profile));
    }
    Ok(result)
}

fn missing(key: &NativeLinkRequirementKey, profile: &ValidatedFinalLinkProfile) -> LinkError {
    error(format!(
        "missing native library {:?} for {}; no candidate in explicit roots or the selected target's system libraries",
        key.library().as_str(),
        profile.canonical_triple()
    ))
}
