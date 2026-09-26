//! Complete source, type, layout, and object products for the normal CLI path.

use super::*;
use scoop_hir as hir;
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_slib as slib;

mod errors;
mod imports;
use LayoutProductionError as Error;
pub use errors::LayoutProductionError;

pub(super) fn assemble(
    hir: current_hir::CurrentConeHirArtifacts,
    request: &ValidatedSingleConeBuildRequest<'_>,
    cone: slib::ConeRecord,
    temporary_parent: &Path,
    selected: mir::SelectedExternalMirSet,
    dump: &mut Option<EmittedStageDump>,
) -> Result<slib::AssembledCrossConeLayoutStrongArtifactV1, CurrentConeProductionFailure> {
    let closure = request.dependencies().semantic();
    let dependencies = closure.layout_dependencies().collect::<Vec<_>>();
    let mir = hir
        .machine_input()
        .lower_selected_mir(selected)
        .map_err(CurrentConeProductionFailure::Mir)?;
    if dump.is_none() {
        *dump = capture_stage_dump(request.emit(), StageDumpKind::Mir, || {
            scoop_mir::dump(mir.strong.module())
        });
    }
    let selected = closure
        .project_dependency_callables_to_lir(&mir.selected_callables)
        .map_err(CurrentConeLirStageError::DependencyProjection)
        .map_err(CurrentConeProductionFailure::Lir)?;
    let (identities, coordinates) =
        protocols::type_identities(&hir, &mir, closure, cone.coordinate())
            .map_err(CurrentConeLirStageError::Identity)
            .map_err(CurrentConeProductionFailure::Lir)?;
    let foundation = hir::OdrFreeHirFoundation::try_new(hir.foundation)
        .map_err(CurrentConeStrongProfileError::HirOdr)
        .map_err(CurrentConeProductionFailure::StrongProfile)?;
    let source = scoop_hir_lower::produce_cross_cone_type_semantics(
        &hir.hir,
        hir::SharedTypeMetadataV1 {
            provider: cone.identity(),
            identities: &identities,
            foundation: &foundation,
            public: &hir.cross_cone_section,
        },
        &dependencies
            .iter()
            .map(|dependency| hir::SharedTypeMetadataV1 {
                provider: dependency.identity(),
                identities: dependency.identity_graph(),
                foundation: dependency.hir_foundation(),
                public: dependency.hir_interface(),
            })
            .collect::<Vec<_>>(),
    )
    .map_err(Error::HirTypes)?;
    let mir_input = scoop_mir_lower::MirTypeBridgeExportInputV1 {
        hir: &hir.hir,
        public: &hir.cross_cone_section,
        source: &source,
        mir: &mir.strong,
        ordinary: &mir.public,
        identities: &identities,
    };
    let mir_exports = scoop_mir_lower::lower_type_bridge_exports(
        mir_input,
        scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
            types: &dependencies
                .iter()
                .map(|d| d.mir_type_bridge().exports().types())
                .collect::<Vec<_>>(),
            callables: &dependencies
                .iter()
                .map(|d| d.mir_type_bridge().exports().callables())
                .collect::<Vec<_>>(),
            direct_callables: &dependencies
                .iter()
                .map(|d| d.mir_cross_cone_bridge())
                .collect::<Vec<_>>(),
            dispatch: &dependencies
                .iter()
                .map(|d| d.mir_type_bridge().exports().dispatch())
                .collect::<Vec<_>>(),
        },
    )
    .map_err(Error::MirExports)?;
    let mir_uses =
        scoop_mir_lower::lower_type_bridge_dependencies(mir_input).map_err(Error::MirUses)?;
    let units = scoop_mir_lower::lower_type_bridge_initialization_units(&mir.strong)
        .map_err(Error::MirUses)?;
    let mir_section = mir::CrossConeMirTypeBridgeSectionV1::try_new(
        mir::MirTypeBridgeLocalInputV1 {
            provider: cone.identity(),
            production: mir.strong.production(),
            ordinary: &mir.public,
        },
        mir_exports,
        units,
        &dependencies
            .iter()
            .map(|d| {
                d.mir_type_bridge()
                    .dependency_view(d.initialization_units())
            })
            .collect::<Vec<_>>(),
        &mir_uses,
        &identities,
    )
    .map_err(Error::MirSection)?;
    let selected_layout = imports::select(
        &mir.strong,
        &mir_section,
        &dependencies,
        request.target().lir_target(),
    )?;
    let initialization = imports::initialization(&mir_section, &dependencies, &selected_layout)?;
    let diagnostics =
        scoop_identity::ExactTypeDiagnosticCatalog::try_new(&identities, &coordinates)
            .map_err(CurrentConeLirStageError::DiagnosticCatalog)
            .map_err(CurrentConeProductionFailure::Lir)?;
    let (lir, lir_public) = machine::lower_selected_lir(
        &mir.strong,
        &mir.public,
        &selected,
        request.target().lir_target(),
        &selected_layout,
        &diagnostics,
    )
    .map_err(CurrentConeProductionFailure::Lir)?;
    if dump.is_none() {
        *dump = capture_stage_dump(request.emit(), StageDumpKind::Lir, || {
            scoop_lir::dump(lir.module())
        });
    }
    let mut pending = scoop_identity::PendingIdentityValidation::from_graph(identities);
    lir.foundation()
        .as_canonical()
        .register_identities(&mut pending)
        .map_err(CurrentConeLirStageError::Identity)
        .map_err(CurrentConeProductionFailure::Lir)?;
    let identities = pending
        .finish()
        .map_err(CurrentConeLirStageError::Identity)
        .map_err(CurrentConeProductionFailure::Lir)?;
    let direct = request
        .dependencies()
        .direct_dependencies()
        .iter()
        .map(slib::DependencyRecord::identity)
        .collect::<Vec<_>>();
    let registration = lir
        .build_production_section_v2(
            cone.coordinate().clone(),
            &direct,
            scoop_lir_lower::lower_entry_production_source(mir.strong.production().entry_bridge()),
            &initialization,
        )
        .map_err(|error| Error::Registration(Box::new(error)))?;
    let input = scoop_lir_lower::LayoutAbiExportInputV1 {
        mir: &mir.strong,
        lir: &lir,
        bridge: mir_section.exports(),
        ordinary: &lir_public,
        registration: &registration,
        identities: &identities,
        coordinates: &coordinates,
    };
    let dependency_layouts = dependencies
        .iter()
        .map(|d| d.lir_exports().layouts())
        .collect::<Vec<_>>();
    let dependency_callables = dependencies
        .iter()
        .map(|d| d.lir_exports().callables())
        .collect::<Vec<_>>();
    let export_dependencies = scoop_lir_lower::LayoutAbiExportDependenciesV1 {
        layouts: &dependency_layouts,
        callables: &dependency_callables,
        direct_callables: &dependencies
            .iter()
            .map(|d| d.lir_exports().direct_callables())
            .collect::<Vec<_>>(),
    };
    let exports = scoop_lir_lower::lower_layout_abi_exports(input, export_dependencies)
        .map_err(Error::LayoutExports)?;
    let physical = selected_layout.physical_imports().records();
    let roots = scoop_lir_lower::lower_layout_abi_dependencies(
        input,
        export_dependencies,
        &exports,
        &mir_uses,
        physical,
    )
    .map_err(Error::LayoutUses)?;
    let section = lir::CrossConeLayoutAbiSectionV1::try_new(
        exports,
        &dependencies
            .iter()
            .map(|d| d.lir_exports())
            .collect::<Vec<_>>(),
        physical.to_vec(),
        &roots,
    )
    .map_err(Error::Selection)?;
    let registration = registration
        .validate_layout_abi(&section)
        .map_err(Error::LayoutJoin)?;
    let backend = scoop_codegen::ValidatedBackendProfile::from_selection(
        request.target().lir_target_selection(),
    )
    .map_err(Error::Codegen)?;
    let objects = scoop_codegen::emit_object_set_v2(&lir, registration, temporary_parent, backend)
        .map_err(Error::Codegen)?;
    let generated = scoop_codegen::emit_c_bridge_object_set(
        &lir,
        temporary_parent,
        request.target().c_bridge_toolchain(),
    )
    .map_err(Error::Codegen)?;
    let producer = slib::ProducerRecord::new(concat!("scoopc/", env!("CARGO_PKG_VERSION")))
        .map_err(CurrentConeProductionFailure::Producer)?;
    let metadata = crate::CrossConeStrongArtifactMetadataInputV1::new(
        producer,
        cone,
        request.dependencies().direct_dependencies().to_vec(),
        &foundation,
        &hir.production_section,
        hir.cross_cone_section,
        mir.strong.foundation(),
        mir.strong.production(),
        &mir.public,
        &lir_public,
    );
    let owners = request
        .dependencies()
        .closure
        .dependency_symbol_owners()
        .cloned()
        .collect::<Vec<_>>();
    crate::CrossConeLayoutArtifactMetadataInputV1::new(
        metadata,
        source.section(),
        &mir_section,
        &section,
    )
    .assemble(objects, &generated, &owners)
    .map_err(Error::Artifact)
    .map_err(Into::into)
}
