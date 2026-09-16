//! Shared build-wide resource accounting for the production `scoopc` path.

use scoop_manifest::{
    SourceDiscoveryLimits, discover_manifest_sources_with_limits,
    load_single_file_source_with_limit,
};
use scoop_slib::{
    SlibClosureDecodeLimitsV1, SlibClosureDecodeMeterV1, SlibClosureDecodePurposeV1,
    probe_prebuilt_manifest_summary,
};
use scoop_toolchain::ResolvedSlibClosureLimitsV1;
use scoop_wire::{DecodeLimits, sha256};

use super::*;

impl SingleConeBuildRequest {
    pub(super) fn build_and_publish_with_closure_limits(
        self,
        limits: DecodeLimits,
        closure_limits: SlibClosureDecodeLimitsV1,
    ) -> Result<SingleConeProductionSuccess, SingleConeProductionError> {
        let temporary_parent = self
            .output
            .as_path()
            .parent()
            .expect("an absolute output path always has a parent");
        let temporary = tempfile::Builder::new()
            .prefix(".scoopc-")
            .tempdir_in(temporary_parent)
            .map_err(SingleConeProductionError::TemporaryWorkspace)?;
        let mut meter = SlibClosureDecodeMeterV1::new(closure_limits);
        let loaded = self
            .load_preflight_metered(limits, &mut meter)
            .map_err(SingleConeProductionError::Preflight)?;
        let validated = loaded
            .validate_metered(&mut meter)
            .map_err(SingleConeProductionError::Validation)?;
        let parsed = validated
            .parse_current_sources_metered(&mut meter)
            .map_err(SingleConeProductionError::Sources)?;
        match parsed {
            ParsedSingleConeBuildRequest::Ordinary(parsed) => parsed
                .build_and_publish(temporary.path(), limits)
                .map_err(SingleConeProductionError::Ordinary),
            ParsedSingleConeBuildRequest::TrustedCoreBootstrap(parsed) => parsed
                .build_and_publish(temporary.path(), limits)
                .map_err(SingleConeProductionError::CoreBootstrap),
        }
    }

    pub(super) fn load_preflight_inner(
        self,
        limits: DecodeLimits,
        mut meter: Option<&mut SlibClosureDecodeMeterV1>,
    ) -> Result<LoadedSingleConeBuildRequest, SingleConePreflightError> {
        let Self {
            current,
            dependencies,
            trusted_core,
            target,
            output,
            diagnostics,
            emit,
        } = self;
        let current = load_current_input(current)?;
        let dependencies = match meter.as_deref_mut() {
            Some(meter) => LoadedExplicitDependencyInputs::load_metered(
                dependencies.direct(),
                dependencies.support(),
                limits,
                meter,
            ),
            None => LoadedExplicitDependencyInputs::load(
                dependencies.direct(),
                dependencies.support(),
                limits,
            ),
        }
        .map_err(|source| SingleConePreflightError::ExplicitDependencyLoad(Box::new(source)))?;
        let trusted_core = match trusted_core {
            TrustedCoreInput::Artifact(input) => {
                let loaded = match meter {
                    Some(meter) => input.load_metered(limits, meter),
                    None => input.load(limits),
                }
                .map_err(|source| SingleConePreflightError::TrustedCoreLoad(Box::new(source)))?;
                LoadedTrustedCoreInput::Artifact(loaded)
            }
            TrustedCoreInput::BootstrapSelf { artifact_slot } => {
                LoadedTrustedCoreInput::BootstrapSelf { artifact_slot }
            }
        };
        Ok(LoadedSingleConeBuildRequest {
            current,
            dependencies,
            trusted_core,
            target,
            output,
            diagnostics,
            emit,
        })
    }

    fn load_preflight_metered(
        self,
        limits: DecodeLimits,
        meter: &mut SlibClosureDecodeMeterV1,
    ) -> Result<LoadedSingleConeBuildRequest, SingleConePreflightError> {
        self.load_preflight_inner(limits, Some(meter))
    }
}

impl LoadedSingleConeBuildRequest {
    pub(super) fn validate_inner(
        &self,
        mut meter: Option<&mut SlibClosureDecodeMeterV1>,
    ) -> Result<ValidatedCoreOnlyBuildRequest<'_>, CoreOnlyRequestValidationError> {
        let (current, dependencies) = match (&self.current, &self.trusted_core) {
            (
                LoadedCurrentConeInput::Manifest { manifest },
                LoadedTrustedCoreInput::Artifact(artifact),
            ) => {
                let trusted_core = Box::new(validate_trusted_core_input(
                    artifact,
                    &self.target,
                    meter.as_deref_mut(),
                )?);
                let current_identity = manifest
                    .parsed()
                    .semantic()
                    .coordinate()
                    .identity()
                    .map_err(CoreOnlyRequestValidationError::CurrentIdentity)?;
                let dependencies = match meter {
                    Some(meter) => self.dependencies.validate_metered(
                        Some(manifest),
                        current_identity,
                        trusted_core.as_ref(),
                        &self.target,
                        meter,
                    ),
                    None => self.dependencies.validate(
                        Some(manifest),
                        current_identity,
                        trusted_core.as_ref(),
                        &self.target,
                    ),
                }
                .map_err(CoreOnlyRequestValidationError::ExplicitDependencies)?;
                (
                    ValidatedCurrentConeInput::Manifest {
                        manifest,
                        trusted_core,
                    },
                    dependencies,
                )
            }
            (
                LoadedCurrentConeInput::SingleFile { source },
                LoadedTrustedCoreInput::Artifact(artifact),
            ) => {
                let trusted_core = Box::new(validate_trusted_core_input(
                    artifact,
                    &self.target,
                    meter.as_deref_mut(),
                )?);
                let dependencies = match meter {
                    Some(meter) => self.dependencies.validate_metered(
                        None,
                        ConeIdentity::SINGLE_FILE,
                        trusted_core.as_ref(),
                        &self.target,
                        meter,
                    ),
                    None => self.dependencies.validate(
                        None,
                        ConeIdentity::SINGLE_FILE,
                        trusted_core.as_ref(),
                        &self.target,
                    ),
                }
                .map_err(CoreOnlyRequestValidationError::ExplicitDependencies)?;
                (
                    ValidatedCurrentConeInput::SingleFile {
                        source,
                        trusted_core,
                    },
                    dependencies,
                )
            }
            (
                LoadedCurrentConeInput::TrustedCoreBootstrap { input },
                LoadedTrustedCoreInput::BootstrapSelf { artifact_slot },
            ) => {
                let dependencies = match meter {
                    Some(meter) => self.dependencies.validate_bootstrap_empty_metered(meter),
                    None => self.dependencies.validate_bootstrap_empty(),
                }
                .map_err(CoreOnlyRequestValidationError::ExplicitDependencies)?;
                (
                    ValidatedCurrentConeInput::TrustedCoreBootstrap {
                        input,
                        artifact_slot,
                    },
                    dependencies,
                )
            }
            _ => return Err(CoreOnlyRequestValidationError::InvalidLoadedInputPair),
        };
        Ok(ValidatedCoreOnlyBuildRequest {
            request: self,
            current,
            dependencies,
        })
    }

    fn validate_metered(
        &self,
        meter: &mut SlibClosureDecodeMeterV1,
    ) -> Result<ValidatedCoreOnlyBuildRequest<'_>, CoreOnlyRequestValidationError> {
        self.validate_inner(Some(meter))
    }
}

fn validate_trusted_core_input<'input>(
    artifact: &'input LoadedTrustedCoreArtifact,
    target: &scoop_toolchain::ResolvedTargetProfile,
    mut meter: Option<&mut SlibClosureDecodeMeterV1>,
) -> Result<ValidatedTrustedCoreArtifact<'input>, CoreOnlyRequestValidationError> {
    let snapshot = sha256(artifact.bytes());
    if let Some(meter) = meter.as_deref_mut() {
        let summary = probe_prebuilt_manifest_summary(
            artifact.bytes(),
            artifact.limits(),
            target.lir_target_selection(),
        )
        .map_err(|source| CoreOnlyRequestValidationError::TrustedCoreSummary(Box::new(source)))?;
        meter
            .observe_artifact_snapshot(&summary, snapshot)
            .map_err(CoreOnlyRequestValidationError::Resource)?;
        meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::GraphSummary,
                summary.artifact_fingerprint(),
                snapshot,
                summary.decode_usage(),
            )
            .map_err(CoreOnlyRequestValidationError::Resource)?;
    }
    let validated = artifact
        .validate(target)
        .map_err(|source| CoreOnlyRequestValidationError::TrustedCore(Box::new(source)))?;
    if let Some(meter) = meter {
        let publication = validated.publication();
        meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::Compile,
                publication.artifact_fingerprint(),
                snapshot,
                publication.compile_summary().decode_usage(),
            )
            .map_err(CoreOnlyRequestValidationError::Resource)?;
        meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::Link,
                publication.artifact_fingerprint(),
                snapshot,
                publication.link_summary().decode_usage(),
            )
            .map_err(CoreOnlyRequestValidationError::Resource)?;
    }
    Ok(validated)
}

impl<'input> ValidatedCoreOnlyBuildRequest<'input> {
    fn parse_current_sources_metered<'request>(
        &'request self,
        meter: &mut SlibClosureDecodeMeterV1,
    ) -> Result<ParsedSingleConeBuildRequest<'request, 'input>, CurrentConeSourceStageError> {
        match &self.current {
            ValidatedCurrentConeInput::Manifest {
                manifest,
                trusted_core,
            } => Ok(ParsedSingleConeBuildRequest::Ordinary(
                ParsedOrdinaryConeBuildRequest {
                    request: self,
                    trusted_core,
                    sources: parse_manifest_current_metered(manifest, meter)?,
                },
            )),
            ValidatedCurrentConeInput::SingleFile {
                source,
                trusted_core,
            } => Ok(ParsedSingleConeBuildRequest::Ordinary(
                ParsedOrdinaryConeBuildRequest {
                    request: self,
                    trusted_core,
                    sources: parse_single_file_current_metered(source, meter)?,
                },
            )),
            ValidatedCurrentConeInput::TrustedCoreBootstrap {
                input,
                artifact_slot,
            } => Ok(ParsedSingleConeBuildRequest::TrustedCoreBootstrap(
                ParsedCoreBootstrapBuildRequest {
                    request: self,
                    authority: input.authority(),
                    artifact_slot,
                    sources: parse_manifest_current_metered(input.source_slot().manifest(), meter)?,
                },
            )),
        }
    }
}

fn parse_manifest_current_metered(
    manifest: &LoadedConeManifest,
    meter: &mut SlibClosureDecodeMeterV1,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    let limits = remaining_source_limits(meter);
    let sources = discover_manifest_sources_with_limits(manifest, limits)
        .map_err(|source| CurrentConeSourceStageError::Discovery(Box::new(source)))?;
    meter
        .charge_sources(sources.usage().files(), sources.usage().bytes())
        .map_err(CurrentConeSourceStageError::Resource)?;
    parse_discovered_sources(&sources)
}

pub(super) fn parse_single_file_current_metered(
    source: &SingleFileLocator,
    meter: &mut SlibClosureDecodeMeterV1,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    meter
        .charge_sources(1, 0)
        .map_err(CurrentConeSourceStageError::Resource)?;
    let byte_limit = remaining_source_limits(meter).bytes();
    let source = load_single_file_source_with_limit(source, byte_limit)
        .map_err(|source| CurrentConeSourceStageError::SingleFile(Box::new(source)))?;
    let byte_length = u64::try_from(source.source_text().len())
        .map_err(|_| CurrentConeSourceStageError::SourceLengthOverflow)?;
    meter
        .charge_sources(0, byte_length)
        .map_err(CurrentConeSourceStageError::Resource)?;
    parse_single_discovered_source(&source)
}

fn remaining_source_limits(meter: &SlibClosureDecodeMeterV1) -> SourceDiscoveryLimits {
    let limits = meter.limits().values();
    let usage = meter.usage();
    SourceDiscoveryLimits::new(
        limits.source_files.saturating_sub(usage.source_files),
        limits.source_bytes.saturating_sub(usage.source_bytes),
    )
}

pub(super) fn production_closure_limits() -> SlibClosureDecodeLimitsV1 {
    ResolvedSlibClosureLimitsV1::M23_DEFAULT.limits()
}
