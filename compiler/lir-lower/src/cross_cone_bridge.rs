//! Producer-side projection of the M23-5 cross-Cone LIR semantic bridge.

use std::fmt;

use scoop_identity::{DependencyCallableDeclarationId, StrongCallableDefinitionOwner};
use scoop_lir as lir;
use scoop_mir as mir;

/// Projects the MIR bridge through one sealed MIR/LIR lowering pair.
///
/// Local exports are rebuilt from the exact MIR materialization root and its
/// lowered physical signature. Consumer selections are rebuilt from the
/// typed dependency-external arena retained by LIR. Neither side is inferred
/// from symbol text.
pub fn lower_cross_cone_bridge_section(
    input: &mir::SingleConeStrongMirInput,
    mir_bridge: &mir::CrossConeMirBridgeSectionV1,
    output: &lir::SingleConeStrongLirOutput,
) -> Result<lir::CrossConeLirBridgeSectionV1, CrossConeLirBridgeLoweringError> {
    validate_authority(input, mir_bridge, output)?;
    validate_mir_selections(input, mir_bridge)?;
    let exports = lower_exports(input, mir_bridge, output)?;
    let selected = lower_selected(mir_bridge, output)?;
    lir::CrossConeLirBridgeSectionV1::try_new(output.foundation(), exports, selected)
        .map_err(|source| CrossConeLirBridgeLoweringError::Bridge(Box::new(source)))
}

fn validate_authority(
    input: &mir::SingleConeStrongMirInput,
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
    input: &mir::SingleConeStrongMirInput,
    bridge: &mir::CrossConeMirBridgeSectionV1,
) -> Result<(), CrossConeLirBridgeLoweringError> {
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(
            input
                .materialization()
                .imported_dependency_callable_roots()
                .len(),
        )
        .map_err(|_| CrossConeLirBridgeLoweringError::Allocation {
            requested_slots: input
                .materialization()
                .imported_dependency_callable_roots()
                .len(),
        })?;
    roots.extend(input.materialization().imported_dependency_callable_roots());
    roots.sort_unstable_by_key(|root| (root.provider(), root.declaration()));

    if roots.len() != bridge.selected().len() {
        return Err(CrossConeLirBridgeLoweringError::MirSelectionCountMismatch {
            bridge: bridge.selected().len(),
            roots: roots.len(),
        });
    }
    for (index, (selected, root)) in bridge.selected().iter().zip(roots).enumerate() {
        if selected.provider() != root.provider()
            || selected.declaration() != root.declaration()
            || selected.implementation() != root.implementation()
            || selected.signature() != root.signature()
        {
            return Err(CrossConeLirBridgeLoweringError::MirSelectionMismatch { index });
        }
    }
    Ok(())
}

fn lower_exports(
    input: &mir::SingleConeStrongMirInput,
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
    input: &mir::SingleConeStrongMirInput,
    output: &lir::SingleConeStrongLirOutput,
    export: &mir::ParamFreeMirCallableExportV1,
) -> Result<lir::ParamFreeLirCallableExportV1, CrossConeLirBridgeLoweringError> {
    let declaration = export.declaration();
    let implementation = export.implementation();
    let callable_owner = implementation.callable_owner();
    let strong = input
        .production()
        .strong_callable_bridges()
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == callable_owner)
        .ok_or(CrossConeLirBridgeLoweringError::MissingMirSignature { declaration })?;
    if strong.signature() != export.signature() {
        return Err(CrossConeLirBridgeLoweringError::MirSignatureMismatch { declaration });
    }
    let root = input
        .materialization()
        .callable_roots()
        .iter()
        .find(|root| root.implementation() == callable_owner)
        .ok_or(CrossConeLirBridgeLoweringError::MissingMirMaterialization { declaration })?;
    let mir_function = &input.module().functions[root.function()];
    let body = callable_body_identity(implementation)
        .map_err(
            |source| CrossConeLirBridgeLoweringError::CallableBodyIdentity {
                declaration,
                source: Box::new(source),
            },
        )?
        .id();
    let lir_function = output
        .module()
        .functions
        .iter()
        .find(|function| function.callable_body.id() == body)
        .ok_or(CrossConeLirBridgeLoweringError::MissingLirBody { declaration })?;
    let gc_effect_matches = matches!(
        (mir_function.gc_effect, lir_function.gc_effect),
        (mir::GcEffect::Managed, lir::GcEffect::Managed)
            | (mir::GcEffect::NoGc, lir::GcEffect::NoGc)
    );
    if !gc_effect_matches {
        return Err(CrossConeLirBridgeLoweringError::GcEffectMismatch { declaration });
    }
    if lir_function.signature.calling_convention() != lir::CallingConvention::Cdecl {
        return Err(CrossConeLirBridgeLoweringError::CallingConventionMismatch { declaration });
    }
    if mir_function.params.len() != lir_function.signature.arguments().len() {
        return Err(CrossConeLirBridgeLoweringError::ArgumentCountMismatch {
            declaration,
            mir: mir_function.params.len(),
            lir: lir_function.signature.arguments().len(),
        });
    }

    let abi_signature = crate::native_abi::canonical_scoop_signature(
        input.module(),
        &output.module().enums,
        export.signature().clone(),
        &mir_function
            .params
            .iter()
            .map(|parameter| parameter.ty.clone())
            .collect::<Vec<_>>(),
        &mir_function.return_ty,
        mir_function.gc_effect,
        &lir_function.signature,
    );
    let root_plan = match lir_function.gc_effect {
        lir::GcEffect::Managed => lir::DependencyExternalCallableRootPlanV1::ManagedStatepoint,
        lir::GcEffect::NoGc => lir::DependencyExternalCallableRootPlanV1::NoGc,
    };
    lir::ParamFreeLirCallableExportV1::new(
        input.module().cone,
        declaration,
        implementation,
        abi_signature,
        lir_function.signature.calling_convention(),
        root_plan,
    )
    .map_err(|source| CrossConeLirBridgeLoweringError::Export {
        declaration,
        source: Box::new(source),
    })
}

fn callable_body_identity(
    owner: StrongCallableDefinitionOwner,
) -> Result<lir::CallableBodyIdentity, lir::CallableBodyIdentityBuildError> {
    match owner {
        StrongCallableDefinitionOwner::Function(id) => lir::CallableBodyIdentity::for_function(id),
        StrongCallableDefinitionOwner::Constructor(id) => {
            lir::CallableBodyIdentity::for_constructor(id)
        }
        StrongCallableDefinitionOwner::PropertyAccessor(id) => {
            lir::CallableBodyIdentity::for_property_accessor(id)
        }
        StrongCallableDefinitionOwner::GeneratedCallable(id) => {
            lir::CallableBodyIdentity::for_generated_callable(id)
        }
    }
}

fn lower_selected(
    bridge: &mir::CrossConeMirBridgeSectionV1,
    output: &lir::SingleConeStrongLirOutput,
) -> Result<Vec<lir::SelectedDependencyLirCallableV1>, CrossConeLirBridgeLoweringError> {
    let mut external = Vec::new();
    external
        .try_reserve_exact(output.module().meta.dependency_external_callables.len())
        .map_err(|_| CrossConeLirBridgeLoweringError::Allocation {
            requested_slots: output.module().meta.dependency_external_callables.len(),
        })?;
    external.extend(
        output
            .module()
            .meta
            .dependency_external_callables
            .iter()
            .map(|(_, callable)| callable),
    );
    external.sort_unstable_by_key(|callable| {
        (
            callable.provider(),
            callable
                .legacy_declaration()
                .expect("legacy bridge materialization retains its declaration"),
        )
    });

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
    for (index, (expected, actual)) in bridge.selected().iter().zip(external).enumerate() {
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
                actual
                    .legacy_declaration()
                    .expect("legacy bridge materialization retains its declaration"),
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
    MirSelectionCountMismatch {
        bridge: usize,
        roots: usize,
    },
    MirSelectionMismatch {
        index: usize,
    },
    MissingMirSignature {
        declaration: DependencyCallableDeclarationId,
    },
    MirSignatureMismatch {
        declaration: DependencyCallableDeclarationId,
    },
    MissingMirMaterialization {
        declaration: DependencyCallableDeclarationId,
    },
    CallableBodyIdentity {
        declaration: DependencyCallableDeclarationId,
        source: Box<lir::CallableBodyIdentityBuildError>,
    },
    MissingLirBody {
        declaration: DependencyCallableDeclarationId,
    },
    GcEffectMismatch {
        declaration: DependencyCallableDeclarationId,
    },
    CallingConventionMismatch {
        declaration: DependencyCallableDeclarationId,
    },
    ArgumentCountMismatch {
        declaration: DependencyCallableDeclarationId,
        mir: usize,
        lir: usize,
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
            Self::CallableBodyIdentity { source, .. } => Some(source.as_ref()),
            Self::Export { source, .. } | Self::Selected { source, .. } => Some(source.as_ref()),
            Self::Bridge(source) => Some(source.as_ref()),
            Self::MirArtifactMismatch { .. }
            | Self::LirArtifactMismatch { .. }
            | Self::Allocation { .. }
            | Self::MirSelectionCountMismatch { .. }
            | Self::MirSelectionMismatch { .. }
            | Self::MissingMirSignature { .. }
            | Self::MirSignatureMismatch { .. }
            | Self::MissingMirMaterialization { .. }
            | Self::MissingLirBody { .. }
            | Self::GcEffectMismatch { .. }
            | Self::CallingConventionMismatch { .. }
            | Self::ArgumentCountMismatch { .. }
            | Self::LirSelectionCountMismatch { .. }
            | Self::LirSelectionMismatch { .. } => None,
        }
    }
}
