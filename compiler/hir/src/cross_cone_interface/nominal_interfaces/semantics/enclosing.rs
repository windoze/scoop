use super::*;
use scoop_identity::{DefinitionOwnerAtom, FieldIdentityView, SignatureTypeKey};

impl NominalInterfaceRecordV1 {
    pub(super) fn validate_enclosing_instance<A, E>(
        &self,
        declaration: &SourceDeclarationKey,
        authority: &mut A,
    ) -> Result<(), NominalInterfaceSemanticValidationError<E>>
    where
        A: NominalInterfaceSemanticAuthority<E>,
    {
        use NominalInterfaceSemanticValidationError as Error;
        for field in self.source_shape.declared_fields() {
            let key = authority
                .nominal_field_key(field.field())
                .map_err(Error::Declaration)?;
            if !matches!(
                key.view(),
                FieldIdentityView::SourceEnclosingInstance { .. }
            ) {
                continue;
            }
            let required = match declaration.owners().owners().last() {
                Some(DefinitionOwnerAtom::Type(id)) => SourceNominalId::Concrete(*id),
                Some(DefinitionOwnerAtom::GenericType(id)) => SourceNominalId::GenericTemplate(*id),
                _ => {
                    return Err(Error::EnclosingInstance(
                        "inner class requires a nominal lexical owner",
                    ));
                }
            };
            let (actual, arguments): (_, &[SignatureTypeKey]) = match field.value_type() {
                SignatureTypeKey::Nominal(id) => (SourceNominalId::Concrete(*id), &[]),
                SignatureTypeKey::NominalApplication { origin, arguments } => (
                    SourceNominalId::GenericTemplate(*origin),
                    arguments.as_slice(),
                ),
                _ => {
                    return Err(Error::EnclosingInstance(
                        "field must have the outer class type",
                    ));
                }
            };
            if actual != required {
                return Err(Error::EnclosingInstance(
                    "field type must match the immediate lexical owner",
                ));
            }
            let shape = authority
                .storage_nominal_shape(required)
                .map_err(Error::Declaration)?;
            if shape.kind() != PublicNominalKindV1::Class {
                return Err(Error::EnclosingInstance(
                    "outer declaration must be a class",
                ));
            }
            if arguments.len() != shape.type_parameter_arity() as usize
                || arguments.len() > self.type_parameters.binders().len()
                || arguments.iter().enumerate().any(|(index, argument)| {
                    *argument
                        != SignatureTypeKey::Binder {
                            depth: 0,
                            index: index as u32,
                        }
                })
            {
                return Err(Error::EnclosingInstance(
                    "outer arguments must be the inner binder prefix",
                ));
            }
        }
        Ok(())
    }
}
