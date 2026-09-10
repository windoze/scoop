use std::fmt;

use scoop_hir as hir;

use crate::Lowerer;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    lowerer: &Lowerer,
    source_files: &[hir::SourceFileMetadata],
    nominals: &hir::HirNominalIdentities,
    functions: &hir::HirFunctionIdentities,
    accessors: &hir::HirPropertyAccessorIdentities,
    constructors: &hir::HirConstructorIdentities,
    properties: &hir::HirPropertyIdentities,
    initialization_units: &hir::HirInitializationUnitIdentities,
) -> Result<hir::HirSourceContextIdentities, PersistentSourceContextIdentityError> {
    hir::HirSourceContextIdentities::from_contexts(hir::HirSourceContextIdentityInputs {
        source_files,
        source_contexts: &lowerer.source_contexts,
        structs: &lowerer.structs,
        enums: &lowerer.enums,
        classes: &lowerer.classes,
        interfaces: &lowerer.interfaces,
        objects: &lowerer.objects,
        functions: &lowerer.functions,
        struct_constructors: &lowerer.struct_constructors,
        class_constructors: &lowerer.class_constructors,
        properties: &lowerer.properties,
        initialization_units: &lowerer.initialization_units,
        singleton_values: &lowerer.singleton_values,
        lambdas: &lowerer.lambdas,
        anonymous_functions: &lowerer.anonymous_functions,
        nominal_identities: nominals,
        function_identities: functions,
        property_accessor_identities: accessors,
        constructor_identities: constructors,
        property_identities: properties,
        initialization_unit_identities: initialization_units,
    })
    .map_err(PersistentSourceContextIdentityError)
}

#[derive(Debug)]
pub(crate) struct PersistentSourceContextIdentityError(hir::HirSourceContextIdentityError);

impl fmt::Display for PersistentSourceContextIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent source-context identity: {}",
            self.0
        )
    }
}

impl std::error::Error for PersistentSourceContextIdentityError {}

#[cfg(test)]
mod tests;
