use std::fmt;

use scoop_hir as hir;

pub(crate) fn build(
    export: &hir::Module,
    local: &hir::concrete::Module,
    dependencies: &hir::ImportedSemanticWorld<'_>,
) -> Result<hir::HirNativeBoundaryTypeDefinitions, PersistentNativeBoundaryTypeError> {
    let core_types = match &export.core_protocols {
        hir::CoreProtocols::Defined(protocols) => {
            hir::HirCoreTypeIdentityAuthority::Defined(&protocols.fundamental_types)
        }
        hir::CoreProtocols::Imported(protocols) => {
            hir::HirCoreTypeIdentityAuthority::Imported(protocols.fundamental_types())
        }
    };
    hir::HirNativeBoundaryTypeDefinitions::from_roots(hir::HirNativeBoundaryTypeDefinitionInputs {
        protocols: &export.core_protocols,
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
            loaded_enum_definitions: &export.loaded_enum_definitions,
            loaded_struct_definitions: &export.loaded_struct_definitions,
            loaded_class_definitions: &export.loaded_class_definitions,
            loaded_interface_definitions: &export.loaded_interface_definitions,
            enum_applications: &export.enum_applications,
            classes: &export.classes,
            class_applications: &export.class_applications,
            interfaces: &export.interfaces,
            interface_applications: &export.interface_applications,
            objects: &export.objects,
            core_types,
            nominal_identities: &export.nominal_identities,
        },
        source_native_contracts: &export.source_native_contracts,
        callback_registration_identities: &export.callback_registration_identities,
        initialization_unit_identities: &export.initialization_unit_identities,
        local,
        dependencies,
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
