use std::path::Path;

use scoop_identity::{ConeIdentity, NormalizedNativeSourcePath};
use scoop_toolchain::ToolchainError;

use super::{ValidatedCurrentConeInput, ValidatedSingleConeBuildRequest};
use crate::request::NativeInputOrigin;

pub(super) fn compile(
    request: &ValidatedSingleConeBuildRequest<'_>,
    cone: ConeIdentity,
    temporary_parent: &Path,
) -> Result<Vec<scoop_slib::UncompressedObjectMember>, ToolchainError> {
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
    let target = request.target();
    let selected =
        scoop_toolchain::selected_native_object_sources(manifest, target, request.purpose());
    if selected.is_empty()
        && matches!(
            &request.request.native_inputs,
            NativeInputOrigin::Source { .. }
        )
    {
        return Ok(Vec::new());
    }
    let directory;
    let prepared;
    let objects = match &request.request.native_inputs {
        NativeInputOrigin::Objects(objects) => objects.as_slice(),
        NativeInputOrigin::Source { sysroot } => {
            let sysroot = scoop_toolchain::configured_sysroot_root(sysroot.clone())?;
            directory = tempfile::Builder::new()
                .prefix("native-")
                .tempdir_in(temporary_parent)
                .map_err(error)?;
            prepared = scoop_toolchain::prepare_native_objects(
                manifest,
                target,
                request.optimization(),
                &sysroot,
                directory.path(),
                request.purpose(),
            )?;
            prepared.objects()
        }
    };
    if selected.len() != objects.len() {
        return Err(error(format!(
            "native object count {} does not match {} selected sources",
            objects.len(),
            selected.len()
        )));
    }
    selected
        .iter()
        .zip(objects)
        .map(|(source, object)| {
            let source = NormalizedNativeSourcePath::new(source.as_str()).map_err(error)?;
            scoop_slib::native_link_object_member(
                cone,
                &source,
                std::fs::read(object).map_err(error)?,
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
