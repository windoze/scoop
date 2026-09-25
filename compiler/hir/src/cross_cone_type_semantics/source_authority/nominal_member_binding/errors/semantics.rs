use super::*;

impl Error {
    pub(in super::super) fn from_callable(
        error: NominalSupportCallableSemanticError<Self>,
    ) -> Self {
        match error {
            NominalSupportCallableSemanticError::Foundation(e)
            | NominalSupportCallableSemanticError::Signature(
                ProtectedCallableSemanticError::Foundation(e),
            ) => e,
            NominalSupportCallableSemanticError::Signature(
                ProtectedCallableSemanticError::Resource(e),
            )
            | NominalSupportCallableSemanticError::Signature(
                ProtectedCallableSemanticError::Signature(SignatureTypeSemanticError::Allocation(
                    e,
                )),
            )
            | NominalSupportCallableSemanticError::Signature(
                ProtectedCallableSemanticError::Source(InheritanceGraphError::Resource(e)),
            )
            | NominalSupportCallableSemanticError::Variant(NominalSupportVariantError::Resource(
                e,
            )) => Self::Resource(e),
            other => Self::Callable(Box::new(other)),
        }
    }
    pub(in super::super) fn from_property(
        error: NominalSupportPropertySemanticError<Self>,
    ) -> Self {
        match error {
            NominalSupportPropertySemanticError::Foundation(e)
            | NominalSupportPropertySemanticError::Runtime(
                ProtectedPropertySemanticError::Foundation(e),
            ) => e,
            NominalSupportPropertySemanticError::Resource(e)
            | NominalSupportPropertySemanticError::Source(InheritanceGraphError::Resource(e))
            | NominalSupportPropertySemanticError::Runtime(
                ProtectedPropertySemanticError::Resource(e),
            )
            | NominalSupportPropertySemanticError::Runtime(
                ProtectedPropertySemanticError::Signature(SignatureTypeSemanticError::Allocation(
                    e,
                )),
            )
            | NominalSupportPropertySemanticError::Runtime(
                ProtectedPropertySemanticError::Source(InheritanceGraphError::Resource(e)),
            )
            | NominalSupportPropertySemanticError::Runtime(
                ProtectedPropertySemanticError::Domain(AccessDomainSemanticError::Resource(e)),
            ) => Self::Resource(e),
            other => Self::Property(Box::new(other)),
        }
    }
}
