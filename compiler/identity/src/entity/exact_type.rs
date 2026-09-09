use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{
    CallingConvention, DeclarationName, DefinitionOwnerAtom, Effect, NonEmptyVec,
    SourceDeclarationKey, SourceDeclarationKind,
};
use crate::ids::derive_persistent_id;
use crate::{
    ConeCoordinate, ConeIdentity, PersistentExactTypeId, PersistentGenericTypeId, PersistentTypeId,
};

mod decode;

pub use decode::{DecodedExactTypeKey, ExactTypeResolutionError};

const MAX_DIAGNOSTIC_NAME_BYTES: usize = 16 * 1024 * 1024;
const MAX_DIAGNOSTIC_RECURSION: usize = 1_024;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactTypeKey {
    Nominal(PersistentTypeId),
    NominalApplication {
        origin: PersistentGenericTypeId,
        arguments: NonEmptyVec<PersistentExactTypeId>,
    },
    Tuple(NonEmptyVec<PersistentExactTypeId>),
    Function {
        effect: Effect,
        parameters: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
    },
    RawPointer(PersistentExactTypeId),
    NativeFunctionPointer {
        calling_convention: CallingConvention,
        parameters: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
    },
}

impl WireEncode for ExactTypeKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal(id) => encode_single_payload(encoder, 1, id),
            Self::NominalApplication { origin, arguments } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                origin.encode(encoder)?;
                encoder.field(2)?;
                encode_ids(encoder, arguments.as_slice())
            }
            Self::Tuple(elements) => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_ids(encoder, elements.as_slice())
            }
            Self::Function {
                effect,
                parameters,
                result,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                effect.encode(encoder)?;
                encoder.field(2)?;
                encode_ids(encoder, parameters)?;
                encoder.field(3)?;
                result.encode(encoder)
            }
            Self::RawPointer(pointee) => encode_single_payload(encoder, 5, pointee),
            Self::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 6)?;
                encoder.field(1)?;
                calling_convention.encode(encoder)?;
                encoder.field(2)?;
                encode_ids(encoder, parameters)?;
                encoder.field(3)?;
                result.encode(encoder)
            }
        }
    }
}

impl PersistentExactTypeId {
    pub fn from_key(key: &ExactTypeKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-exact-type-v1", key)
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_single_payload(
    encoder: &mut Encoder,
    tag: u64,
    id: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    id.encode(encoder)
}

fn encode_ids(
    encoder: &mut Encoder,
    ids: &[PersistentExactTypeId],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(ids.len() as u64)?;
    for id in ids {
        id.encode(encoder)?;
    }
    Ok(())
}

/// Read-only access to an already validated exact-type identity graph.
pub trait ExactTypeDiagnosticGraph {
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Option<&ExactTypeKey>;

    fn source_type_declaration(&self, id: PersistentTypeId) -> Option<&SourceDeclarationKey>;

    fn source_generic_type_declaration(
        &self,
        id: PersistentGenericTypeId,
    ) -> Option<&SourceDeclarationKey>;

    fn cone_coordinate(&self, id: ConeIdentity) -> Option<&ConeCoordinate>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExactTypeDiagnosticName(String);

impl CanonicalExactTypeDiagnosticName {
    pub fn from_validated_graph(
        root: PersistentExactTypeId,
        graph: &impl ExactTypeDiagnosticGraph,
    ) -> Result<Self, ExactTypeDiagnosticError> {
        let mut costs = BTreeMap::new();
        let mut active = BTreeSet::new();
        let cost = exact_type_cost(root, graph, &mut costs, &mut active, 1)?;
        if cost > MAX_DIAGNOSTIC_NAME_BYTES {
            return Err(ExactTypeDiagnosticError::NameTooLong {
                limit: MAX_DIAGNOSTIC_NAME_BYTES,
                observed: cost,
            });
        }
        let mut output = String::new();
        output
            .try_reserve_exact(cost)
            .map_err(|_| ExactTypeDiagnosticError::Allocation)?;
        write_exact_type(root, graph, &mut output, 1)?;
        if output.len() != cost {
            return Err(ExactTypeDiagnosticError::GraphChangedDuringPrint);
        }
        Ok(Self(output))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CanonicalExactTypeDiagnosticName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactTypeDiagnosticError {
    MissingExactType(PersistentExactTypeId),
    MissingSourceType(PersistentTypeId),
    MissingSourceGenericType(PersistentGenericTypeId),
    MissingCone(ConeIdentity),
    NonNominalDeclaration,
    ConstructorUsedAsNominalName,
    NonNominalOwner,
    Cycle(PersistentExactTypeId),
    RecursionLimit,
    LengthOverflow,
    NameTooLong { limit: usize, observed: usize },
    Allocation,
    GraphChangedDuringPrint,
}

impl fmt::Display for ExactTypeDiagnosticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingExactType(id) => write!(formatter, "missing exact type {id}"),
            Self::MissingSourceType(id) => write!(formatter, "missing source type {id}"),
            Self::MissingSourceGenericType(id) => {
                write!(formatter, "missing source generic type {id}")
            }
            Self::MissingCone(id) => write!(formatter, "missing Cone coordinate for {id}"),
            Self::NonNominalDeclaration => {
                formatter.write_str("exact nominal type refers to a non-nominal declaration")
            }
            Self::ConstructorUsedAsNominalName => {
                formatter.write_str("nominal declaration has a constructor name")
            }
            Self::NonNominalOwner => {
                formatter.write_str("nominal declaration has a non-nominal owner")
            }
            Self::Cycle(id) => write!(formatter, "cycle in exact type graph at {id}"),
            Self::RecursionLimit => {
                formatter.write_str("exact type diagnostic recursion limit exceeded")
            }
            Self::LengthOverflow => formatter.write_str("exact type diagnostic length overflow"),
            Self::NameTooLong { limit, observed } => {
                write!(
                    formatter,
                    "exact type diagnostic name has {observed} bytes, limit is {limit}"
                )
            }
            Self::Allocation => {
                formatter.write_str("failed to allocate exact type diagnostic name")
            }
            Self::GraphChangedDuringPrint => {
                formatter.write_str("validated exact type graph changed while printing")
            }
        }
    }
}

impl std::error::Error for ExactTypeDiagnosticError {}

fn exact_type_cost(
    id: PersistentExactTypeId,
    graph: &impl ExactTypeDiagnosticGraph,
    costs: &mut BTreeMap<PersistentExactTypeId, usize>,
    active: &mut BTreeSet<PersistentExactTypeId>,
    depth: usize,
) -> Result<usize, ExactTypeDiagnosticError> {
    if depth > MAX_DIAGNOSTIC_RECURSION {
        return Err(ExactTypeDiagnosticError::RecursionLimit);
    }
    if let Some(cost) = costs.get(&id) {
        return Ok(*cost);
    }
    if !active.insert(id) {
        return Err(ExactTypeDiagnosticError::Cycle(id));
    }
    let key = graph
        .exact_type_key(id)
        .ok_or(ExactTypeDiagnosticError::MissingExactType(id))?;
    let cost = match key {
        ExactTypeKey::Nominal(declaration) => {
            let declaration = graph
                .source_type_declaration(*declaration)
                .ok_or(ExactTypeDiagnosticError::MissingSourceType(*declaration))?;
            nominal_atom_cost(declaration, graph)?
        }
        ExactTypeKey::NominalApplication { origin, arguments } => {
            let declaration = graph
                .source_generic_type_declaration(*origin)
                .ok_or(ExactTypeDiagnosticError::MissingSourceGenericType(*origin))?;
            let mut cost = checked_add(4, nominal_atom_cost(declaration, graph)?)?;
            cost = checked_add(
                cost,
                sequence_cost(arguments.as_slice(), graph, costs, active, depth)?,
            )?;
            checked_add(cost, 2)?
        }
        ExactTypeKey::Tuple(elements) => checked_add(
            5,
            sequence_cost(elements.as_slice(), graph, costs, active, depth)?,
        )?,
        ExactTypeKey::Function {
            parameters, result, ..
        }
        | ExactTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            let mut cost = checked_add(5, sequence_cost(parameters, graph, costs, active, depth)?)?;
            cost = checked_add(
                cost,
                exact_type_cost(*result, graph, costs, active, depth + 1)?,
            )?;
            checked_add(cost, 4)?
        }
        ExactTypeKey::RawPointer(pointee) => checked_add(
            3,
            exact_type_cost(*pointee, graph, costs, active, depth + 1)?,
        )?,
    };
    active.remove(&id);
    costs.insert(id, cost);
    Ok(cost)
}

fn sequence_cost(
    ids: &[PersistentExactTypeId],
    graph: &impl ExactTypeDiagnosticGraph,
    costs: &mut BTreeMap<PersistentExactTypeId, usize>,
    active: &mut BTreeSet<PersistentExactTypeId>,
    depth: usize,
) -> Result<usize, ExactTypeDiagnosticError> {
    let mut cost = 0usize;
    for (index, id) in ids.iter().enumerate() {
        if index > 0 {
            cost = checked_add(cost, 1)?;
        }
        cost = checked_add(cost, exact_type_cost(*id, graph, costs, active, depth + 1)?)?;
    }
    Ok(cost)
}

fn nominal_atom_cost(
    declaration: &SourceDeclarationKey,
    graph: &impl ExactTypeDiagnosticGraph,
) -> Result<usize, ExactTypeDiagnosticError> {
    let kind = nominal_kind_tag(declaration.declaration_kind())?;
    let name = nominal_name(declaration)?;
    let coordinate = graph
        .cone_coordinate(declaration.origin())
        .ok_or(ExactTypeDiagnosticError::MissingCone(declaration.origin()))?;
    let mut cost = "n(c=".len();
    cost = checked_add(cost, coordinate_escaped_cost(coordinate)?)?;
    cost = checked_add(cost, ";p=".len())?;
    cost = checked_add(cost, package_escaped_cost(declaration)?)?;
    cost = checked_add(cost, ";o=".len())?;
    cost = checked_add(cost, owner_chain_cost(declaration, graph)?)?;
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
) -> Result<usize, ExactTypeDiagnosticError> {
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

fn checked_add(left: usize, right: usize) -> Result<usize, ExactTypeDiagnosticError> {
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

fn nominal_kind_tag(kind: SourceDeclarationKind) -> Result<char, ExactTypeDiagnosticError> {
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

fn nominal_name(
    declaration: &SourceDeclarationKey,
) -> Result<&crate::CanonicalIdentifier, ExactTypeDiagnosticError> {
    match declaration.name() {
        DeclarationName::Named(name) => Ok(name),
        DeclarationName::Constructor => Err(ExactTypeDiagnosticError::ConstructorUsedAsNominalName),
    }
}

fn write_exact_type(
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
        ExactTypeKey::Nominal(declaration) => {
            let declaration = graph
                .source_type_declaration(*declaration)
                .ok_or(ExactTypeDiagnosticError::MissingSourceType(*declaration))?;
            write_nominal_atom(declaration, graph, output)
        }
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

fn write_nominal_atom(
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

fn is_unescaped(byte: u8) -> bool {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use scoop_wire::encode;

    use super::{CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticGraph, ExactTypeKey};
    use crate::{
        CanonicalIdentifier, ConeCoordinate, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
        Effect, NonEmptyVec, PackagePath, PersistentExactTypeId, PersistentGenericTypeId,
        PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };

    #[derive(Default)]
    struct Graph {
        exact: BTreeMap<PersistentExactTypeId, ExactTypeKey>,
        types: BTreeMap<PersistentTypeId, SourceDeclarationKey>,
        generic_types: BTreeMap<PersistentGenericTypeId, SourceDeclarationKey>,
        cones: BTreeMap<ConeIdentity, ConeCoordinate>,
    }

    impl ExactTypeDiagnosticGraph for Graph {
        fn exact_type_key(&self, id: PersistentExactTypeId) -> Option<&ExactTypeKey> {
            self.exact.get(&id)
        }

        fn source_type_declaration(&self, id: PersistentTypeId) -> Option<&SourceDeclarationKey> {
            self.types.get(&id)
        }

        fn source_generic_type_declaration(
            &self,
            id: PersistentGenericTypeId,
        ) -> Option<&SourceDeclarationKey> {
            self.generic_types.get(&id)
        }

        fn cone_coordinate(&self, id: ConeIdentity) -> Option<&ConeCoordinate> {
            self.cones.get(&id)
        }
    }

    fn nominal_graph() -> (Graph, PersistentExactTypeId) {
        let coordinate = ConeCoordinate::new("org.example", "demo", "1.2.3").unwrap();
        let cone = coordinate.identity().unwrap();
        let declaration = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                cone,
                PackagePath::from_segments(vec![CanonicalIdentifier::new("app").unwrap()]),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("User").unwrap(),
            SourceNominalKind::Class,
            0,
        );
        let type_id = PersistentTypeId::from_source_declaration(&declaration).unwrap();
        let exact_key = ExactTypeKey::Nominal(type_id);
        let exact_id = PersistentExactTypeId::from_key(&exact_key).unwrap();
        let mut graph = Graph::default();
        graph.cones.insert(cone, coordinate);
        graph.types.insert(type_id, declaration);
        graph.exact.insert(exact_id, exact_key);
        (graph, exact_id)
    }

    #[test]
    fn exact_type_variants_have_distinct_fixed_wire_and_hash() {
        let (mut graph, nominal) = nominal_graph();
        let tuple_key = ExactTypeKey::Tuple(NonEmptyVec::from_first(nominal, [nominal]));
        let tuple = PersistentExactTypeId::from_key(&tuple_key).unwrap();
        graph.exact.insert(tuple, tuple_key.clone());
        let function_key = ExactTypeKey::Function {
            effect: Effect::Suspend,
            parameters: vec![nominal],
            result: tuple,
        };
        let function = PersistentExactTypeId::from_key(&function_key).unwrap();
        graph.exact.insert(function, function_key);

        assert_eq!(
            hex(&encode(&tuple_key).unwrap()),
            format!("a2000301825820{}5820{}", nominal, nominal)
        );
        assert_eq!(
            tuple.to_string(),
            "8567e8450c021da219bdedb25eed21433bacff159b424ed4bd45bcc7cd95135d"
        );
        assert_ne!(tuple, function);
    }

    #[test]
    fn all_six_exact_type_variants_have_fixed_wire_shapes() {
        let type_id = PersistentTypeId(ConeIdentity::CORE.0);
        let generic_id = PersistentGenericTypeId(ConeIdentity::SINGLE_FILE.0);
        let child = PersistentExactTypeId(ConeIdentity::CORE.0);
        let vectors = [
            (
                ExactTypeKey::Nominal(type_id),
                format!("a20001015820{type_id}"),
            ),
            (
                ExactTypeKey::NominalApplication {
                    origin: generic_id,
                    arguments: NonEmptyVec::from_first(child, []),
                },
                format!("a30002015820{generic_id}02815820{child}"),
            ),
            (
                ExactTypeKey::Tuple(NonEmptyVec::from_first(child, [])),
                format!("a2000301815820{child}"),
            ),
            (
                ExactTypeKey::Function {
                    effect: Effect::Ordinary,
                    parameters: vec![child],
                    result: child,
                },
                format!("a40004010102815820{child}035820{child}"),
            ),
            (
                ExactTypeKey::RawPointer(child),
                format!("a20005015820{child}"),
            ),
            (
                ExactTypeKey::NativeFunctionPointer {
                    calling_convention: crate::CallingConvention::C,
                    parameters: vec![child],
                    result: child,
                },
                format!("a40006010102815820{child}035820{child}"),
            ),
        ];
        for (key, expected) in vectors {
            assert_eq!(hex(&encode(&key).unwrap()), expected);
        }
    }

    #[test]
    fn canonical_diagnostic_name_uses_only_identity_graph_spelling() {
        let (mut graph, nominal) = nominal_graph();
        let tuple_key = ExactTypeKey::Tuple(NonEmptyVec::from_first(nominal, [nominal]));
        let tuple = PersistentExactTypeId::from_key(&tuple_key).unwrap();
        graph.exact.insert(tuple, tuple_key);
        let function_key = ExactTypeKey::Function {
            effect: Effect::Suspend,
            parameters: vec![nominal],
            result: tuple,
        };
        let function = PersistentExactTypeId::from_key(&function_key).unwrap();
        graph.exact.insert(function, function_key);

        let atom = "n(c=org.example%3Ademo%3A1.2.3;p=app;o=-;k=C;x=User)";
        assert_eq!(
            CanonicalExactTypeDiagnosticName::from_validated_graph(function, &graph)
                .unwrap()
                .as_str(),
            format!("f(s;[{atom}]->t([{atom},{atom}]))")
        );
    }

    #[test]
    fn missing_child_is_a_typed_error_instead_of_a_panic() {
        let (mut graph, nominal) = nominal_graph();
        let pointer_key = ExactTypeKey::RawPointer(nominal);
        let pointer = PersistentExactTypeId::from_key(&pointer_key).unwrap();
        graph.exact.remove(&nominal);
        graph.exact.insert(pointer, pointer_key);
        assert!(CanonicalExactTypeDiagnosticName::from_validated_graph(pointer, &graph).is_err());
    }

    #[test]
    fn shared_dag_cost_is_recounted_before_any_large_allocation() {
        let (mut graph, mut root) = nominal_graph();
        for _ in 0..20 {
            let key = ExactTypeKey::Tuple(NonEmptyVec::from_first(root, [root]));
            let id = PersistentExactTypeId::from_key(&key).unwrap();
            graph.exact.insert(id, key);
            root = id;
        }
        assert!(matches!(
            CanonicalExactTypeDiagnosticName::from_validated_graph(root, &graph),
            Err(super::ExactTypeDiagnosticError::NameTooLong { .. })
        ));
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
