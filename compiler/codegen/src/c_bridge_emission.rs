use std::path::{Path, PathBuf};

use super::*;

/// One generated-C object together with the exact bridge-unit authority and
/// source path from which it was compiled.
#[derive(Debug)]
pub struct EmittedGeneratedCBridgeObjectMemberV1 {
    plan: scoop_lir::GeneratedBridgeUnitPlanV1,
    source_path: PathBuf,
    object_path: PathBuf,
}

impl EmittedGeneratedCBridgeObjectMemberV1 {
    pub const fn unit(&self) -> scoop_lir::GeneratedBridgeUnitId {
        self.plan.unit()
    }

    pub const fn plan(&self) -> &scoop_lir::GeneratedBridgeUnitPlanV1 {
        &self.plan
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn object_path(&self) -> &Path {
        &self.object_path
    }
}

/// Complete generated-C source/object production for one sealed strong LIR
/// product. The owned temporary directory keeps all source and object files
/// immutable and alive for exactly as long as this result.
#[derive(Debug)]
pub struct EmittedGeneratedCBridgeObjectSetV1 {
    profile: scoop_lir::CBridgeToolchainProfileV1,
    production: scoop_lir::CBridgeProductionSetV1,
    sources: GeneratedCBridgeSourceSetV1,
    members: Vec<EmittedGeneratedCBridgeObjectMemberV1>,
    backing: tempfile::TempDir,
}

impl EmittedGeneratedCBridgeObjectSetV1 {
    pub const fn profile(&self) -> &scoop_lir::CBridgeToolchainProfileV1 {
        &self.profile
    }

    pub const fn production(&self) -> &scoop_lir::CBridgeProductionSetV1 {
        &self.production
    }

    pub const fn sources(&self) -> &GeneratedCBridgeSourceSetV1 {
        &self.sources
    }

    pub fn members(&self) -> &[EmittedGeneratedCBridgeObjectMemberV1] {
        &self.members
    }

    pub fn temporary_directory(&self) -> &Path {
        self.backing.path()
    }
}

/// Generate and compile exactly one canonical C translation unit for every
/// bridge unit in a sealed strong LIR product.
pub fn emit_c_bridge_object_set(
    input: &scoop_lir::ConeLirOutput,
    temporary_parent: &Path,
    profile: &scoop_lir::ValidatedCBridgeToolchainInvocation,
) -> Result<EmittedGeneratedCBridgeObjectSetV1, CodegenError> {
    profile
        .validate_target(input.module().meta.target_profile)
        .map_err(|error| {
            CodegenError(format!("invalid generated-C toolchain projection: {error}"))
        })?;
    let target = input.module().meta.target_profile;
    let symbol_surface = scoop_lir::StrongObjectSymbolSurfaceV1::from_foundation(
        input.foundation(),
    )
    .map_err(|error| {
        CodegenError(format!(
            "cannot plan generated-C atom boundary symbols: {error}"
        ))
    })?;
    let sources = render_c_bridge_source_set(input)?;
    emit_source_set_with_compiler(
        sources,
        temporary_parent,
        profile.profile().clone(),
        |plan, source, object| {
            let output = profile
                .object_compilation_command(source, object)
                .output()
                .map_err(|error| {
                    CodegenError(format!(
                        "failed to run generated-C compiler `{}`: {error}",
                        profile.compiler_driver().display()
                    ))
                })?;
            if !output.status.success() {
                return Err(CodegenError(format!(
                    "generated-C compilation failed (status {}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                )));
            }
            crate::generated_c_atom_boundaries::materialize_v1(
                object,
                target,
                plan,
                &symbol_surface,
            )
        },
    )
}

fn emit_source_set_with_compiler(
    sources: GeneratedCBridgeSourceSetV1,
    temporary_parent: &Path,
    profile: scoop_lir::CBridgeToolchainProfileV1,
    mut compile: impl FnMut(
        &scoop_lir::GeneratedBridgeUnitPlanV1,
        &Path,
        &Path,
    ) -> Result<(), CodegenError>,
) -> Result<EmittedGeneratedCBridgeObjectSetV1, CodegenError> {
    std::fs::create_dir_all(temporary_parent).map_err(|error| {
        CodegenError(format!(
            "cannot create generated-C temporary parent {}: {error}",
            temporary_parent.display()
        ))
    })?;
    let backing = tempfile::Builder::new()
        .prefix("scoop-generated-c-")
        .tempdir_in(temporary_parent)
        .map_err(|error| {
            CodegenError(format!(
                "cannot create immutable generated-C backing under {}: {error}",
                temporary_parent.display()
            ))
        })?;

    if sources.plan().units().len() != sources.units().len() {
        return Err(CodegenError(format!(
            "generated-C source set has {} authority plans but {} source units",
            sources.plan().units().len(),
            sources.units().len()
        )));
    }

    let mut members = Vec::with_capacity(sources.units().len());
    for (plan, source) in sources.plan().units().iter().zip(sources.units()) {
        if plan.unit() != source.unit() {
            return Err(CodegenError(format!(
                "generated-C source unit {} does not match authority plan {}",
                source.unit(),
                plan.unit()
            )));
        }
        let source_path = backing.path().join(format!("{}.c", source.unit()));
        let object_path = backing.path().join(format!("{}.o", source.unit()));
        std::fs::write(&source_path, source.source()).map_err(|error| {
            CodegenError(format!(
                "cannot write generated-C source for unit {} to {}: {error}",
                source.unit(),
                source_path.display()
            ))
        })?;
        seal_regular_file(&source_path, "generated-C source", false)?;
        compile(plan, &source_path, &object_path).map_err(|error| {
            CodegenError(format!(
                "cannot compile generated-C unit {}: {error}",
                source.unit()
            ))
        })?;
        seal_regular_file(&object_path, "generated-C object", true)?;
        members.push(EmittedGeneratedCBridgeObjectMemberV1 {
            plan: plan.clone(),
            source_path,
            object_path,
        });
    }

    let production =
        scoop_lir::CBridgeProductionSetV1::from_generated_bridge_plan(sources.plan(), &profile);
    Ok(EmittedGeneratedCBridgeObjectSetV1 {
        profile,
        production,
        sources,
        members,
        backing,
    })
}

fn seal_regular_file(path: &Path, role: &str, require_nonempty: bool) -> Result<(), CodegenError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        CodegenError(format!("cannot inspect {role} {}: {error}", path.display()))
    })?;
    if !metadata.file_type().is_file() {
        return Err(CodegenError(format!(
            "{role} {} is not a regular file",
            path.display()
        )));
    }
    if require_nonempty && metadata.len() == 0 {
        return Err(CodegenError(format!("{role} {} is empty", path.display())));
    }
    let mut permissions = metadata.permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).map_err(|error| {
        CodegenError(format!(
            "cannot seal {role} {} read-only: {error}",
            path.display()
        ))
    })
}

#[cfg(test)]
pub(crate) fn emit_c_bridge_object_set_with_compiler_for_test(
    sources: GeneratedCBridgeSourceSetV1,
    temporary_parent: &Path,
    profile: scoop_lir::CBridgeToolchainProfileV1,
    compile: impl FnMut(&scoop_lir::GeneratedBridgeUnitPlanV1, &Path, &Path) -> Result<(), CodegenError>,
) -> Result<EmittedGeneratedCBridgeObjectSetV1, CodegenError> {
    emit_source_set_with_compiler(sources, temporary_parent, profile, compile)
}
