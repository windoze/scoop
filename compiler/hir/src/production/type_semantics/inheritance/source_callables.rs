use super::*;
use scoop_wire::WirePath;

mod contract;

type Declaration = InheritanceCallableDeclarationV1;

pub(super) fn local(
    export: &ExportHir,
    id: FunctionId,
    declaration: Declaration,
) -> Result<InheritanceSourceCallableV1, Error> {
    contract::project(export, id, declaration)
}

pub(super) fn imported(
    metadata: SharedTypeMetadataV1<'_>,
    declaration: Declaration,
) -> Result<
    (
        InheritanceSourceCallableV1,
        scoop_identity::PersistentTypeId,
    ),
    Error,
> {
    let origin = match declaration {
        Declaration::Function(id) => scoop_identity::CallableTemplateOrigin::Function(id),
        Declaration::Getter(id) | Declaration::Setter(id) => {
            scoop_identity::CallableTemplateOrigin::Accessor(id)
        }
    };
    let source = metadata
        .public
        .callable_interfaces()
        .declaration(origin)
        .ok_or_else(|| invalid("dependency dispatch callable has no declaration"))?;
    let PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner)) = source.owner() else {
        return Err(invalid("dispatch callable needs an exact nominal owner"));
    };
    let receiver = scoop_identity::SignatureTypeKey::Nominal(owner);
    let signature = InheritanceCallableSignatureV1::try_new(
        scoop_identity::ExactCallableSignature::new(
            source.effects().execution(),
            Some(metadata.signature_exact_type(&receiver).map_err(invalid)?),
            source
                .parameters()
                .parameters()
                .iter()
                .map(|parameter| {
                    metadata
                        .signature_exact_type(parameter.value_type())
                        .map_err(invalid)
                })
                .collect::<Result<Vec<_>, _>>()?,
            metadata
                .signature_exact_type(source.result())
                .map_err(invalid)?,
        ),
        source.effects(),
    )
    .map_err(invalid)?;
    let access = metadata
        .callable_declaration_access(source)
        .map_err(invalid)?;
    Ok((
        InheritanceSourceCallableV1::new(declaration, signature, source.modality(), access),
        owner,
    ))
}

pub(super) fn identity(export: &ExportHir, function: FunctionId) -> Option<Declaration> {
    match &export.function_identities[function] {
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => {
            Some(Declaration::Function(record.id()))
        }
        HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Getter(id)) => Some(
            Declaration::Getter(export.property_accessor_identities[*id].id()),
        ),
        HirFunctionIdentity::PropertyAccessor(HirPropertyAccessorFunction::Setter(id)) => Some(
            Declaration::Setter(export.property_accessor_identities[*id].id()),
        ),
        HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(_))
        | HirFunctionIdentity::LexicalGenerated(_)
        | HirFunctionIdentity::Initialization { .. }
        | HirFunctionIdentity::DerivedEquality(_) => None,
    }
}

fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
fn invalid(reason: impl std::fmt::Display) -> Error {
    Error::InvalidSourceDeclaration(reason.to_string())
}
