use super::*;
use crate::production::callable_interfaces::source_function_effects;
use crate::production::type_semantics::nominals::declaration_access_for_subject;
use scoop_identity::{DefinitionOriginSubject, ExactCallableSignature, SourceDeclarationKey};

pub(super) fn project(
    export: &ExportHir,
    id: FunctionId,
    declaration: Declaration,
) -> Result<InheritanceSourceCallableV1, Error> {
    let function = &export.functions[id];
    let method = function
        .method
        .ok_or_else(|| invalid("dispatch source callable has no member owner"))?;
    let owner = exact(export, method.owner)?;
    if !matches!(function.genericity, FunctionGenericity::Plain) {
        return Err(Error::GenericOdrRequired(owner));
    }
    let Some((receiver, parameters)) = function.params.split_first() else {
        return Err(invalid(
            "dispatch source signature has no receiver parameter",
        ));
    };
    if receiver.ty != method.owner {
        return Err(invalid(
            "dispatch source receiver disagrees with its method owner",
        ));
    }
    let path = WirePath::root();

    let mut exact_parameters = Vec::new();
    scoop_wire::allocation::try_reserve(&mut exact_parameters, parameters.len(), &path)
        .map_err(resource)?;
    for parameter in parameters {
        exact_parameters.push(exact(export, parameter.ty)?);
    }
    let effects =
        source_function_effects(export, function).map_err(|error| invalid(error.to_string()))?;
    let signature = InheritanceCallableSignatureV1::try_new(
        ExactCallableSignature::new(
            effects.execution(),
            Some(owner),
            exact_parameters,
            exact(export, function.return_ty)?,
        ),
        effects,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let modality = match method.dispatch {
        MethodDispatch::Interface(member) => {
            let member = &export.interface_methods[member];
            if member.function != id {
                return Err(invalid(
                    "dispatch source interface member disagrees with its function",
                ));
            }
            match member.implementation {
                InterfaceMemberImplementation::Body => CallableModalityV1::InterfaceDefault,
                InterfaceMemberImplementation::AbstractSlot => CallableModalityV1::Abstract,
            }
        }
        MethodDispatch::Direct | MethodDispatch::Virtual(_) | MethodDispatch::FinalOverride(_) => {
            match method.modifier {
                MethodModifier::Final => CallableModalityV1::Final,
                MethodModifier::Open => CallableModalityV1::Open,
                MethodModifier::Abstract => CallableModalityV1::Abstract,
            }
        }
    };
    let (key, subject, visibility) = access_source(export, id)?;

    let access = declaration_access_for_subject(export, key, subject, visibility.into())?;
    Ok(InheritanceSourceCallableV1::new(
        declaration,
        signature,
        modality,
        access,
    ))
}

fn access_source(
    export: &ExportHir,
    function: FunctionId,
) -> Result<
    (
        &SourceDeclarationKey,
        DefinitionOriginSubject,
        DeclaredVisibility,
    ),
    Error,
> {
    let (identity, visibility) = match &export.function_identities[function] {
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => {
            return Ok((
                record.key(),
                DefinitionOriginSubject::Function(record.id()),
                export.functions[function].access.declared,
            ));
        }
        HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Getter(id)) => (
            &export.property_accessor_identities[*id],
            export.property_getters[*id].access.declared,
        ),
        HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Setter(id)) => (
            &export.property_accessor_identities[*id],
            export.property_setters[*id].access.declared,
        ),
        _ => {
            return Err(invalid(
                "dispatch source callable has no source definition origin",
            ));
        }
    };
    let HirPropertyIdentity::Ordinary(property) = &export.property_identities[identity.property()]
    else {
        return Err(invalid(
            "dispatch accessor belongs to an extension property",
        ));
    };
    Ok((
        property.key(),
        DefinitionOriginSubject::PropertyAccessor(identity.id()),
        visibility,
    ))
}
