//! Type plans paired with the parsed descriptor objects used by content hashing.

use crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1;
use scoop_lir::{
    StrongDescriptorReference, StrongTypeDescriptorRefV1, StrongTypeDispatchCallableRefV1,
    StrongTypeRegistrationPlanSet,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRegistrationDataV1<
    D = StrongTypeDescriptorRefV1,
    C = StrongTypeDispatchCallableRefV1,
> {
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: StrongTypeRegistrationPlanSet<D, C>,
}
impl<D: StrongDescriptorReference, C: Clone> TypeRegistrationDataV1<D, C> {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }
    pub const fn builtins(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.builtins
    }
    pub const fn plan(&self) -> &StrongTypeRegistrationPlanSet<D, C> {
        &self.plan
    }
}

pub fn bind_type_registrations_v1<D: StrongDescriptorReference, C: Clone>(
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: StrongTypeRegistrationPlanSet<D, C>,
) -> TypeRegistrationDataV1<D, C> {
    TypeRegistrationDataV1 { builtins, plan }
}
