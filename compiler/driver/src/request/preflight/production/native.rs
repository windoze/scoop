use std::path::Path;

use scoop_identity::{ConeIdentity, NormalizedSourcePath};
use scoop_manifest::ImmutableInputSnapshot;
use scoop_toolchain::{NativeSourceInput, ToolchainError};

use super::{ValidatedCurrentConeInput, ValidatedSingleConeBuildRequest};
use crate::request::NativeInputOrigin;

pub(super) fn compile(
    request: &ValidatedSingleConeBuildRequest<'_>,
    cone: ConeIdentity,
    temporary_parent: &Path,
) -> Result<Vec<scoop_slib::SlibMember>, ToolchainError> {
    let ValidatedCurrentConeInput::Manifest { manifest } = request.current() else {
        if let NativeInputOrigin::Preprocessed(inputs) = &request.request.native_inputs
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
    let units = match &request.request.native_inputs {
        NativeInputOrigin::Source => scoop_toolchain::prepare_native_inputs(
            manifest,
            target.id(),
            target.c_bridge_toolchain(),
            request.optimization(),
            &scoop_toolchain::development_runtime_root().join("include"),
        )?
        .units()
        .to_vec(),
        NativeInputOrigin::Preprocessed(inputs) => {
            let mut selected: Vec<_> = config
                .sources()
                .iter()
                .filter(|source| source.predicate().matches(target.id()))
                .collect();
            selected.sort_by_key(|source| source.path());
            if selected.len() != inputs.len() {
                return Err(error(format!(
                    "native input count {} does not match {} selected sources",
                    inputs.len(),
                    selected.len()
                )));
            }
            selected
                .iter()
                .zip(inputs)
                .map(|(source, input)| {
                    let captured = ImmutableInputSnapshot::capture(input).map_err(error)?;
                    Ok(NativeSourceInput::new(
                        source.path().clone(),
                        captured.shared_bytes(),
                    ))
                })
                .collect::<Result<Vec<_>, ToolchainError>>()?
        }
    };
    if units.is_empty() {
        return Ok(Vec::new());
    }
    let directory = tempfile::Builder::new()
        .prefix("native-")
        .tempdir_in(temporary_parent)
        .map_err(error)?;
    units
        .into_iter()
        .enumerate()
        .map(|(index, unit)| {
            let output = directory.path().join(format!("{index:08}.o"));
            scoop_toolchain::compile_native_source(
                &unit,
                config,
                target.c_bridge_toolchain(),
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
