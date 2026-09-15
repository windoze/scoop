use scoop_hir as hir;

use crate::Lowerer;

pub(crate) use hir::{
    HirSignatureBinder as SignatureBinder,
    HirSignatureTypeMappingError as SignatureTypeMappingError,
};

pub(crate) struct SignatureTypeMapper<'a>(hir::HirSignatureTypeMapper<'a>);

impl<'a> SignatureTypeMapper<'a> {
    pub(crate) fn new(
        lowerer: &'a Lowerer,
        nominals: &'a hir::HirNominalIdentities,
        intrinsic_core: &'a hir::IntrinsicTypeCore,
    ) -> Self {
        Self(hir::HirSignatureTypeMapper::new(identity_inputs(
            lowerer,
            nominals,
            intrinsic_core,
        )))
    }

    pub(crate) fn map(
        &self,
        ty: hir::TypeId,
        binders: &[SignatureBinder],
    ) -> Result<scoop_identity::SignatureTypeKey, SignatureTypeMappingError> {
        self.0.map(ty, binders)
    }
}

pub(crate) fn identity_inputs<'a>(
    lowerer: &'a Lowerer,
    nominals: &'a hir::HirNominalIdentities,
    intrinsic_core: &'a hir::IntrinsicTypeCore,
) -> hir::HirTypeIdentityInputs<'a> {
    hir::HirTypeIdentityInputs {
        types: &lowerer.types,
        function_types: &lowerer.function_types,
        structs: &lowerer.structs,
        struct_applications: &lowerer.struct_applications,
        enums: &lowerer.enums,
        enum_applications: &lowerer.enum_applications,
        classes: &lowerer.classes,
        class_applications: &lowerer.class_applications,
        interfaces: &lowerer.interfaces,
        interface_applications: &lowerer.interface_applications,
        objects: &lowerer.objects,
        core_types: hir::HirCoreTypeIdentityAuthority::Defined(intrinsic_core),
        nominal_identities: nominals,
    }
}
