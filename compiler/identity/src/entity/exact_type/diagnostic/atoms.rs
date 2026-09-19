use super::render::is_unescaped;
use super::*;

pub(super) fn nominal_atom_cost(
    declaration: &SourceDeclarationKey,
    graph: &impl ExactTypeDiagnosticGraph,
    meter: &mut BudgetMeter,
) -> Result<usize, ExactTypeDiagnosticError> {
    let kind = nominal_kind_tag(declaration.declaration_kind())?;
    let name = nominal_name(declaration)?;
    let coordinate = graph
        .cone_coordinate(declaration.origin())
        .ok_or(ExactTypeDiagnosticError::MissingCone(declaration.origin()))?;
    let path = WirePath::root();
    meter.charge_work(
        (name.as_str().len() as u64)
            .saturating_add(coordinate.group().len() as u64)
            .saturating_add(coordinate.name().len() as u64)
            .saturating_add(coordinate.version().len() as u64)
            .saturating_add(declaration.package().segments().len() as u64),
        &path,
    )?;
    for segment in declaration.package().segments() {
        meter.charge_work(segment.as_str().len() as u64, &path)?;
    }
    let mut cost = "n(c=".len();
    cost = checked_add(cost, coordinate_escaped_cost(coordinate)?)?;
    cost = checked_add(cost, ";p=".len())?;
    cost = checked_add(cost, package_escaped_cost(declaration)?)?;
    cost = checked_add(cost, ";o=".len())?;
    cost = checked_add(cost, owner_chain_cost(declaration, graph, meter)?)?;
    cost = checked_add(cost, ";k=".len())?;
    cost = checked_add(cost, kind.len_utf8())?;
    cost = checked_add(cost, ";x=".len())?;
    cost = checked_add(cost, escaped_cost(name.as_str().as_bytes())?)?;
    checked_add(cost, 1)
}

fn coordinate_escaped_cost(coordinate: &ConeCoordinate) -> Result<usize, ExactTypeDiagnosticError> {
    let mut cost = escaped_cost(coordinate.group().as_bytes())?;
    cost = checked_add(cost, 3)?;
    cost = checked_add(cost, escaped_cost(coordinate.name().as_bytes())?)?;
    cost = checked_add(cost, 3)?;
    checked_add(cost, escaped_cost(coordinate.version().as_bytes())?)
}

fn package_escaped_cost(
    declaration: &SourceDeclarationKey,
) -> Result<usize, ExactTypeDiagnosticError> {
    let mut cost = 0usize;
    for (index, segment) in declaration.package().segments().iter().enumerate() {
        if index > 0 {
            cost = checked_add(cost, 1)?;
        }
        cost = checked_add(cost, escaped_cost(segment.as_str().as_bytes())?)?;
    }
    Ok(cost)
}

fn owner_chain_cost(
    declaration: &SourceDeclarationKey,
    graph: &impl ExactTypeDiagnosticGraph,
    meter: &mut BudgetMeter,
) -> Result<usize, ExactTypeDiagnosticError> {
    meter.charge_work(
        declaration.owners().owners().len() as u64,
        &WirePath::root(),
    )?;
    if declaration.owners().owners().is_empty() {
        return Ok(1);
    }
    let mut cost = 0usize;
    for (index, owner) in declaration.owners().owners().iter().enumerate() {
        if index > 0 {
            cost = checked_add(cost, 1)?;
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
        nominal_kind_tag(owner.declaration_kind())?;
        let name = nominal_name(owner)?;
        meter.charge_work(name.as_str().len() as u64, &WirePath::root())?;
        cost = checked_add(cost, 2)?;
        cost = checked_add(cost, escaped_cost(name.as_str().as_bytes())?)?;
    }
    Ok(cost)
}

fn escaped_cost(bytes: &[u8]) -> Result<usize, ExactTypeDiagnosticError> {
    let mut cost = 0usize;
    for byte in bytes {
        cost = checked_add(cost, if is_unescaped(*byte) { 1 } else { 3 })?;
    }
    Ok(cost)
}

pub(super) fn checked_add(left: usize, right: usize) -> Result<usize, ExactTypeDiagnosticError> {
    let sum = left
        .checked_add(right)
        .ok_or(ExactTypeDiagnosticError::LengthOverflow)?;
    if sum > MAX_DIAGNOSTIC_NAME_BYTES {
        Err(ExactTypeDiagnosticError::NameTooLong {
            limit: MAX_DIAGNOSTIC_NAME_BYTES,
            observed: sum,
        })
    } else {
        Ok(sum)
    }
}

pub(super) fn nominal_kind_tag(
    kind: SourceDeclarationKind,
) -> Result<char, ExactTypeDiagnosticError> {
    match kind {
        SourceDeclarationKind::Class => Ok('C'),
        SourceDeclarationKind::Interface => Ok('I'),
        SourceDeclarationKind::Struct => Ok('S'),
        SourceDeclarationKind::Enum => Ok('E'),
        SourceDeclarationKind::Object => Ok('O'),
        SourceDeclarationKind::AnnotationClass => Ok('A'),
        _ => Err(ExactTypeDiagnosticError::NonNominalDeclaration),
    }
}

pub(super) fn nominal_name(
    declaration: &SourceDeclarationKey,
) -> Result<&crate::CanonicalIdentifier, ExactTypeDiagnosticError> {
    match declaration.name() {
        DeclarationName::Named(name) => Ok(name),
        DeclarationName::Constructor => Err(ExactTypeDiagnosticError::ConstructorUsedAsNominalName),
    }
}
