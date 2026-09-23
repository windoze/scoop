use scoop_hir::DefaultLocalFunctionSignatureAuthority;
use scoop_identity::{
    CallableTemplateOrigin, PersistentFunctionId, PersistentGenericFunctionId, SourceDeclarationKey,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::{DefaultMetadataNominalError, DefaultNominalShapes};

impl DefaultLocalFunctionSignatureAuthority<DefaultMetadataNominalError>
    for DefaultNominalShapes<'_>
{
    fn default_local_function_own_binder_arity(
        &mut self,
        declaration: CallableTemplateOrigin,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<u32, DefaultMetadataNominalError> {
        meter.charge_work(
            (u64::from(self.identities.identity_count().max(1).ilog2()) + 1) * 64,
            path,
        )?;
        let key = match declaration {
            CallableTemplateOrigin::Function(id) => self
                .identities
                .canonical_key::<PersistentFunctionId, SourceDeclarationKey>(id),
            CallableTemplateOrigin::GenericFunction(id) => {
                self.identities
                    .canonical_key::<PersistentGenericFunctionId, SourceDeclarationKey>(id)
            }
            _ => {
                return Err(DefaultMetadataNominalError::NonFunctionSignature(
                    declaration,
                ));
            }
        }
        .map_err(DefaultMetadataNominalError::Identity)?;
        Ok(key.duplicate_signature().type_parameter_count())
    }
}
