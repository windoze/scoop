use super::*;
use crate::{CoreCompilerProtocolSurfaceV1, CoreProtocolCallableDefinitionV1};
use scoop_identity::SignatureCallableShape;

impl HirDependencyCallSiteV1 {
    /// Checks the resolved language role after the source declaration is bound.
    /// Neither the role nor this relation alone grants machine-use capability.
    pub fn validate_runtime_constructor_role(
        &self,
        target: ExternalHirTargetV1,
        owner: PersistentTypeId,
        roles: &CoreCompilerProtocolSurfaceV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), HirRuntimeConstructorError> {
        use HirRuntimeConstructorError as Error;
        let path = WirePath::root();
        meter.charge_work(1, &path)?;
        if !matches!(self.reason(), HirDependencyCallReasonV1::CastFailure { .. }) {
            return Err(Error::CallShape);
        }
        let selected = roles.class_cast_exception_constructor();
        let expected = match selected.definition() {
            CoreProtocolCallableDefinitionV1::Constructor(id) => {
                ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(id))
            }
            CoreProtocolCallableDefinitionV1::GeneratedCallable(id) => {
                ExternalHirTargetV1::GeneratedCallable(id)
            }
            _ => return Err(Error::RoleTarget),
        };
        if target != expected {
            return Err(Error::RoleTarget);
        }
        let signature = SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            SignatureTypeKey::Nominal(owner),
        );
        meter.charge_work(
            scoop_wire::encoded_length(&signature).map_err(|_| Error::RoleSignature)?,
            &path,
        )?;
        if roles.class_cast_exception_type() != owner || selected.signature() != &signature {
            return Err(Error::RoleSignature);
        }
        Ok(())
    }
}
