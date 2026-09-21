//! Shared build-wide resource accounting for the production `scoopc` path.

use scoop_manifest::{
    SourceDiscoveryLimits, discover_manifest_sources_with_limits,
    load_single_file_source_with_limit,
};
use scoop_slib::{SlibClosureDecodeLimitsV1, SlibClosureDecodeMeterV1};
use scoop_toolchain::ResolvedSlibClosureLimitsV1;
use scoop_wire::DecodeLimits;

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
                .map_err(|source| SingleConeProductionError::Ordinary(Box::new(source))),
            ParsedSingleConeBuildRequest::TrustedCoreBootstrap(parsed) => parsed
                .build_and_publish(temporary.path(), limits)
                .map_err(SingleConeProductionError::CoreBootstrap),
        }
    }

    pub(super) fn load_preflight_inner(
        self,
        limits: DecodeLimits,
        meter: Option<&mut SlibClosureDecodeMeterV1>,
    ) -> Result<LoadedSingleConeBuildRequest, SingleConePreflightError> {
        let Self {
            current,
            mut dependencies,
            trusted_core,
            target,
            output,
            diagnostics,
            emit,
        } = self;
        let current = load_current_input(current)?;
        if let TrustedCoreInput::Artifact(input) = trusted_core {
            dependencies.direct.push(input);
        }
        let dependencies = match meter {
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
        Ok(LoadedSingleConeBuildRequest {
            current,
            dependencies,
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
        let (current, dependencies) = match &self.current {
            LoadedCurrentConeInput::Manifest { manifest }
                if manifest.identity() == ConeIdentity::CORE =>
            {
                let dependencies = match meter {
                    Some(meter) => self.dependencies.validate_bootstrap_empty_metered(meter),
                    None => self.dependencies.validate_bootstrap_empty(),
                }
                .map_err(CoreOnlyRequestValidationError::ExplicitDependencies)?;
                (
                    ValidatedCurrentConeInput::TrustedCoreBootstrap { manifest },
                    dependencies,
                )
            }
            LoadedCurrentConeInput::Manifest { manifest } => {
                let (dependencies, trusted_core) = self
                    .dependencies
                    .validate_inner(
                        Some(manifest),
                        manifest.identity(),
                        &self.target,
                        meter.as_deref_mut(),
                    )
                    .map_err(CoreOnlyRequestValidationError::ExplicitDependencies)?;
                (
                    ValidatedCurrentConeInput::Manifest {
                        manifest,
                        trusted_core,
                    },
                    dependencies,
                )
            }
            LoadedCurrentConeInput::SingleFile { source } => {
                let (dependencies, trusted_core) = self
                    .dependencies
                    .validate_inner(None, ConeIdentity::SINGLE_FILE, &self.target, meter)
                    .map_err(CoreOnlyRequestValidationError::ExplicitDependencies)?;
                (
                    ValidatedCurrentConeInput::SingleFile {
                        source,
                        trusted_core,
                    },
                    dependencies,
                )
            }
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
            ValidatedCurrentConeInput::TrustedCoreBootstrap { manifest } => {
                Ok(ParsedSingleConeBuildRequest::TrustedCoreBootstrap(
                    ParsedCoreBootstrapBuildRequest {
                        request: self,
                        sources: parse_manifest_current_metered(manifest, meter)?,
                    },
                ))
            }
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
