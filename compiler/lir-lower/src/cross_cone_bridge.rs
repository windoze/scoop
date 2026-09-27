//! Producer-side projection of the M23-5 cross-Cone LIR semantic bridge.

use std::fmt;

use scoop_identity::DependencyCallableDeclarationId;
use scoop_lir as lir;
use scoop_mir as mir;

/// Projects the MIR bridge through one sealed MIR/LIR lowering pair.
///
/// Local exports are rebuilt from the exact MIR materialization root and its
/// lowered physical signature. Consumer selections are rebuilt from the
/// typed dependency-external arena retained by LIR. Neither side is inferred
/// from symbol text.
pub fn lower_cross_cone_bridge_section(
    input: &mir::ConeMirInput,
    mir_bridge: &mir::CrossConeMirBridgeSectionV1,
    output: &lir::SingleConeStrongLirOutput,
) -> Result<lir::CrossConeLirBridgeSectionV1, CrossConeLirBridgeLoweringError> {
    validate_providers(input, mir_bridge, output)?;
    validate_mir_selections(input, mir_bridge)?;
    let exports = lower_exports(input, mir_bridge, output)?;
    let selected = lower_selected(mir_bridge, output)?;
    lir::CrossConeLirBridgeSectionV1::try_new(output.foundation(), exports, selected)
        .map_err(|source| CrossConeLirBridgeLoweringError::Bridge(Box::new(source)))
}

fn validate_providers(
    input: &mir::ConeMirInput,
    bridge: &mir::CrossConeMirBridgeSectionV1,
    output: &lir::SingleConeStrongLirOutput,
) -> Result<(), CrossConeLirBridgeLoweringError> {
    let expected = input.module().cone;
    if bridge.artifact() != expected {
        return Err(CrossConeLirBridgeLoweringError::MirArtifactMismatch {
            expected,
            actual: bridge.artifact(),
        });
    }
    if output.foundation().producer() != expected {
        return Err(CrossConeLirBridgeLoweringError::LirArtifactMismatch {
            expected,
            actual: output.foundation().producer(),
        });
    }
    Ok(())
}

fn validate_mir_selections(
    input: &mir::ConeMirInput,
    bridge: &mir::CrossConeMirBridgeSectionV1,
) -> Result<(), CrossConeLirBridgeLoweringError> {
    let roots = input.materialization().external_callable_roots();
    for (index, selected) in bridge.selected().iter().enumerate() {
        let root = roots
            .iter()
            .find(|root| {
                root.provider() == selected.provider()
                    && root.implementation() == selected.implementation()
            })
            .ok_or(CrossConeLirBridgeLoweringError::MirSelectionMismatch { index })?;
        if selected.signature() != root.signature() {
            return Err(CrossConeLirBridgeLoweringError::MirSelectionMismatch { index });
        }
    }
    Ok(())
}

fn lower_exports(
    input: &mir::ConeMirInput,
    bridge: &mir::CrossConeMirBridgeSectionV1,
    output: &lir::SingleConeStrongLirOutput,
) -> Result<Vec<lir::ParamFreeLirCallableExportV1>, CrossConeLirBridgeLoweringError> {
    let mut exports = Vec::new();
    exports
        .try_reserve_exact(bridge.exports().len())
        .map_err(|_| CrossConeLirBridgeLoweringError::Allocation {
            requested_slots: bridge.exports().len(),
        })?;
    for export in bridge.exports() {
        exports.push(lower_export(input, output, export)?);
    }
    Ok(exports)
}

fn lower_export(
    input: &mir::ConeMirInput,
    output: &lir::SingleConeStrongLirOutput,
    export: &mir::ParamFreeMirCallableExportV1,
) -> Result<lir::ParamFreeLirCallableExportV1, CrossConeLirBridgeLoweringError> {
    let declaration = export.declaration();
    let implementation = export.implementation();
    let callable = crate::callable_abi::LocalCallableMaterialization::resolve(
        input,
        &output.module().functions,
        implementation,
        export.signature(),
    )
    .and_then(|body| body.abi_record(&output.module().enums))
    .map_err(|source| CrossConeLirBridgeLoweringError::CallableAbi {
        declaration,
        source,
    })?;
    lir::ParamFreeLirCallableExportV1::from_abi(declaration, callable).map_err(|source| {
        CrossConeLirBridgeLoweringError::Export {
            declaration,
            source: Box::new(source),
        }
    })
}

fn lower_selected(
    bridge: &mir::CrossConeMirBridgeSectionV1,
    output: &lir::SingleConeStrongLirOutput,
) -> Result<Vec<lir::SelectedDependencyLirCallableV1>, CrossConeLirBridgeLoweringError> {
    let mut external = Vec::new();
    external
        .try_reserve_exact(output.module().meta.external_callables.len())
        .map_err(|_| CrossConeLirBridgeLoweringError::Allocation {
            requested_slots: output.module().meta.external_callables.len(),
        })?;
    external.extend(
        output
            .module()
            .meta
            .external_callables
            .iter()
            .filter_map(|(_, callable)| {
                callable
                    .legacy_declaration()
                    .map(|declaration| (callable, declaration))
            }),
    );
    external.sort_unstable_by_key(|(callable, declaration)| (callable.provider(), *declaration));

    if external.len() != bridge.selected().len() {
        return Err(CrossConeLirBridgeLoweringError::LirSelectionCountMismatch {
            mir: bridge.selected().len(),
            lir: external.len(),
        });
    }
    let mut selected = Vec::new();
    selected.try_reserve_exact(external.len()).map_err(|_| {
        CrossConeLirBridgeLoweringError::Allocation {
            requested_slots: external.len(),
        }
    })?;
    for (index, (expected, (actual, declaration))) in
        bridge.selected().iter().zip(external).enumerate()
    {
        if expected.provider() != actual.provider()
            || Some(expected.declaration()) != actual.legacy_declaration()
            || expected.implementation() != actual.target()
            || expected.signature() != actual.canonical_signature().signature()
        {
            return Err(CrossConeLirBridgeLoweringError::LirSelectionMismatch { index });
        }
        selected.push(
            lir::SelectedDependencyLirCallableV1::new(
                actual.provider(),
                declaration,
                actual.target(),
                actual.canonical_signature().clone(),
                actual.calling_convention(),
                actual.root_plan(),
            )
            .map_err(|source| CrossConeLirBridgeLoweringError::Selected {
                index,
                source: Box::new(source),
            })?,
        );
    }
    Ok(selected)
}

#[derive(Debug)]
pub enum CrossConeLirBridgeLoweringError {
    MirArtifactMismatch {
        expected: lir::ConeIdentity,
        actual: lir::ConeIdentity,
    },
    LirArtifactMismatch {
        expected: lir::ConeIdentity,
        actual: lir::ConeIdentity,
    },
    Allocation {
        requested_slots: usize,
    },
    MirSelectionMismatch {
        index: usize,
    },
    CallableAbi {
        declaration: DependencyCallableDeclarationId,
        source: crate::CallableAbiProjectionError,
    },
    Export {
        declaration: DependencyCallableDeclarationId,
        source: Box<lir::ParamFreeLirCallableBuildError>,
    },
    LirSelectionCountMismatch {
        mir: usize,
        lir: usize,
    },
    LirSelectionMismatch {
        index: usize,
    },
    Selected {
        index: usize,
        source: Box<lir::ParamFreeLirCallableBuildError>,
    },
    Bridge(Box<lir::CrossConeLirBridgeBuildError>),
}

impl fmt::Display for CrossConeLirBridgeLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot lower cross-Cone LIR bridge: {self:?}")
    }
}

impl std::error::Error for CrossConeLirBridgeLoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CallableAbi { source, .. } => Some(source),
            Self::Export { source, .. } | Self::Selected { source, .. } => Some(source.as_ref()),
            Self::Bridge(source) => Some(source.as_ref()),
            Self::MirArtifactMismatch { .. }
            | Self::LirArtifactMismatch { .. }
            | Self::Allocation { .. }
            | Self::MirSelectionMismatch { .. }
            | Self::LirSelectionCountMismatch { .. }
            | Self::LirSelectionMismatch { .. } => None,
        }
    }
}
