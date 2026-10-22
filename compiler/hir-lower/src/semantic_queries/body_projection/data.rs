use super::*;

pub(super) struct ProjectionData {
    pub(super) functions: hir::HirFunctionIdentities,
    pub(super) properties: hir::HirPropertyIdentities,
    pub(super) accessors: hir::HirPropertyAccessorIdentities,
    pub(super) constructors: hir::HirConstructorIdentities,
    pub(super) initializations: hir::HirInitializationUnitIdentities,
    pub(super) objects: hir::HirObjectValueIdentities,
    pub(super) callbacks: hir::HirCallbackRegistrationIdentities,
    pub(super) contexts: hir::HirSourceContextIdentities,
    pub(super) source_files: Vec<hir::SourceFileMetadata>,
    pub(super) local_scopes: la_arena::Arena<hir::DefaultLocalValueScope>,
    pub(super) imported: hir::SelectedImportedDependencySet,
}

impl ProjectionData {
    pub(super) fn new(lowerer: &Lowerer) -> Result<Self, Diagnostic> {
        let nominals = lowerer
            .nominal_identities
            .as_ref()
            .expect("fixed queries follow nominal identity construction");
        let variants = lowerer
            .enum_member_identities
            .as_ref()
            .expect("fixed queries follow variant identity construction");
        let core = lowerer.query_core_types();
        let types = crate::persistent_type_identities::build(lowerer, nominals, core)
            .map_err(projection_error)?;
        let constructors = crate::persistent_constructor_identities::build(lowerer, nominals, core)
            .map_err(projection_error)?;
        let properties = crate::persistent_properties::build(lowerer, nominals, core)
            .map_err(projection_error)?;
        let accessors =
            crate::persistent_accessors::build(lowerer, &properties).map_err(projection_error)?;
        let objects =
            crate::persistent_object_values::build(lowerer, nominals).map_err(projection_error)?;
        let initializations =
            crate::persistent_initialization_units::build(lowerer, nominals, &properties)
                .map_err(projection_error)?;
        let functions = crate::persistent_functions::build(
            lowerer,
            nominals,
            &properties,
            &accessors,
            &initializations,
            &types,
            &constructors,
            variants,
            core,
        )
        .map_err(projection_error)?;
        let callbacks = crate::persistent_callbacks::build(
            lowerer,
            nominals,
            &accessors,
            &constructors,
            variants,
            &functions,
            core,
        )
        .map_err(projection_error)?;
        let source_files = lowerer.source_file_metadata();
        let contexts = crate::persistent_source_contexts::build(
            lowerer,
            &source_files,
            nominals,
            &functions,
            &accessors,
            &constructors,
            &properties,
            &initializations,
        )
        .map_err(projection_error)?;
        let local_scopes = crate::defaults::finish_default_local_scopes(
            lowerer.default_local_value_scopes.clone().into_owned(),
            &functions,
            &accessors,
            &constructors,
            variants,
        );
        let imported = lowerer
            .dependencies
            .as_ref()
            .expect("fixed queries have their selected dependencies")
            .clone()
            .finish();
        Ok(Self {
            functions,
            properties,
            accessors,
            constructors,
            initializations,
            objects,
            callbacks,
            contexts,
            source_files,
            local_scopes,
            imported,
        })
    }
}
