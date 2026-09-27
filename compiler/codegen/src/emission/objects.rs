use super::*;

/// One verified provisional Scoop object and its exact producer units.
#[derive(Debug)]
pub struct EmittedConeObjectMemberV1 {
    units: ScoopLirObjectUnitSetV1,
    path: std::path::PathBuf,
    kind: EmittedConeObjectMemberKind,
}

#[derive(Debug)]
enum EmittedConeObjectMemberKind {
    NonCallable {
        runtime_metadata: EmittedStrongRuntimeMetadataV1,
        digest_patches: Vec<EmittedStrongDigestPatchMaterializationV1>,
    },
    CallableBody {
        body: scoop_lir::PersistentCallableBodyId,
    },
}

/// Borrowed typed contents of one verified provisional member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmittedConeObjectMemberKindV1<'a> {
    NonCallable {
        runtime_metadata: &'a EmittedStrongRuntimeMetadataV1,
        digest_patches: &'a [EmittedStrongDigestPatchMaterializationV1],
    },
    CallableBody {
        body: scoop_lir::PersistentCallableBodyId,
    },
}

impl EmittedConeObjectMemberV1 {
    pub const fn units(&self) -> &ScoopLirObjectUnitSetV1 {
        &self.units
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn kind(&self) -> EmittedConeObjectMemberKindV1<'_> {
        match &self.kind {
            EmittedConeObjectMemberKind::NonCallable {
                runtime_metadata,
                digest_patches,
            } => EmittedConeObjectMemberKindV1::NonCallable {
                runtime_metadata,
                digest_patches,
            },
            EmittedConeObjectMemberKind::CallableBody { body } => {
                EmittedConeObjectMemberKindV1::CallableBody { body: *body }
            }
        }
    }
}

/// Complete verified provisional Scoop object set retained for `.slib`
/// packaging. The owned temporary directory keeps every member immutable and
/// alive for exactly as long as this result.
#[derive(Debug)]
pub struct EmittedConeObjectSet<P> {
    target_selection: scoop_lir::ValidatedLirTargetSelection,
    foundation: scoop_lir::ConeLirFoundation,
    production: P,
    partition: ScoopLirObjectPartitionV1,
    members: Vec<EmittedConeObjectMemberV1>,
    backing: tempfile::TempDir,
}

pub type EmittedConeObjectSetV1 = EmittedConeObjectSet<scoop_lir::ConeProductionSectionV1>;
pub type EmittedConeObjectSetV2 = EmittedConeObjectSet<scoop_lir::ConeProductionSectionV2>;

impl<P> EmittedConeObjectSet<P> {
    pub const fn target(&self) -> scoop_lir::LirTargetProfile {
        self.target_selection.target()
    }

    pub const fn target_selection(&self) -> scoop_lir::ValidatedLirTargetSelection {
        self.target_selection
    }

    pub const fn foundation(&self) -> &scoop_lir::ConeLirFoundation {
        &self.foundation
    }

    pub const fn production(&self) -> &P {
        &self.production
    }

    /// Returns the production records after the caller has consumed the
    /// temporary object members.
    pub fn into_production(self) -> P {
        self.production
    }

    pub fn members(&self) -> &[EmittedConeObjectMemberV1] {
        &self.members
    }

    pub const fn partition(&self) -> &ScoopLirObjectPartitionV1 {
        &self.partition
    }

    pub fn temporary_directory(&self) -> &Path {
        self.backing.path()
    }
}

/// One rendered physical member used by diagnostics and golden tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedConeObjectModuleV1 {
    units: ScoopLirObjectUnitSetV1,
    llvm_ir: String,
}

impl RenderedConeObjectModuleV1 {
    pub const fn units(&self) -> &ScoopLirObjectUnitSetV1 {
        &self.units
    }

    pub fn llvm_ir(&self) -> &str {
        &self.llvm_ir
    }
}

/// Emits provisional objects and retains their production and patch records
/// for `.slib` packaging.
pub fn emit_object_set(
    input: &scoop_lir::ConeLirOutput,
    coordinate: &scoop_lir::ConeCoordinate,
    direct_dependencies: &[scoop_lir::ConeIdentity],
    entry_source: scoop_lir::EntryProductionSourceV1,
    temporary_parent: &Path,
    profile: ValidatedBackendProfile,
) -> Result<EmittedConeObjectSetV1, CodegenError> {
    validate_object_set_input(input, profile)?;
    let production = input
        .build_production_section(coordinate.clone(), direct_dependencies, entry_source)
        .map_err(|error| {
            CodegenError(format!("cannot build strong production section: {error}"))
        })?;
    emit_object_set_with_production(input, production, temporary_parent, profile)
}

/// Emits the complete LIR and Strong V2 production records for one Cone.
pub fn emit_object_set_v2(
    input: &scoop_lir::ConeLirOutput,
    production: scoop_lir::ConeProductionSectionV2,
    temporary_parent: &Path,
    profile: ValidatedBackendProfile,
) -> Result<EmittedConeObjectSetV2, CodegenError> {
    validate_object_set_input(input, profile)?;
    if production.image_plan().cone().identity() != input.module().cone {
        return Err(CodegenError(
            "strong production belongs to a different Cone than the emitted LIR".to_owned(),
        ));
    }
    emit_object_set_with_production(input, production, temporary_parent, profile)
}

fn emit_object_set_with_production<D: scoop_lir::StrongDescriptorReference, C: Clone, I: Clone>(
    input: &scoop_lir::ConeLirOutput,
    production: scoop_lir::ConeProductionSection<D, C, I>,
    temporary_parent: &Path,
    profile: ValidatedBackendProfile,
) -> Result<EmittedConeObjectSet<scoop_lir::ConeProductionSection<D, C, I>>, CodegenError> {
    let module = input.module();
    let partition =
        ScoopLirObjectPartitionV1::from_input(input, production.canonical_definitions())
            .map_err(|error| CodegenError(error.to_string()))?;
    let expected_safepoints = statepoint::expectations(module)?;
    let expected_eh = artifact::eh_expectations(module)?;
    let machine = profile.create_target_machine()?;
    std::fs::create_dir_all(temporary_parent).map_err(|error| {
        CodegenError(format!(
            "cannot create object temporary parent {}: {error}",
            temporary_parent.display()
        ))
    })?;
    let backing = tempfile::Builder::new()
        .prefix("scoop-lir-")
        .tempdir_in(temporary_parent)
        .map_err(|error| {
            CodegenError(format!(
                "cannot create immutable object backing under {}: {error}",
                temporary_parent.display()
            ))
        })?;
    let mut members = Vec::with_capacity(partition.objects().len());
    for units in partition.objects() {
        let path = backing
            .path()
            .join(format!("{}.o", units.definition_plans()[0]));
        let context = Context::create();
        let member = match units.kind() {
            ScoopLirObjectKindV1::NonCallable => {
                let selected_safepoints = expected_safepoints.without_body_sites();
                let selected_eh = expected_eh.without_body_metadata();
                let (llvm, runtime_metadata) = prepare_non_callable_strong_llvm_module(
                    &context,
                    module,
                    &production,
                    &machine,
                    profile,
                    &selected_safepoints,
                )?;
                write_object(&machine, &llvm, &path)?;
                atom_boundaries::materialize_global_linkages_v1(
                    &path,
                    module.meta.target_profile,
                    production.canonical_definitions(),
                    units.definition_plans(),
                )?;
                verify_and_seal_object(&path, profile, &selected_safepoints, &selected_eh)?;
                let digest_patches =
                    object_materialization::resolve_digest_patch_materializations_v1(
                        &path,
                        module.meta.target_profile,
                        production.canonical_definitions(),
                        &runtime_metadata,
                    )?;
                EmittedConeObjectMemberV1 {
                    units: units.clone(),
                    path,
                    kind: EmittedConeObjectMemberKind::NonCallable {
                        runtime_metadata,
                        digest_patches,
                    },
                }
            }
            ScoopLirObjectKindV1::CallableBody(body) => {
                let function = module
                    .functions
                    .iter()
                    .find(|function| function.callable_body.id() == body)
                    .ok_or_else(|| {
                        CodegenError(format!(
                            "strong object partition selected missing callable body {body}"
                        ))
                    })?;
                let selected_safepoints = expected_safepoints.for_function(function.symbol())?;
                let selected_eh = expected_eh.for_function(function.symbol());
                let llvm = prepare_callable_strong_llvm_module(
                    &context,
                    module,
                    &production,
                    &machine,
                    profile,
                    &selected_safepoints,
                    body,
                )?;
                write_object(&machine, &llvm, &path)?;
                let definition = production
                    .canonical_definitions()
                    .plan(units.definition_plans()[0])
                    .ok_or_else(|| {
                        CodegenError(format!(
                            "callable object unit {} has no canonical symbol plan",
                            units.definition_plans()[0]
                        ))
                    })?;
                if let Err(error) = crate::callable_atom_boundaries::materialize_v1(
                    &path,
                    module.meta.target_profile,
                    definition,
                    body,
                ) {
                    return Err(discard_invalid_object(&path, error));
                }
                verify_and_seal_object(&path, profile, &selected_safepoints, &selected_eh)?;
                EmittedConeObjectMemberV1 {
                    units: units.clone(),
                    path,
                    kind: EmittedConeObjectMemberKind::CallableBody { body },
                }
            }
        };
        members.push(member);
    }
    Ok(EmittedConeObjectSet {
        target_selection: profile.lir_target_selection(),
        foundation: input.foundation().clone(),
        production,
        partition,
        members,
        backing,
    })
}

fn validate_object_set_input(
    input: &scoop_lir::ConeLirOutput,
    profile: ValidatedBackendProfile,
) -> Result<(), CodegenError> {
    validation::validate_module(input.module())?;
    profile.validate_lir_target_profile(input.module().meta.target_profile)?;
    if let scoop_lir::LirOutput::Executable { entry } = input.module().output {
        validation::validate_executable_entry(input.module(), entry)?;
    }
    Ok(())
}

/// Render every physical strong object module without writing artifacts.
pub fn render_llvm_ir_members(
    input: &scoop_lir::ConeLirOutput,
    coordinate: &scoop_lir::ConeCoordinate,
    direct_dependencies: &[scoop_lir::ConeIdentity],
    entry_source: scoop_lir::EntryProductionSourceV1,
    profile: ValidatedBackendProfile,
) -> Result<Vec<RenderedConeObjectModuleV1>, CodegenError> {
    validate_object_set_input(input, profile)?;
    let module = input.module();
    let production = input
        .build_production_section(coordinate.clone(), direct_dependencies, entry_source)
        .map_err(|error| {
            CodegenError(format!("cannot build strong production section: {error}"))
        })?;
    let partition =
        ScoopLirObjectPartitionV1::from_input(input, production.canonical_definitions())
            .map_err(|error| CodegenError(error.to_string()))?;
    let expected_safepoints = statepoint::expectations(module)?;
    let machine = profile.create_target_machine()?;
    partition
        .objects()
        .iter()
        .map(|units| {
            let context = Context::create();
            let llvm = match units.kind() {
                ScoopLirObjectKindV1::NonCallable => {
                    let selected = expected_safepoints.without_body_sites();
                    prepare_non_callable_strong_llvm_module(
                        &context,
                        module,
                        &production,
                        &machine,
                        profile,
                        &selected,
                    )?
                    .0
                }
                ScoopLirObjectKindV1::CallableBody(body) => {
                    let function = module
                        .functions
                        .iter()
                        .find(|function| function.callable_body.id() == body)
                        .ok_or_else(|| {
                            CodegenError(format!(
                                "strong object partition selected missing callable body {body}"
                            ))
                        })?;
                    let selected = expected_safepoints.for_function(function.symbol())?;
                    prepare_callable_strong_llvm_module(
                        &context,
                        module,
                        &production,
                        &machine,
                        profile,
                        &selected,
                        body,
                    )?
                }
            };
            Ok(RenderedConeObjectModuleV1 {
                units: units.clone(),
                llvm_ir: llvm.print_to_string().to_string(),
            })
        })
        .collect()
}
