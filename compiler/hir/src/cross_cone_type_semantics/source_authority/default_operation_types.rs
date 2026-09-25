//! Operation type inputs from imported language roles, without extra origin gates.
use crate::*;
use scoop_identity::{PersistentGenericTypeId, SignatureTypeKey};
use scoop_wire::{WireError, WirePath};
mod errors;
pub use errors::DefaultOperationProtocolTypeError;
type Error = DefaultOperationProtocolTypeError;

impl ImportedCoreProtocols {
    pub fn default_operation_type(
        &self,
        role: DefaultOperationCoreTypeV1,
    ) -> Result<SignatureTypeKey, WireError> {
        use DefaultOperationCoreTypeV1 as Role;
        let fundamental = self.fundamental_types();
        let nominal = match role {
            Role::Unit => fundamental.unit(),
            Role::Boolean => fundamental.boolean(),
            Role::Integer(kind) => fundamental.integer(kind.into()),
            Role::String => fundamental.string(),
            Role::Throwable => self.exceptions().throwable(),
            Role::ForeignCallbackState => self.foreign_callbacks().state(),
        };
        Ok(SignatureTypeKey::Nominal(nominal.persistent()))
    }

    /// Classification only. Argument semantics, declaration access and executable
    /// capability still belong to the complete source transaction.
    pub fn classify_default_operation_application(
        &self,
        value: &SignatureTypeKey,

        path: &WirePath,
    ) -> Result<Option<DefaultCoreApplicationV1>, Error> {
        let SignatureTypeKey::NominalApplication { origin, arguments } = value else {
            return Ok(None);
        };
        let roles = [
            (
                self.fundamental_types().array().persistent(),
                Application::Array,
            ),
            (
                self.fundamental_types().mutable_array().persistent(),
                Application::MutableArray,
            ),
            (self.option().option().persistent(), Application::Option),
            (
                self.foreign_callbacks().callback().persistent(),
                Application::ForeignCallback,
            ),
        ];

        let Some((_, role)) = roles.into_iter().find(|(id, _)| id == origin) else {
            return Ok(None);
        };
        let [argument] = arguments.as_slice() else {
            return Err(Error::Arity {
                origin: *origin,
                actual: arguments.as_slice().len(),
            });
        };
        let argument = copy_default_signature_type(argument, path).map_err(Error::copy)?;
        Ok(Some(match role {
            Application::Array => DefaultCoreApplicationV1::Array { element: argument },
            Application::MutableArray => {
                DefaultCoreApplicationV1::MutableArray { element: argument }
            }
            Application::Option => DefaultCoreApplicationV1::Option { element: argument },
            Application::ForeignCallback => DefaultCoreApplicationV1::ForeignCallback {
                function_type: argument,
            },
        }))
    }
}

enum Application {
    Array,
    MutableArray,
    Option,
    ForeignCallback,
}
