use super::*;
use scoop_identity::{DuplicateSignatureKey, SignatureTypeKey};
use std::borrow::Cow;

pub(super) fn expected<'a>(
    export: &ExportHir,
    signatures: &HirInterfaceSignatureProjector<'_>,
    owner: ExportParameterOwner,
    key: &'a SourceDeclarationKey,
    binders: &[HirSignatureBinder],
) -> Result<Cow<'a, [SignatureTypeKey]>, Error> {
    if let ExportParameterOwner::VariantConstructor(reference) = owner {
        let variant =
            &export.enums[reference.enumeration()].variants[reference.local_index() as usize];
        let path = WirePath::root();

        let mut fields = Vec::new();
        scoop_wire::allocation::try_reserve(&mut fields, variant.fields.len(), &path)
            .map_err(resource)?;
        for field in &variant.fields {
            fields.push(signatures.map_type(field.ty, binders).map_err(invalid)?);
        }
        Ok(Cow::Owned(fields))
    } else {
        match key.duplicate_signature() {
            DuplicateSignatureKey::Function { parameters, .. }
            | DuplicateSignatureKey::Constructor { parameters } => Ok(Cow::Borrowed(parameters)),
            _ => Err(invalid(
                "nominal parameter source key has another declaration kind",
            )),
        }
    }
}
