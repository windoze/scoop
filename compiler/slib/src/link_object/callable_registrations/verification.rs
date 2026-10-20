//! Callable production plans paired with the parsed object data used by content hashing.

use crate::link_object::VerifiedBuiltinObjectStrongRelocationSetV1;
use scoop_lir::StrongCallableRegistrationPlanSetV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableRegistrationDataV1 {
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: StrongCallableRegistrationPlanSetV1,
}
impl CallableRegistrationDataV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }
    pub const fn builtins(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.builtins
    }
    pub const fn plan(&self) -> &StrongCallableRegistrationPlanSetV1 {
        &self.plan
    }
}

pub fn bind_callable_registrations_v1(
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: StrongCallableRegistrationPlanSetV1,
) -> CallableRegistrationDataV1 {
    CallableRegistrationDataV1 { builtins, plan }
}
