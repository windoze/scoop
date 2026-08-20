use super::*;
use crate::call_resolution::candidates::ValueParameterCalling;

impl ImportedIntrinsicSignature {
    pub(super) fn validate_atomic(&self, state: &mut Lowerer) -> Result<(), String> {
        let interface = self.declaration.interface();
        let hir::CallableImplementationV1::Intrinsic(hir::IntrinsicFunctionKind::Atomic(kind)) =
            interface.effects().implementation()
        else {
            return Ok(());
        };
        if self.atomic_signature_matches(state, kind) {
            Ok(())
        } else {
            Err(format!(
                "malformed imported atomic intrinsic `{}`",
                kind.name()
            ))
        }
    }

    fn atomic_signature_matches(&self, state: &mut Lowerer, kind: hir::AtomicIntrinsic) -> bool {
        let Some(receiver) = self.signature.receiver else {
            return false;
        };
        let Some((family, value)) = state.atomic_value_type(receiver) else {
            return false;
        };
        let signature = &self.signature.signature;
        let values = kind.method().value_parameter_count();
        let orders = kind.method().order_parameter_count();
        let result = match kind.method() {
            hir::AtomicMethod::Store => state.unit,
            hir::AtomicMethod::CompareAndSet => state.boolean,
            _ => value,
        };
        let effects = self.declaration.interface().effects();
        if family != kind.family()
            || !signature.callable_parameters.is_empty()
            || effects.execution() != scoop_identity::Effect::Ordinary
            || effects.gc_effect() != scoop_identity::GcEffect::NoGc
            || signature.return_type != result
            || signature.value_parameters.len() != values + orders
        {
            return false;
        }
        let order = signature.value_parameters[values].ty;
        state.atomic_order_type_matches(order)
            && signature
                .value_parameters
                .iter()
                .enumerate()
                .all(|(index, parameter)| {
                    if index < values {
                        parameter.ty == value
                            && matches!(parameter.calling, ValueParameterCalling::Required)
                    } else {
                        parameter.ty == order
                            && matches!(parameter.calling, ValueParameterCalling::Default(_))
                    }
                })
    }
}
