use super::*;
use scoop_identity::SignatureTypeKey;

/// A source type together with the existing declaration-to-application
/// relation. No substituted type graph is stored alongside HIR.
#[derive(Clone, Copy)]
pub enum StaticShapeTypeSource<'a> {
    Hir(TypeId),
    Published(&'a SignatureTypeKey),
}

#[derive(Clone, Copy)]
pub struct StaticShapeTypeUse<'a> {
    pub(super) source: StaticShapeTypeSource<'a>,
    pub(super) parameters: &'a [TypeParamDecl],
    pub(super) arguments: &'a [TypeId],
}

impl<'a> StaticShapeTypeUse<'a> {
    pub const fn source(self) -> StaticShapeTypeSource<'a> {
        self.source
    }

    pub fn substitutions(self) -> impl ExactSizeIterator<Item = (TypeParamId, TypeId)> {
        self.parameters
            .iter()
            .zip(self.arguments)
            .map(|(parameter, argument)| (parameter.id, *argument))
    }

    /// Closed arguments produce a closed signature. Open arguments retain
    /// the caller's binder identities, including when two arguments coincide.
    pub fn signature(
        self,
        module: &Module,
        binders: &[HirSignatureBinder],
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        match self.source {
            StaticShapeTypeSource::Hir(ty) => {
                HirSignatureTypeMapper::new(HirTypeIdentityInputs::from_export(module))
                    .map_substituted(ty, binders, &self.substitutions().collect::<Vec<_>>())
            }
            StaticShapeTypeSource::Published(signature) => {
                substitute_signature(module, self.arguments, signature, binders)
            }
        }
    }
}

impl StaticNominalShape<'_> {
    pub(super) fn substitute_signature(
        self,
        signature: &SignatureTypeKey,
        binders: &[HirSignatureBinder],
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        substitute_signature(self.module, self.arguments, signature, binders)
    }
}

fn substitute_signature(
    module: &Module,
    arguments: &[TypeId],
    signature: &SignatureTypeKey,
    binders: &[HirSignatureBinder],
) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
    if arguments.is_empty() {
        return Ok(signature.clone());
    }
    let mapper = HirSignatureTypeMapper::new(HirTypeIdentityInputs::from_export(module));
    let arguments = arguments
        .iter()
        .map(|ty| mapper.map(*ty, binders))
        .collect::<Result<Vec<_>, _>>()?;
    let mapping = CanonicalBinderUseListV1::try_new(arguments)
        .expect("HIR application arities fit the source binder representation");
    let provider = DefaultTemplateProviderShapeV1::try_new(mapping.len_u32(), 0)
        .expect("a nominal substitution has one binder frame");
    Ok(mapping
        .substitute_provider_type(provider, signature)
        .expect("the imported signature was checked in its declaring binder scope"))
}
