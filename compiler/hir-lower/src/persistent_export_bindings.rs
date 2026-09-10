use std::fmt;

use scoop_hir as hir;

use crate::Lowerer;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    lowerer: &Lowerer,
    surface: &hir::PublicSemanticSurface,
    nominals: &hir::HirNominalIdentities,
    object_values: &hir::HirObjectValueIdentities,
    functions: &hir::HirFunctionIdentities,
    properties: &hir::HirPropertyIdentities,
    aliases: &hir::HirTypeAliasIdentities,
) -> Result<hir::HirExportBindingIdentities, PersistentExportBindingIdentityError> {
    hir::HirExportBindingIdentities::from_public_surface(hir::HirExportBindingIdentityInputs {
        surface,
        structs: &lowerer.structs,
        enums: &lowerer.enums,
        classes: &lowerer.classes,
        interfaces: &lowerer.interfaces,
        objects: &lowerer.objects,
        singleton_values: &lowerer.singleton_values,
        functions: &lowerer.functions,
        properties: &lowerer.properties,
        type_aliases: &lowerer.type_aliases,
        nominal_identities: nominals,
        object_value_identities: object_values,
        function_identities: functions,
        property_identities: properties,
        type_alias_identities: aliases,
    })
    .map_err(PersistentExportBindingIdentityError)
}

#[derive(Debug)]
pub(crate) struct PersistentExportBindingIdentityError(hir::HirExportBindingIdentityError);

impl fmt::Display for PersistentExportBindingIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent export binding identity: {}",
            self.0
        )
    }
}

impl std::error::Error for PersistentExportBindingIdentityError {}

#[cfg(test)]
mod tests;
