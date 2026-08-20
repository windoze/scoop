use super::*;

pub(super) struct PublicFixtureSurface {
    pub(super) surface: hir::PublicSemanticSurface,
    pub(super) identities: hir::HirExportBindingIdentities,
    pub(super) bindings: hir::CanonicalPublicExportBindingsV1,
}

impl Harness {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn test_public_surface(
        &self,
        surface: hir::PublicSemanticSurface,
        nominal_identities: &hir::HirNominalIdentities,
        enum_member_identities: &hir::HirEnumMemberIdentities,
        object_value_identities: &hir::HirObjectValueIdentities,
        function_identities: &hir::HirFunctionIdentities,
        property_identities: &hir::HirPropertyIdentities,
        type_alias_identities: &hir::HirTypeAliasIdentities,
    ) -> PublicFixtureSurface {
        let identities = hir::HirExportBindingIdentities::from_public_surface(
            hir::HirExportBindingIdentityInputs {
                surface: &surface,
                annotations: &hir::SourceAnnotations::default(),
                structs: &self.structs,
                enums: &self.enums,
                classes: &self.classes,
                interfaces: &self.interfaces,
                objects: &Arena::new(),
                singleton_values: &Arena::new(),
                functions: &self.functions,
                properties: &self.properties,
                type_aliases: &Arena::new(),
                nominal_identities,
                enum_member_identities,
                object_value_identities,
                function_identities,
                property_identities,
                type_alias_identities,
            },
        )
        .expect("the MIR test public declarations have valid export bindings");
        let bindings = hir::CanonicalPublicExportBindingsV1::try_new(
            identities
                .iter()
                .map(|identity| {
                    hir::PublicExportBindingRecordV1::new(
                        identity.id(),
                        hir::ExportBindingSourceV1::DeclaredCurrent {
                            declaration: identity.key().target(),
                        },
                    )
                })
                .collect(),
        )
        .unwrap();
        PublicFixtureSurface {
            surface,
            identities,
            bindings,
        }
    }
}
