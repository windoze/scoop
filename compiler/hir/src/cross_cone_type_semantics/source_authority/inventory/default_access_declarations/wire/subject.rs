use super::*;

pub(super) fn decode(
    decoder: &mut Decoder<'_, '_>,
) -> Result<DecodedDefinitionOriginSubject, WireError> {
    use DecodedDefinitionOriginSubject::*;
    let subject = DecodedDefinitionOriginSubject::decode(decoder)?;
    let tag = match subject {
        Type(_) | GenericType(_) | Function(_) | GenericFunction(_) | Constructor(_)
        | Property(_) | ExtensionProperty(_) | PropertyAccessor(_) => return Ok(subject),
        TypeAlias(_) => 9,
        Field(_) => 10,
        EnumVariant(_) => 11,
        EnumVariantField(_) => 12,
        GeneratedCallable(_) => 13,
        InitializationUnit(_) => 14,
        LocalBinding(_) => 15,
        LocalValue(_) => 16,
        CallbackRegistration(_) => 17,
        SourceNativeContract(_) => 18,
    };
    Err(WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    ))
}
