use super::wire;
use scoop_identity::CallableTemplateOrigin;

mod decode;
mod errors;
mod indexed;
mod keys;
mod parameters;
mod semantics;
mod table;
#[cfg(test)]
mod tests;
pub use decode::*;
pub use errors::*;
pub use indexed::*;
pub use keys::*;
pub use parameters::*;
pub use semantics::*;
pub use table::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedCallableSourceInterfaceV1 {
    owner: CallableTemplateOrigin,
    parameters: CanonicalProtectedSourceParametersV1,
}
impl ProtectedCallableSourceInterfaceV1 {
    pub fn try_new(
        owner: CallableTemplateOrigin,
        parameters: CanonicalProtectedSourceParametersV1,
    ) -> Result<Self, ProtectedSourceBuildError> {
        if matches!(owner, CallableTemplateOrigin::Accessor(_)) {
            return Err(ProtectedSourceBuildError::AccessorOwner);
        }
        for (position, parameter) in (0_u32..).zip(parameters.parameters()) {
            if let Some(key) = parameter.calling().template() {
                if key.owner() != owner || key.parameter_position() != position {
                    return Err(ProtectedSourceBuildError::TemplateOwner { position });
                }
            }
        }
        Ok(Self { owner, parameters })
    }
    pub const fn owner(&self) -> CallableTemplateOrigin {
        self.owner
    }
    pub const fn parameters(&self) -> &CanonicalProtectedSourceParametersV1 {
        &self.parameters
    }
}
