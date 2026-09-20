use super::*;
use hir::{ProtectedParameterCallingKindV1 as Kind, ProtectedParameterCallingV1 as Calling};
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

pub(super) fn candidate(
    owner: CallableTemplateOrigin,
    parameters: &[hir::InheritanceSourceParameterV1],
) -> hir::ProtectedCallableSourceInterfaceV1 {
    let parameters = parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            let template =
                || hir::ProtectedDefaultTemplateKeyV1::try_new(owner, position as u32).unwrap();
            let element_type = || {
                let SignatureTypeKey::NominalApplication { arguments, .. } =
                    parameter.shape().value_type()
                else {
                    panic!("source vararg Array application")
                };
                arguments.as_slice()[0].clone()
            };
            let calling = match parameter.calling_kind() {
                Kind::Required => Calling::Required,
                Kind::Default => Calling::Default {
                    template: template(),
                },
                Kind::VarargEmpty => Calling::VarargEmpty {
                    element_type: element_type(),
                },
                Kind::VarargDefault => Calling::VarargDefault {
                    element_type: element_type(),
                    template: template(),
                },
            };
            hir::ProtectedSourceParameterV1::new(
                parameter.shape().name().clone(),
                parameter.shape().value_type().clone(),
                calling,
                parameter.definition_origin().clone(),
            )
        })
        .collect();
    hir::ProtectedCallableSourceInterfaceV1::try_new(
        owner,
        hir::CanonicalProtectedSourceParametersV1::try_new(parameters).unwrap(),
    )
    .unwrap()
}
