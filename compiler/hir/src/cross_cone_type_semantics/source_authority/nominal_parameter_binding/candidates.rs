use super::*;

impl BoundNominalParameterProtocolsV1<'_, '_, '_, '_> {
    /// Replay a candidate protocol against the complete artifact-bound source.
    /// Default template bodies still require the independent defaults transaction.
    pub fn validate_source_protocol<'c>(
        &mut self,
        candidate: &'c ProtectedCallableSourceInterfaceV1,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedProtectedSourceProtocolV1<'c>, ProtectedSourceSemanticError<Error>> {
        let owner = candidate.owner();
        let (payload, access) = match owner {
            CallableTemplateOrigin::Constructor(id) => {
                query(self.constructors.table().records().len(), meter)
                    .map_err(ProtectedSourceSemanticError::Foundation)?;
                let source = self
                    .constructors
                    .constructor_source(id)
                    .map_err(Error::from)
                    .map_err(ProtectedSourceSemanticError::Foundation)?;
                (source.payload(), source.declaration_access())
            }
            CallableTemplateOrigin::Function(_)
            | CallableTemplateOrigin::GenericFunction(_)
            | CallableTemplateOrigin::VariantConstructor(_) => {
                query(self.members.callables().records().len(), meter)
                    .map_err(ProtectedSourceSemanticError::Foundation)?;
                let source = self
                    .members
                    .callable_source(owner)
                    .map_err(Error::from)
                    .map_err(ProtectedSourceSemanticError::Foundation)?;
                (source.payload(), source.declaration_access())
            }
            CallableTemplateOrigin::Accessor(_) => {
                return Err(ProtectedSourceSemanticError::Foundation(
                    Error::Declaration(owner),
                ));
            }
        };
        candidate.validate(owner, payload, access, self, meter)
    }
}
