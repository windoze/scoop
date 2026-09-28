use scoop_hir::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, CrossConeHirInterfaceSectionV1, ExternalHirReferenceV1,
};

pub(super) fn empty_interface(
    references: Vec<ExternalHirReferenceV1>,
) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(references).unwrap(),
        Default::default(),
        Default::default(),
        Default::default(),
    )
}
