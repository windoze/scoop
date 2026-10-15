use std::path::Path;

use scoop_identity::{ConeIdentity, NormalizedSourcePath};
use scoop_toolchain::{NativeToolchain, ToolchainError};

use super::{ValidatedCurrentConeInput, ValidatedSingleConeBuildRequest};
use crate::request::NativeInputOrigin;

pub(super) fn compile(
    request: &ValidatedSingleConeBuildRequest<'_>,
    cone: ConeIdentity,
    temporary_parent: &Path,
) -> Result<Vec<scoop_slib::SlibMember>, ToolchainError> {
    let ValidatedCurrentConeInput::Manifest { manifest } = request.current() else {
        if let NativeInputOrigin::Objects(inputs) = &request.request.native_inputs
            && !inputs.is_empty()
        {
            return Err(error(
                "single-file input cannot have native translation units",
            ));
        }
        return Ok(Vec::new());
    };
    let config = manifest.parsed().semantic().native();
    let target = request.target();
    let mut selected: Vec<_> = config
        .sources()
        .iter()
        .filter(|s| s.predicate().matches(target.id()))
        .collect();
    selected.sort_by_key(|s| s.path());
    if let NativeInputOrigin::Objects(objects) = &request.request.native_inputs {
        if selected.len() != objects.len() {
            return Err(error(format!(
                "native object count {} does not match {} selected sources",
                objects.len(),
                selected.len()
            )));
        }
        return selected
            .iter()
            .zip(objects)
            .map(|(source, object)| {
                let source = NormalizedSourcePath::new(source.path().as_str()).map_err(error)?;
                scoop_slib::native_link_object_member(
                    cone,
                    target.lir_target(),
                    &source,
                    std::fs::read(object).map_err(error)?,
                )
                .map_err(error)
            })
            .collect();
    }
    let compiler = NativeToolchain::resolve(target.c_bridge_toolchain(), manifest)?;
    let prepared = compiler.prepare(
        manifest,
        request.optimization(),
        &scoop_toolchain::development_runtime_root().join("include"),
    )?;
    let units = prepared.units();
    if units.is_empty() {
        return Ok(Vec::new());
    }
    let directory = tempfile::Builder::new()
        .prefix("native-")
        .tempdir_in(temporary_parent)
        .map_err(error)?;
    units
        .iter()
        .enumerate()
        .map(|(index, unit)| {
            let output = directory.path().join(format!("{index:08}.o"));
            scoop_toolchain::compile_native_source(
                unit,
                config,
                &compiler,
                request.optimization(),
                &output,
            )?;
            let source = NormalizedSourcePath::new(unit.path().as_str()).map_err(error)?;
            scoop_slib::native_link_object_member(
                cone,
                target.lir_target(),
                &source,
                std::fs::read(output).map_err(error)?,
            )
            .map_err(error)
        })
        .collect()
}

fn error(error: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("native compilation: {error}"))
}

pub(super) fn libraries(
    request: &ValidatedSingleConeBuildRequest<'_>,
) -> Result<Vec<scoop_lir::CanonicalNativeLibraryRequirementV1>, ToolchainError> {
    match request.current() {
        ValidatedCurrentConeInput::Manifest { manifest } => manifest
            .parsed()
            .semantic()
            .native()
            .library_requirements(request.target().id())
            .map_err(error),
        ValidatedCurrentConeInput::SingleFile { .. } => Ok(Vec::new()),
    }
}

pub(super) fn cxx(request: &ValidatedSingleConeBuildRequest<'_>) -> bool {
    matches!(request.current(), ValidatedCurrentConeInput::Manifest { manifest } if manifest.parsed().semantic().native().cxx())
}
