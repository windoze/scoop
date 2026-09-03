//! Concrete substitution after call resolution.

use super::*;

impl Lowerer {
    pub(super) fn substitute_call_level(&mut self, ty: TypeId, type_args: &[TypeId]) -> TypeId {
        if type_args.is_empty() {
            ty
        } else {
            self.instantiate_ty(ty, type_args)
        }
    }
}
