use super::*;
use scoop_identity::SignatureTypeKey;

impl NominalInterfaceRecordV1 {
    pub(super) fn validate_exact_supertypes<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), NominalInterfaceSemanticValidationError<E>>
    where
        A: NominalInterfaceSemanticAuthority<E>,
    {
        let scope = self.type_parameters.signature_scope(None);
        let mut class_index = None;
        let integer_kind = match &self.source_shape {
            crate::NominalSourceShapeV1::Enum(shape) => shape.representation().integer_kind(),
            _ => None,
        };
        let mut found_integer = false;
        for (index, supertype) in self.exact_supertypes.values().iter().enumerate() {
            let shape = scope
                .validate_nominal_signature_semantics(supertype, authority)
                .map_err(
                    |error| NominalInterfaceSemanticValidationError::ExactSupertype {
                        index,
                        error: ExactSupertypeSemanticError::Signature(error),
                    },
                )?;
            if shape.kind() == PublicNominalKindV1::Struct
                && let (Some(expected), SignatureTypeKey::Nominal(parent)) =
                    (integer_kind, supertype)
                && !found_integer
                && authority
                    .integer_kind(SourceNominalId::Concrete(*parent))
                    .map_err(NominalInterfaceSemanticValidationError::Declaration)?
                    == Some(expected)
            {
                found_integer = true;
                continue;
            }
            match shape.kind() {
                PublicNominalKindV1::Interface => {}
                PublicNominalKindV1::Class => {
                    if !matches!(
                        self.kind,
                        PublicNominalKindV1::Class | PublicNominalKindV1::Object
                    ) {
                        return Err(NominalInterfaceSemanticValidationError::ExactSupertype {
                            index,
                            error: ExactSupertypeSemanticError::ClassNotAllowed {
                                owner: self.kind,
                            },
                        });
                    }
                    if let Some(first_index) = class_index {
                        return Err(NominalInterfaceSemanticValidationError::ExactSupertype {
                            index,
                            error: ExactSupertypeSemanticError::MultipleClasses { first_index },
                        });
                    }
                    class_index = Some(index);
                }
                kind @ (PublicNominalKindV1::Struct
                | PublicNominalKindV1::Enum
                | PublicNominalKindV1::Object) => {
                    return Err(NominalInterfaceSemanticValidationError::ExactSupertype {
                        index,
                        error: ExactSupertypeSemanticError::InvalidTargetKind(kind),
                    });
                }
            }
        }
        if let Some(kind) = integer_kind
            && !found_integer
        {
            return Err(NominalInterfaceSemanticValidationError::IntegerParent(kind));
        }
        Ok(())
    }
}
