use std::fmt;

use scoop_hir as hir;

pub(crate) fn build(
    export: &hir::Module,
    local: &hir::concrete::Module,
) -> Result<hir::HirNativeBoundaryTypeDefinitions, PersistentNativeBoundaryTypeError> {
    hir::HirNativeBoundaryTypeDefinitions::from_roots(hir::HirNativeBoundaryTypeDefinitionInputs {
        structs: &export.structs,
        enums: &export.enums,
        classes: &export.classes,
        interfaces: &export.interfaces,
        objects: &export.objects,
        nominal_identities: &export.nominal_identities,
        field_identities: &export.field_identities,
        enum_member_identities: &export.enum_member_identities,
        type_inputs: hir::HirTypeIdentityInputs {
            types: &export.types,
            function_types: &export.function_types,
            structs: &export.structs,
            struct_applications: &export.struct_applications,
            enums: &export.enums,
            enum_applications: &export.enum_applications,
            classes: &export.classes,
            class_applications: &export.class_applications,
            interfaces: &export.interfaces,
            interface_applications: &export.interface_applications,
            objects: &export.objects,
            core_types: hir::HirCoreTypeIdentityAuthority::Defined(
                &export.core_protocols.fundamental_types,
            ),
            nominal_identities: &export.nominal_identities,
        },
        source_native_contracts: &export.source_native_contracts,
        callback_registration_identities: &export.callback_registration_identities,
        initialization_unit_identities: &export.initialization_unit_identities,
        local,
    })
    .map_err(PersistentNativeBoundaryTypeError)
}

#[derive(Debug)]
pub(crate) struct PersistentNativeBoundaryTypeError(hir::HirNativeBoundaryTypeDefinitionError);

impl fmt::Display for PersistentNativeBoundaryTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive native-boundary type definitions: {}",
            self.0
        )
    }
}

impl std::error::Error for PersistentNativeBoundaryTypeError {}
