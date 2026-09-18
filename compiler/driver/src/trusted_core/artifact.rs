use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use scoop_hir::{
    CoreCallableDefinitionV1, CoreHirCallableCapabilityV1, CoreHirInterfaceBranchV1,
    CoreHirInterfaceV1, CoreInterfaceImportError, ImportedCoreInputs, ImportedCorePreludeTarget,
    SelectedImportedCoreSet, SelectedImportedCoreTarget,
};
use scoop_identity::{
    ConeCoordinate, ConeIdentity, CoreBuiltinNominal, CoreImportedCallableKind, Effect,
    ExactCallableSignature, ExactTypeKey, PersistentExactTypeId, PersistentExportBindingId,
    SemanticIdentitySession,
};
use scoop_lir::{
    ImportedLirCallableProjectionError, ImportedLirSelectionError,
    ImportedLirTypeDescriptorProjectionError, SelectedImportedLirCallable, SelectedImportedLirSet,
    ValidatedLirTargetSelection,
};
use scoop_mir::{
    CoreMirBridgeBranchV1, ImportedMirCallableProjectionError, ImportedMirSelectionError,
    SelectedImportedMirCallable, SelectedImportedMirSet,
};
use scoop_slib::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CompositeIdentityAbiFingerprint, ConeKind,
    ConeSourceForm, CrossConeArtifactClosureValidationError, CrossConeSemanticsStrongProfile,
    DecodedSlibEnvelope, GraphValidationError, PublishableCrossConeArtifact,
    SlibClosureDecodeMeterV1, SlibClosureResourceErrorV1, SlibReadError, ValidatedCompileArtifact,
    ValidatedCompletedCrossConeArtifactClosure, ValidatedGraphArtifact,
    validate_completed_cross_cone_artifact_closure,
};
use scoop_toolchain::ResolvedTargetProfile;
use scoop_wire::{DecodeLimits, sha256};

use super::TrustedCoreArtifactInput;

mod projection;
pub use projection::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedCoreArtifactLoadOperation {
    Open,
    Inspect,
    Read,
}

impl fmt::Display for TrustedCoreArtifactLoadOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Open => "open",
            Self::Inspect => "inspect",
            Self::Read => "read",
        })
    }
}

#[derive(Debug)]
pub enum TrustedCoreArtifactLoadError {
    Io {
        operation: TrustedCoreArtifactLoadOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    NotRegularFile(PathBuf),
    ArtifactTooLarge {
        path: PathBuf,
        actual: u64,
        limit: u64,
    },
    Resource(SlibClosureResourceErrorV1),
}

impl fmt::Display for TrustedCoreArtifactLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "cannot {operation} trusted core artifact {}: {source}",
                path.display()
            ),
            Self::NotRegularFile(path) => write!(
                formatter,
                "trusted core artifact {} is not a regular file",
                path.display()
            ),
            Self::ArtifactTooLarge {
                path,
                actual,
                limit,
            } => write!(
                formatter,
                "trusted core artifact {} has {actual} bytes, exceeding the {limit}-byte input limit",
                path.display()
            ),
            Self::Resource(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for TrustedCoreArtifactLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Resource(source) => Some(source),
            Self::NotRegularFile(_) | Self::ArtifactTooLarge { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct LoadedTrustedCoreArtifact {
    input: TrustedCoreArtifactInput,
    bytes: Vec<u8>,
    limits: DecodeLimits,
}

impl TrustedCoreArtifactInput {
    pub fn load(
        self,
        limits: DecodeLimits,
    ) -> Result<LoadedTrustedCoreArtifact, TrustedCoreArtifactLoadError> {
        let path = self.path.clone();
        let file = File::open(&path).map_err(|source| TrustedCoreArtifactLoadError::Io {
            operation: TrustedCoreArtifactLoadOperation::Open,
            path: path.clone(),
            source,
        })?;
        let metadata = file
            .metadata()
            .map_err(|source| TrustedCoreArtifactLoadError::Io {
                operation: TrustedCoreArtifactLoadOperation::Inspect,
                path: path.clone(),
                source,
            })?;
        if !metadata.is_file() {
            return Err(TrustedCoreArtifactLoadError::NotRegularFile(path));
        }
        require_input_size(&path, metadata.len(), limits.owned_bytes)?;

        let mut bytes = Vec::new();
        file.take(limits.owned_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| TrustedCoreArtifactLoadError::Io {
                operation: TrustedCoreArtifactLoadOperation::Read,
                path: path.clone(),
                source,
            })?;
        let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        require_input_size(&path, actual, limits.owned_bytes)?;
        Ok(LoadedTrustedCoreArtifact {
            input: self,
            bytes,
            limits,
        })
    }

    pub(crate) fn load_metered(
        self,
        limits: DecodeLimits,
        meter: &mut SlibClosureDecodeMeterV1,
    ) -> Result<LoadedTrustedCoreArtifact, TrustedCoreArtifactLoadError> {
        let loaded = self.load(limits)?;
        let byte_length = u64::try_from(loaded.bytes.len()).unwrap_or(u64::MAX);
        meter
            .observe_raw_artifact_snapshot(sha256(&loaded.bytes), byte_length)
            .map_err(TrustedCoreArtifactLoadError::Resource)?;
        Ok(loaded)
    }
}

fn require_input_size(
    path: &Path,
    actual: u64,
    limit: u64,
) -> Result<(), TrustedCoreArtifactLoadError> {
    if actual > limit {
        Err(TrustedCoreArtifactLoadError::ArtifactTooLarge {
            path: path.to_path_buf(),
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

impl LoadedTrustedCoreArtifact {
    pub fn path(&self) -> &Path {
        self.input.path()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) const fn limits(&self) -> DecodeLimits {
        self.limits
    }

    pub fn validate<'input>(
        &'input self,
        target: &ResolvedTargetProfile,
    ) -> Result<ValidatedTrustedCoreArtifact<'input>, TrustedCoreArtifactValidationError> {
        self.validate_against(
            target.lir_target_selection(),
            target.c_bridge_toolchain().profile(),
        )
    }

    fn validate_against<'input>(
        &'input self,
        target_selection: ValidatedLirTargetSelection,
        c_bridge_profile: &scoop_lir::CBridgeToolchainProfileV1,
    ) -> Result<ValidatedTrustedCoreArtifact<'input>, TrustedCoreArtifactValidationError> {
        if self.input.target != target_selection {
            return Err(TrustedCoreArtifactValidationError::authority(
                TrustedCoreArtifactAuthorityError::TargetSelection {
                    expected: self.input.target,
                    actual: target_selection,
                },
            ));
        }

        let authority_graph = DecodedSlibEnvelope::open(&self.bytes, self.limits, target_selection)
            .map_err(|source| TrustedCoreArtifactValidationError::Envelope(Box::new(source)))?
            .validate_graph()
            .map_err(|source| TrustedCoreArtifactValidationError::Graph(Box::new(source)))?;
        validate_graph_authority(&authority_graph, &self.input)?;

        let mut semantic_session = SemanticIdentitySession::new();
        let closure = validate_completed_cross_cone_artifact_closure(
            ConeIdentity::CORE,
            target_selection,
            Vec::new(),
            Vec::new(),
            &self.bytes,
            self.limits,
            c_bridge_profile,
            &mut semantic_session,
        )
        .map_err(|source| TrustedCoreArtifactValidationError::Closure(Box::new(source)))?;
        let compile = closure.current_compile();
        let interface = match compile.production().hir_core().core_interface() {
            CoreHirInterfaceBranchV1::Core(interface) => interface.as_ref().clone(),
            CoreHirInterfaceBranchV1::NotCore => {
                return Err(TrustedCoreArtifactValidationError::MissingCoreInterface);
            }
        };
        let strong_callable_bindings = match compile.production().mir_core().core_bridge() {
            CoreMirBridgeBranchV1::Core(bridge) => bridge
                .callable_targets()
                .iter()
                .map(|target| target.binding())
                .collect(),
            CoreMirBridgeBranchV1::NotCore => {
                return Err(TrustedCoreArtifactValidationError::MissingCoreMirBridge);
            }
        };
        let core_interface = ValidatedCoreInterface {
            interface,
            strong_callable_bindings,
        };

        Ok(ValidatedTrustedCoreArtifact {
            closure,
            authority: TrustedCoreArtifactAuthority::from_input(&self.input),
            core_interface,
            _semantic_session: semantic_session,
        })
    }
}

fn validate_graph_authority(
    artifact: &ValidatedGraphArtifact<'_>,
    input: &TrustedCoreArtifactInput,
) -> Result<(), TrustedCoreArtifactValidationError> {
    if artifact.coordinate() != &input.expected_coordinate {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::Coordinate {
                expected: input.expected_coordinate.clone(),
                actual: artifact.coordinate().clone(),
            },
        ));
    }
    if artifact.identity() != ConeIdentity::CORE {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::Identity {
                actual: artifact.identity(),
            },
        ));
    }
    if artifact.kind() != ConeKind::Library {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::Kind {
                actual: artifact.kind(),
            },
        ));
    }
    if artifact.source_form() != ConeSourceForm::Manifest {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::SourceForm {
                actual: artifact.source_form(),
            },
        ));
    }
    if !artifact.direct_dependencies().is_empty() {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::Dependencies {
                actual: artifact.direct_dependencies().len(),
            },
        ));
    }
    let actual_abi = artifact.compatibility().composite_identity_abi();
    if actual_abi != input.toolchain_compatibility {
        return Err(TrustedCoreArtifactValidationError::authority(
            TrustedCoreArtifactAuthorityError::ToolchainCompatibility {
                expected: input.toolchain_compatibility,
                actual: actual_abi,
            },
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedCoreArtifactAuthority {
    path: PathBuf,
    coordinate: ConeCoordinate,
    target: ValidatedLirTargetSelection,
    toolchain_compatibility: CompositeIdentityAbiFingerprint,
}

impl TrustedCoreArtifactAuthority {
    fn from_input(input: &TrustedCoreArtifactInput) -> Self {
        Self {
            path: input.path.clone(),
            coordinate: input.expected_coordinate.clone(),
            target: input.target,
            toolchain_compatibility: input.toolchain_compatibility,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub const fn target(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub const fn toolchain_compatibility(&self) -> CompositeIdentityAbiFingerprint {
        self.toolchain_compatibility
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidatedCoreInterface {
    interface: CoreHirInterfaceV1,
    strong_callable_bindings: Vec<PersistentExportBindingId>,
}

pub struct ValidatedTrustedCoreArtifact<'input> {
    closure: ValidatedCompletedCrossConeArtifactClosure<'input>,
    authority: TrustedCoreArtifactAuthority,
    core_interface: ValidatedCoreInterface,
    // Retained as the owner of the session-local identity world. It is not a
    // lookup surface and deliberately has no projection getter.
    _semantic_session: SemanticIdentitySession,
}

impl<'input> ValidatedTrustedCoreArtifact<'input> {
    fn compile(&self) -> &ValidatedCompileArtifact<'input, CrossConeSemanticsStrongProfile> {
        self.closure.current_compile()
    }

    pub const fn authority(&self) -> &TrustedCoreArtifactAuthority {
        &self.authority
    }

    pub fn dependency_record(&self) -> scoop_slib::DependencyRecord {
        self.compile().dependency_record()
    }

    /// Atomically projects the only HIR lookup and compiler-protocol
    /// capabilities authorized for an M23-3 consumer from this artifact's
    /// own Compile proof and core interface.
    pub fn import_core_inputs(&self) -> Result<ImportedCoreInputs<'_>, CoreInterfaceImportError> {
        self.compile().hir().import_core_inputs(
            &self.core_interface.interface,
            &self.core_interface.strong_callable_bindings,
        )
    }

    pub fn publication(&self) -> &PublishableCrossConeArtifact {
        self.closure.current_publication()
    }

    pub fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        self.closure.current_link().defined_symbols()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrustedCoreArtifactAuthorityError {
    Coordinate {
        expected: ConeCoordinate,
        actual: ConeCoordinate,
    },
    Identity {
        actual: ConeIdentity,
    },
    Kind {
        actual: ConeKind,
    },
    SourceForm {
        actual: ConeSourceForm,
    },
    Dependencies {
        actual: usize,
    },
    TargetSelection {
        expected: ValidatedLirTargetSelection,
        actual: ValidatedLirTargetSelection,
    },
    ToolchainCompatibility {
        expected: CompositeIdentityAbiFingerprint,
        actual: CompositeIdentityAbiFingerprint,
    },
}

impl fmt::Display for TrustedCoreArtifactAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinate { expected, actual } => write!(
                formatter,
                "trusted core coordinate mismatch: expected {expected}, found {actual}"
            ),
            Self::Identity { actual } => write!(
                formatter,
                "trusted core identity mismatch: expected {}, found {actual}",
                ConeIdentity::CORE
            ),
            Self::Kind { actual } => write!(
                formatter,
                "trusted core must be a library, found {actual:?}"
            ),
            Self::SourceForm { actual } => write!(
                formatter,
                "trusted core must use Manifest source form, found {actual:?}"
            ),
            Self::Dependencies { actual } => write!(
                formatter,
                "trusted core must have no direct dependencies, found {actual}"
            ),
            Self::TargetSelection { expected, actual } => write!(
                formatter,
                "trusted core target mismatch: expected {expected:?}, requested {actual:?}"
            ),
            Self::ToolchainCompatibility { expected, actual } => write!(
                formatter,
                "trusted core identity ABI mismatch: expected {expected}, found {actual}"
            ),
        }
    }
}

impl std::error::Error for TrustedCoreArtifactAuthorityError {}

#[derive(Debug)]
pub enum TrustedCoreArtifactValidationError {
    Authority(Box<TrustedCoreArtifactAuthorityError>),
    Envelope(Box<SlibReadError>),
    Graph(Box<GraphValidationError>),
    Closure(Box<CrossConeArtifactClosureValidationError>),
    MissingCoreInterface,
    MissingCoreMirBridge,
}

impl TrustedCoreArtifactValidationError {
    fn authority(source: TrustedCoreArtifactAuthorityError) -> Self {
        Self::Authority(Box::new(source))
    }
}

impl fmt::Display for TrustedCoreArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authority(error) => error.fmt(formatter),
            Self::Envelope(error) => {
                write!(
                    formatter,
                    "trusted core envelope validation failed: {error}"
                )
            }
            Self::Graph(error) => {
                write!(formatter, "trusted core graph validation failed: {error}")
            }
            Self::Closure(error) => {
                write!(formatter, "trusted core closure validation failed: {error}")
            }
            Self::MissingCoreInterface => {
                formatter.write_str("trusted core Compile proof has no Core HIR interface")
            }
            Self::MissingCoreMirBridge => {
                formatter.write_str("trusted core Compile proof has no Core MIR bridge")
            }
        }
    }
}

impl std::error::Error for TrustedCoreArtifactValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Authority(error) => Some(error),
            Self::Envelope(error) => Some(error.as_ref()),
            Self::Graph(error) => Some(error.as_ref()),
            Self::Closure(error) => Some(error.as_ref()),
            Self::MissingCoreInterface | Self::MissingCoreMirBridge => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_hir::CanonicalHirFoundation;
    use scoop_identity::ConeCoordinate;
    use scoop_lir::{
        AppleClangCompilerIdentityV1, CBridgeToolchainProfileV1, CanonicalLirFoundation,
        DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1,
    };
    use scoop_mir::CanonicalMirFoundation;
    use scoop_slib::{
        ConeRecord, IdentityFoundationArtifact, IdentityFoundationArtifactInput, ProducerRecord,
    };

    use super::*;

    #[test]
    fn loader_binds_the_exact_opened_bytes_and_budget() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scoop.core.slib");
        std::fs::write(&path, b"trusted bytes").unwrap();
        let input = TrustedCoreArtifactInput::for_test(path.clone());

        let loaded = input.load(DecodeLimits::default()).unwrap();

        assert_eq!(loaded.path(), path);
        assert_eq!(loaded.bytes(), b"trusted bytes");
    }

    #[test]
    fn loader_rejects_an_artifact_larger_than_the_owned_input_budget() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scoop.core.slib");
        std::fs::write(&path, b"oversized").unwrap();
        let limits = DecodeLimits {
            owned_bytes: 4,
            ..DecodeLimits::default()
        };

        assert!(matches!(
            TrustedCoreArtifactInput::for_test(path.clone()).load(limits),
            Err(TrustedCoreArtifactLoadError::ArtifactTooLarge {
                path: actual_path,
                actual: 9,
                limit: 4,
            }) if actual_path == path
        ));
    }

    #[test]
    fn validation_rejects_a_non_core_artifact_before_profile_decoding() {
        let coordinate = ConeCoordinate::new("test", "ordinary", "0.0.0").unwrap();
        let bytes = foundation_artifact(
            ConeRecord::new(
                coordinate.clone(),
                ConeKind::Library,
                ConeSourceForm::Manifest,
            )
            .unwrap(),
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scoop.core.slib");
        std::fs::write(&path, bytes).unwrap();
        let loaded = TrustedCoreArtifactInput::for_test(path)
            .load(DecodeLimits::default())
            .unwrap();

        assert!(matches!(
            loaded.validate_against(selection(), &c_bridge_profile()),
            Err(TrustedCoreArtifactValidationError::Authority(error))
                if matches!(error.as_ref(),
                    TrustedCoreArtifactAuthorityError::Coordinate { actual, .. }
                        if actual == &coordinate)
        ));
    }

    #[test]
    fn validation_requires_the_cross_cone_strong_profile_after_core_authority() {
        let bytes = foundation_artifact(
            ConeRecord::new(
                ConeCoordinate::reserved_core(),
                ConeKind::Library,
                ConeSourceForm::Manifest,
            )
            .unwrap(),
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scoop.core.slib");
        std::fs::write(&path, bytes).unwrap();
        let loaded = TrustedCoreArtifactInput::for_test(path)
            .load(DecodeLimits::default())
            .unwrap();

        assert!(matches!(
            loaded.validate_against(selection(), &c_bridge_profile()),
            Err(TrustedCoreArtifactValidationError::Closure(_))
        ));
    }

    fn foundation_artifact(cone: ConeRecord) -> Vec<u8> {
        IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
            ProducerRecord::new("trusted-core-test").unwrap(),
            cone,
            selection(),
            &CanonicalHirFoundation::empty(),
            &CanonicalMirFoundation::empty(),
            &CanonicalLirFoundation::empty(),
        ))
        .unwrap()
        .as_bytes()
        .to_vec()
    }

    fn selection() -> ValidatedLirTargetSelection {
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
    }

    fn c_bridge_profile() -> CBridgeToolchainProfileV1 {
        CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
            DarwinCBridgeDeploymentContractV1::new(
                DarwinPackedVersionV1::new(0x000d_0100).unwrap(),
                DarwinPackedVersionV1::new(0x000e_0200).unwrap(),
                Vec::new(),
            )
            .unwrap(),
            AppleClangCompilerIdentityV1::new(21, 0, 0, "clang-2100.1.1.101").unwrap(),
        )
        .unwrap()
    }
}
