use std::process::Command;

use super::*;

pub(super) fn compile_c_bridge(
    source: &Path,
    object: &Path,
    target_profile: scoop_codegen::TargetProfile,
    file: usize,
) -> Result<(), Vec<Diagnostic>> {
    let output = Command::new("cc")
        .arg("-target")
        .arg(target_profile.canonical_triple())
        .arg("-std=c11")
        .arg("-c")
        .arg(source)
        .arg("-o")
        .arg(object)
        .output()
        .map_err(|error| {
            vec![no_span(
                file,
                format!("failed to run C bridge compiler `cc`: {error}"),
            )]
        })?;
    if !output.status.success() {
        return Err(vec![no_span(
            file,
            format!(
                "C bridge compilation failed (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        )]);
    }
    Ok(())
}

/// Compile the C runtime into a static library cached under
/// `target/scoop-rt/`. The complete source set comes from the selected target
/// profile; rebuilding it is still cheap enough (see
/// `docs/milestone1/DESIGN.md` section 2.7).
pub(super) fn build_runtime(
    file: usize,
    target_profile: scoop_codegen::TargetProfile,
) -> Result<PathBuf, Vec<Diagnostic>> {
    let root = workspace_root();
    let out_dir = root.join("target/scoop-rt");
    std::fs::create_dir_all(&out_dir).map_err(|e| {
        vec![no_span(
            file,
            format!(
                "cannot create runtime build directory {}: {e}",
                out_dir.display()
            ),
        )]
    })?;
    let mut build = cc::Build::new();
    for source in target_profile.runtime_sources() {
        build.file(root.join(source));
    }
    build.include(root.join("runtime/include"));
    for flag in target_profile.runtime_c_flags() {
        build.flag(flag);
    }
    build
        .out_dir(&out_dir)
        // The driver is not a build script: cargo does not provide
        // TARGET/HOST here, so set them explicitly and silence cargo
        // metadata output.
        .target(target_profile.canonical_triple())
        .host(target_profile.canonical_triple())
        .cargo_metadata(false)
        // Outside a build script there is no OPT_LEVEL/DEBUG either.
        .opt_level(0)
        .debug(false)
        .try_compile("scoop_rt")
        .map_err(|e| {
            vec![no_span(
                file,
                format!("failed to compile the C runtime: {e}"),
            )]
        })?;
    Ok(out_dir.join("libscoop_rt.a"))
}

/// Link the object file and the runtime static library into an executable
/// using the system `cc` driver.
pub(super) struct LinkRequest<'a> {
    pub(super) object: &'a Path,
    pub(super) bridge_object: Option<&'a Path>,
    pub(super) runtime_lib: &'a Path,
    pub(super) libraries: &'a [String],
    pub(super) library_paths: &'a [PathBuf],
    pub(super) binary: &'a Path,
    pub(super) target_profile: scoop_codegen::TargetProfile,
    pub(super) file: usize,
}

pub(super) fn link(request: LinkRequest<'_>) -> Result<(), Vec<Diagnostic>> {
    let LinkRequest {
        object,
        bridge_object,
        runtime_lib,
        libraries,
        library_paths,
        binary,
        target_profile,
        file,
    } = request;
    let mut command = Command::new("cc");
    command
        .arg("-target")
        .arg(target_profile.canonical_triple());
    command.arg(object);
    if let Some(bridge_object) = bridge_object {
        command.arg(bridge_object);
    }
    command.arg(runtime_lib);
    for path in library_paths {
        command.arg("-L").arg(path);
    }
    for library in libraries {
        command.arg(format!("-l{library}"));
    }
    command.args(target_profile.linker_args());
    let output = command
        .arg("-o")
        .arg(binary)
        .output()
        .map_err(|e| vec![no_span(file, format!("failed to run the linker `cc`: {e}"))])?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(vec![no_span(
            file,
            format!(
                "linker failed (status {}): {}",
                output.status,
                stderr.trim()
            ),
        )]);
    }
    Ok(())
}
