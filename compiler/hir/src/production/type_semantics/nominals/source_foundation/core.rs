use super::*;
use scoop_identity::ConeIdentity;
use scoop_wire::WirePath;

impl CrossConeTypeSemanticsFoundationV1 {
    /// Projects source evidence from the actual trusted-core bootstrap pair.
    /// This source transcript is not a generic materialization or use capability.
    pub fn from_core_bootstrap(output: &Output, meter: &mut BudgetMeter) -> Result<Self, Error> {
        let path = WirePath::root();
        meter
            .check_semantic_depth(1, &path)
            .map_err(inheritance::source_resources::resource)?;
        meter
            .charge_nodes(1, &path)
            .map_err(inheritance::source_resources::resource)?;
        meter
            .charge_work(1, &path)
            .map_err(inheritance::source_resources::resource)?;
        validate(output)?;
        project_pair(output, FactProvider::CoreBootstrap, meter).map(|p| p.foundation)
    }
}

fn validate(output: &Output) -> Result<(), Error> {
    let invalid = |reason: &str| Error::InvalidCoreSourcePair(reason.into());
    if output.export.cone != ConeIdentity::CORE || output.local.cone != ConeIdentity::CORE {
        return Err(invalid("provider is not CORE"));
    }
    if !matches!(
        (&output.export.core_protocols, &output.local.core_protocols),
        (
            CoreProtocols::Defined(_),
            concrete::ConcreteCoreProtocols::Defined(_)
        )
    ) {
        return Err(invalid("core protocols are not locally defined"));
    }
    if [
        output.export.imported_core_callables.len(),
        output.export.imported_core_types.len(),
        output.export.imported_core_values.len(),
        output.export.imported_dependency_callables.len(),
        output.local.imported_core_callables.len(),
        output.local.imported_core_types.len(),
        output.local.imported_core_values.len(),
        output.local.imported_dependency_callables.len(),
    ]
    .into_iter()
    .any(|count| count != 0)
    {
        return Err(invalid("bootstrap core contains imported uses"));
    }
    if !matches!(
        (output.export.output_kind(), output.local.output_kind()),
        (ConeOutputKind::Library, LocalConeOutputKind::Library)
    ) {
        return Err(invalid("core output is not a library"));
    }
    let LocalConcreteMaterializationContract::CoreShapeSupport(actual) =
        output.local.materialization()
    else {
        return Err(invalid("core shape-support plan is absent"));
    };
    let interface = CoreHirInterfaceV1::from_core_export(&output.export)
        .map_err(|e| Error::InvalidCoreSourcePair(e.to_string()))?;
    let expected =
        LocalCoreShapeSupportPlan::try_new(&output.local, &interface.shape_support_requirements())
            .map_err(|e| Error::InvalidCoreSourcePair(e.to_string()))?;
    if actual != &expected {
        return Err(invalid(
            "core shape-support plan does not match the export pair",
        ));
    }
    Ok(())
}
