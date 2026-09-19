use super::*;

pub(super) fn write_exact_type(
    id: PersistentExactTypeId,
    graph: &impl ExactTypeDiagnosticGraph,
    output: &mut String,
    depth: usize,
) -> Result<(), ExactTypeDiagnosticError> {
    if depth > MAX_DIAGNOSTIC_RECURSION {
        return Err(ExactTypeDiagnosticError::RecursionLimit);
    }
    let key = graph
        .exact_type_key(id)
        .ok_or(ExactTypeDiagnosticError::MissingExactType(id))?;
    match key {
        ExactTypeKey::Nominal(declaration) => write_nominal(*declaration, graph, output),
        ExactTypeKey::NominalApplication { origin, arguments } => {
            let declaration = graph
                .source_generic_type_declaration(*origin)
                .ok_or(ExactTypeDiagnosticError::MissingSourceGenericType(*origin))?;
            output.push_str("a(");
            write_nominal_atom(declaration, graph, output)?;
            output.push_str(";[");
            write_sequence(arguments.as_slice(), graph, output, depth)?;
            output.push_str("])");
            Ok(())
        }
        ExactTypeKey::Tuple(elements) => {
            output.push_str("t([");
            write_sequence(elements.as_slice(), graph, output, depth)?;
            output.push_str("])");
            Ok(())
        }
        ExactTypeKey::Function {
            effect,
            parameters,
            result,
        } => write_function(
            match effect {
                Effect::Ordinary => 'o',
                Effect::Suspend => 's',
            },
            parameters,
            *result,
            graph,
            output,
            depth,
            'f',
        ),
        ExactTypeKey::RawPointer(pointee) => {
            output.push_str("r(");
            write_exact_type(*pointee, graph, output, depth + 1)?;
            output.push(')');
            Ok(())
        }
        ExactTypeKey::NativeFunctionPointer {
            calling_convention: CallingConvention::C,
            parameters,
            result,
        } => write_function('c', parameters, *result, graph, output, depth, 'x'),
    }
}

fn write_function(
    flavor: char,
    parameters: &[PersistentExactTypeId],
    result: PersistentExactTypeId,
    graph: &impl ExactTypeDiagnosticGraph,
    output: &mut String,
    depth: usize,
    prefix: char,
) -> Result<(), ExactTypeDiagnosticError> {
    output.push(prefix);
    output.push('(');
    output.push(flavor);
    output.push_str(";[");
    write_sequence(parameters, graph, output, depth)?;
    output.push_str("]->");
    write_exact_type(result, graph, output, depth + 1)?;
    output.push(')');
    Ok(())
}

fn write_sequence(
    ids: &[PersistentExactTypeId],
    graph: &impl ExactTypeDiagnosticGraph,
    output: &mut String,
    depth: usize,
) -> Result<(), ExactTypeDiagnosticError> {
    for (index, id) in ids.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write_exact_type(*id, graph, output, depth + 1)?;
    }
    Ok(())
}

pub(super) fn write_nominal_atom(
    declaration: &SourceDeclarationKey,
    graph: &impl ExactTypeDiagnosticGraph,
    output: &mut String,
) -> Result<(), ExactTypeDiagnosticError> {
    let coordinate = graph
        .cone_coordinate(declaration.origin())
        .ok_or(ExactTypeDiagnosticError::MissingCone(declaration.origin()))?;
    let kind = nominal_kind_tag(declaration.declaration_kind())?;
    let name = nominal_name(declaration)?;
    output.push_str("n(c=");
    write_escaped(coordinate.group().as_bytes(), output);
    write_escaped(b":", output);
    write_escaped(coordinate.name().as_bytes(), output);
    write_escaped(b":", output);
    write_escaped(coordinate.version().as_bytes(), output);
    output.push_str(";p=");
    for (index, segment) in declaration.package().segments().iter().enumerate() {
        if index > 0 {
            output.push('.');
        }
        write_escaped(segment.as_str().as_bytes(), output);
    }
    output.push_str(";o=");
    if declaration.owners().owners().is_empty() {
        output.push('-');
    } else {
        for (index, owner) in declaration.owners().owners().iter().enumerate() {
            if index > 0 {
                output.push('/');
            }
            let owner = match owner {
                DefinitionOwnerAtom::Type(id) => graph
                    .source_type_declaration(*id)
                    .ok_or(ExactTypeDiagnosticError::MissingSourceType(*id))?,
                DefinitionOwnerAtom::GenericType(id) => graph
                    .source_generic_type_declaration(*id)
                    .ok_or(ExactTypeDiagnosticError::MissingSourceGenericType(*id))?,
                _ => return Err(ExactTypeDiagnosticError::NonNominalOwner),
            };
            output.push(nominal_kind_tag(owner.declaration_kind())?);
            output.push(':');
            write_escaped(nominal_name(owner)?.as_str().as_bytes(), output);
        }
    }
    output.push_str(";k=");
    output.push(kind);
    output.push_str(";x=");
    write_escaped(name.as_str().as_bytes(), output);
    output.push(')');
    Ok(())
}

pub(super) fn is_unescaped(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}

fn write_escaped(bytes: &[u8], output: &mut String) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in bytes {
        if is_unescaped(*byte) {
            output.push(char::from(*byte));
        } else {
            output.push('%');
            output.push(char::from(HEX[usize::from(*byte >> 4)]));
            output.push(char::from(HEX[usize::from(*byte & 0x0f)]));
        }
    }
}
