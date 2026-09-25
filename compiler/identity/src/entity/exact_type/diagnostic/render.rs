use super::*;

enum Part<'a> {
    Type(PersistentExactTypeId),
    End(PersistentExactTypeId),
    Sequence(&'a [PersistentExactTypeId]),
    Text(&'static str),
}

pub(super) fn write_exact_type(
    id: PersistentExactTypeId,
    graph: &impl ExactTypeDiagnosticGraph,
    output: &mut NameOutput,
) -> Result<(), ExactTypeDiagnosticError> {
    let mut pending = vec![Part::Type(id)];
    let mut active = BTreeSet::new();
    while let Some(part) = pending.pop() {
        pending
            .try_reserve(5)
            .map_err(|_| ExactTypeDiagnosticError::Allocation)?;
        match part {
            Part::Text(text) => output.push_str(text)?,
            Part::End(id) => {
                active.remove(&id);
            }
            Part::Sequence(ids) => {
                if let Some((first, rest)) = ids.split_first() {
                    if !rest.is_empty() {
                        pending.push(Part::Sequence(rest));
                        pending.push(Part::Text(","));
                    }
                    pending.push(Part::Type(*first));
                }
            }
            Part::Type(id) => {
                if !active.insert(id) {
                    return Err(ExactTypeDiagnosticError::Cycle(id));
                }
                let key = graph
                    .exact_type_key(id)
                    .ok_or(ExactTypeDiagnosticError::MissingExactType(id))?;
                pending.push(Part::End(id));
                match key {
                    ExactTypeKey::Nominal(declaration) => {
                        write_nominal(*declaration, graph, output)?
                    }
                    ExactTypeKey::NominalApplication { origin, arguments } => {
                        let declaration = graph
                            .source_generic_type_declaration(*origin)
                            .ok_or(ExactTypeDiagnosticError::MissingSourceGenericType(*origin))?;
                        output.push_str("a(")?;
                        write_nominal_atom(declaration, graph, output)?;
                        output.push_str(";[")?;
                        pending.push(Part::Text("])"));
                        pending.push(Part::Sequence(arguments.as_slice()));
                    }
                    ExactTypeKey::Tuple(elements) => {
                        output.push_str("t([")?;
                        pending.push(Part::Text("])"));
                        pending.push(Part::Sequence(elements.as_slice()));
                    }
                    ExactTypeKey::RawPointer(pointee) => {
                        output.push_str("r(")?;
                        pending.push(Part::Text(")"));
                        pending.push(Part::Type(*pointee));
                    }
                    ExactTypeKey::Function {
                        effect,
                        parameters,
                        result,
                    } => {
                        output.push_str(match effect {
                            Effect::Ordinary => "f(o;[",
                            Effect::Suspend => "f(s;[",
                        })?;
                        pending.push(Part::Text(")"));
                        pending.push(Part::Type(*result));
                        pending.push(Part::Text("]->"));
                        pending.push(Part::Sequence(parameters));
                    }
                    ExactTypeKey::NativeFunctionPointer {
                        calling_convention: CallingConvention::C,
                        parameters,
                        result,
                    } => {
                        output.push_str("x(c;[")?;
                        pending.push(Part::Text(")"));
                        pending.push(Part::Type(*result));
                        pending.push(Part::Text("]->"));
                        pending.push(Part::Sequence(parameters));
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn write_nominal_atom(
    declaration: &SourceDeclarationKey,
    graph: &impl ExactTypeDiagnosticGraph,
    output: &mut NameOutput,
) -> Result<(), ExactTypeDiagnosticError> {
    let coordinate = graph
        .cone_coordinate(declaration.origin())
        .ok_or(ExactTypeDiagnosticError::MissingCone(declaration.origin()))?;
    let kind = nominal_kind_tag(declaration.declaration_kind())?;
    let name = nominal_name(declaration)?;
    output.push_str("n(c=")?;
    write_escaped(coordinate.group().as_bytes(), output)?;
    write_escaped(b":", output)?;
    write_escaped(coordinate.name().as_bytes(), output)?;
    write_escaped(b":", output)?;
    write_escaped(coordinate.version().as_bytes(), output)?;
    output.push_str(";p=")?;
    for (index, segment) in declaration.package().segments().iter().enumerate() {
        if index > 0 {
            output.push('.')?;
        }
        write_escaped(segment.as_str().as_bytes(), output)?;
    }
    output.push_str(";o=")?;
    if declaration.owners().owners().is_empty() {
        output.push('-')?;
    } else {
        for (index, owner) in declaration.owners().owners().iter().enumerate() {
            if index > 0 {
                output.push('/')?;
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
            output.push(nominal_kind_tag(owner.declaration_kind())?)?;
            output.push(':')?;
            write_escaped(nominal_name(owner)?.as_str().as_bytes(), output)?;
        }
    }
    output.push_str(";k=")?;
    output.push(kind)?;
    output.push_str(";x=")?;
    write_escaped(name.as_str().as_bytes(), output)?;
    output.push(')')?;
    Ok(())
}

pub(super) fn is_unescaped(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}

fn write_escaped(bytes: &[u8], output: &mut NameOutput) -> Result<(), ExactTypeDiagnosticError> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in bytes {
        if is_unescaped(*byte) {
            output.push(char::from(*byte))?;
        } else {
            output.push('%')?;
            output.push(char::from(HEX[usize::from(*byte >> 4)]))?;
            output.push(char::from(HEX[usize::from(*byte & 0x0f)]))?;
        }
    }
    Ok(())
}
