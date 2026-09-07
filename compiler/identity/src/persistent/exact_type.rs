//! Exact type identity and the canonical diagnostic name printer
//! (DESIGN section 3.1).
//!
//! An exact type is the closed structural identity used across Cones for
//! signatures, specializations and runtime type registration. The key is
//! hashed to a `PersistentExactTypeId`; the diagnostic name is derived
//! exclusively from the verified key plus the declaration atoms by a
//! fixed ASCII printer.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use crate::{CborWriter, ConeCoordinate, Digest256, DomainHasher};

pub const EXACT_TYPE_DOMAIN: &[u8] = b"scoop-exact-type-v1";

/// Upper bound for one canonical diagnostic name (matches the single
/// semantic text/bytes field budget of `.slib` v1).
pub const DIAGNOSTIC_NAME_MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedFunctionEffect {
    Ordinary,
    Suspend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCallingConvention {
    /// v1's only convention; new ones require a new stable tag.
    C,
}

/// Compiler-generated nominal roles frozen by DESIGN 3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedNominalRole {
    ClosureEnvironment = 1,
    CallableAdapterEnvironment = 2,
    CoroutineFrame = 3,
    ContinuationAdapterEnvironment = 4,
    CoroutineStep = 5,
}

impl GeneratedNominalRole {
    pub fn tag(self) -> u64 {
        self as u64
    }
}

/// The structural identity of one concrete exact type:
/// `SHA-256("scoop-exact-type-v1" || canonical key)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PersistentExactTypeId(Digest256);

impl PersistentExactTypeId {
    pub fn of(key: &ExactTypeKey) -> Self {
        let digest = DomainHasher::new(EXACT_TYPE_DOMAIN)
            .field(&key.canonical_cbor())
            .finish();
        PersistentExactTypeId(digest)
    }

    pub fn from_validated(bytes: [u8; 32]) -> Self {
        PersistentExactTypeId(Digest256::from_bytes(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for PersistentExactTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PersistentExactTypeId({})", self.0)
    }
}

impl fmt::Display for PersistentExactTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The closed exact-type key. Collection shapes are enforced by the
/// checked constructors and by decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactTypeKey {
    Nominal {
        declaration: crate::persistent::PersistentTypeId,
    },
    GeneratedNominal {
        role: GeneratedNominalRole,
        identity: [u8; 32],
    },
    NominalApplication {
        origin: crate::persistent::PersistentGenericTypeId,
        /// Always non-empty.
        arguments: Vec<PersistentExactTypeId>,
    },
    Tuple {
        /// Always non-empty.
        elements: Vec<PersistentExactTypeId>,
    },
    Function {
        effect: ManagedFunctionEffect,
        parameters: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
    },
    RawPointer {
        pointee: PersistentExactTypeId,
    },
    NativeFunctionPointer {
        calling_convention: NativeCallingConvention,
        parameters: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
    },
}

impl ExactTypeKey {
    pub fn application(
        origin: crate::persistent::PersistentGenericTypeId,
        arguments: Vec<PersistentExactTypeId>,
    ) -> Result<Self, ExactTypeError> {
        if arguments.is_empty() {
            return Err(ExactTypeError::EmptyArguments);
        }
        Ok(ExactTypeKey::NominalApplication { origin, arguments })
    }

    pub fn tuple(elements: Vec<PersistentExactTypeId>) -> Result<Self, ExactTypeError> {
        if elements.is_empty() {
            return Err(ExactTypeError::EmptyTuple);
        }
        Ok(ExactTypeKey::Tuple { elements })
    }

    fn variant_tag(&self) -> u64 {
        match self {
            ExactTypeKey::Nominal { .. } => 1,
            ExactTypeKey::GeneratedNominal { .. } => 2,
            ExactTypeKey::NominalApplication { .. } => 3,
            ExactTypeKey::Tuple { .. } => 4,
            ExactTypeKey::Function { .. } => 5,
            ExactTypeKey::RawPointer { .. } => 6,
            ExactTypeKey::NativeFunctionPointer { .. } => 7,
        }
    }

    /// Canonical CBOR: variant tag at key 0, payload fields by variant.
    pub fn canonical_cbor(&self) -> Vec<u8> {
        let mut writer = CborWriter::new();
        match self {
            ExactTypeKey::Nominal { declaration } => {
                writer.map(2);
                writer.field(0).unsigned(self.variant_tag());
                writer.field(1).bytes(declaration.as_bytes());
            }
            ExactTypeKey::GeneratedNominal { role, identity } => {
                writer.map(3);
                writer.field(0).unsigned(self.variant_tag());
                writer.field(1).unsigned(role.tag());
                writer.field(2).bytes(identity);
            }
            ExactTypeKey::NominalApplication { origin, arguments } => {
                writer.map(3);
                writer.field(0).unsigned(self.variant_tag());
                writer.field(1).bytes(origin.as_bytes());
                writer.field(2).array(arguments.len() as u64);
                for argument in arguments {
                    writer.bytes(argument.as_bytes());
                }
            }
            ExactTypeKey::Tuple { elements } => {
                writer.map(2);
                writer.field(0).unsigned(self.variant_tag());
                writer.field(1).array(elements.len() as u64);
                for element in elements {
                    writer.bytes(element.as_bytes());
                }
            }
            ExactTypeKey::Function {
                effect,
                parameters,
                result,
            } => {
                writer.map(4);
                writer.field(0).unsigned(self.variant_tag());
                writer.field(1).unsigned(match effect {
                    ManagedFunctionEffect::Ordinary => 1,
                    ManagedFunctionEffect::Suspend => 2,
                });
                writer.field(2).array(parameters.len() as u64);
                for parameter in parameters {
                    writer.bytes(parameter.as_bytes());
                }
                writer.field(3).bytes(result.as_bytes());
            }
            ExactTypeKey::RawPointer { pointee } => {
                writer.map(2);
                writer.field(0).unsigned(self.variant_tag());
                writer.field(1).bytes(pointee.as_bytes());
            }
            ExactTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => {
                writer.map(4);
                writer.field(0).unsigned(self.variant_tag());
                writer.field(1).unsigned(match calling_convention {
                    NativeCallingConvention::C => 1,
                });
                writer.field(2).array(parameters.len() as u64);
                for parameter in parameters {
                    writer.bytes(parameter.as_bytes());
                }
                writer.field(3).bytes(result.as_bytes());
            }
        }
        writer.into_bytes()
    }

    /// Direct child references in key order.
    pub fn children(&self) -> Vec<PersistentExactTypeId> {
        match self {
            ExactTypeKey::Nominal { .. } | ExactTypeKey::GeneratedNominal { .. } => Vec::new(),
            ExactTypeKey::NominalApplication { arguments, .. } => arguments.clone(),
            ExactTypeKey::Tuple { elements } => elements.clone(),
            ExactTypeKey::Function {
                parameters, result, ..
            } => {
                let mut children = parameters.clone();
                children.push(*result);
                children
            }
            ExactTypeKey::RawPointer { pointee } => vec![*pointee],
            ExactTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                let mut children = parameters.clone();
                children.push(*result);
                children
            }
        }
    }
}

/// An interned exact-type table: id → verified key.
#[derive(Debug, Default, Clone)]
pub struct ExactTypeTable {
    entries: BTreeMap<[u8; 32], ExactTypeKey>,
}

impl ExactTypeTable {
    pub fn new() -> Self {
        ExactTypeTable::default()
    }

    /// Interns a key whose children must already be present, computing
    /// and returning the id.
    pub fn intern(&mut self, key: ExactTypeKey) -> Result<PersistentExactTypeId, ExactTypeError> {
        for child in key.children() {
            if !self.entries.contains_key(child.as_bytes()) {
                return Err(ExactTypeError::MissingChild {
                    child: child.to_string(),
                });
            }
        }
        let id = PersistentExactTypeId::of(&key);
        match self.entries.entry(*id.as_bytes()) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(key);
                Ok(id)
            }
            std::collections::btree_map::Entry::Occupied(slot) => {
                if slot.get() == &key {
                    Ok(id)
                } else {
                    Err(ExactTypeError::IdCollision { id: id.to_string() })
                }
            }
        }
    }

    pub fn get(&self, id: &PersistentExactTypeId) -> Option<&ExactTypeKey> {
        self.entries.get(id.as_bytes())
    }

    /// Test/producer hook for inserting a pre-verified entry.
    pub fn entries_mut(&mut self) -> &mut BTreeMap<[u8; 32], ExactTypeKey> {
        &mut self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Full validation: every id equals the hash of its key, every child
    /// exists, and the reference graph is acyclic.
    pub fn validate(&self) -> Result<(), ExactTypeError> {
        for (raw, key) in &self.entries {
            if raw != PersistentExactTypeId::of(key).as_bytes() {
                return Err(ExactTypeError::IdMismatch {
                    id: Digest256::from_bytes(*raw).to_hex(),
                });
            }
            for child in key.children() {
                if !self.entries.contains_key(child.as_bytes()) {
                    return Err(ExactTypeError::MissingChild {
                        child: child.to_string(),
                    });
                }
            }
        }
        #[derive(Clone, Copy, PartialEq)]
        enum Color {
            White,
            Gray,
            Black,
        }
        let mut colors: BTreeMap<[u8; 32], Color> =
            self.entries.keys().map(|id| (*id, Color::White)).collect();
        let starts: Vec<[u8; 32]> = self.entries.keys().copied().collect();
        for start in starts {
            if colors[&start] != Color::White {
                continue;
            }
            let mut stack: Vec<([u8; 32], usize)> = vec![(start, 0)];
            colors.insert(start, Color::Gray);
            while let Some(current) = stack.last().copied() {
                let children = self.entries[&current.0].children();
                if current.1 >= children.len() {
                    colors.insert(current.0, Color::Black);
                    stack.pop();
                    continue;
                }
                let child = *children[current.1].as_bytes();
                stack.last_mut().expect("current exists").1 += 1;
                match colors[&child] {
                    Color::White => {
                        colors.insert(child, Color::Gray);
                        stack.push((child, 0));
                    }
                    Color::Gray => return Err(ExactTypeError::Cycle),
                    Color::Black => {}
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactTypeError {
    EmptyArguments,
    EmptyTuple,
    MissingChild { child: String },
    IdCollision { id: String },
    IdMismatch { id: String },
    Cycle,
    NameTooLarge(usize),
    NameCycle,
}

impl fmt::Display for ExactTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExactTypeError::EmptyArguments => write!(f, "exact type application needs arguments"),
            ExactTypeError::EmptyTuple => write!(f, "tuple exact type needs elements"),
            ExactTypeError::MissingChild { child } => {
                write!(f, "exact type references unregistered child {child}")
            }
            ExactTypeError::IdCollision { id } => {
                write!(f, "exact type id {id} already maps to a different key")
            }
            ExactTypeError::IdMismatch { id } => {
                write!(f, "exact type id {id} does not match its key")
            }
            ExactTypeError::Cycle => write!(f, "exact type reference graph has a cycle"),
            ExactTypeError::NameTooLarge(size) => write!(
                f,
                "canonical diagnostic name of {size} bytes exceeds the {DIAGNOSTIC_NAME_MAX_BYTES}-byte budget"
            ),
            ExactTypeError::NameCycle => write!(f, "exact type graph has a cycle while printing"),
        }
    }
}

impl std::error::Error for ExactTypeError {}

/// Nominal kind tags of the diagnostic grammar (DESIGN 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NominalKindTag {
    Class,
    Interface,
    Struct,
    Enum,
    Object,
    AnnotationClass,
}

impl NominalKindTag {
    fn letter(self) -> char {
        match self {
            NominalKindTag::Class => 'C',
            NominalKindTag::Interface => 'I',
            NominalKindTag::Struct => 'S',
            NominalKindTag::Enum => 'E',
            NominalKindTag::Object => 'O',
            NominalKindTag::AnnotationClass => 'A',
        }
    }
}

/// One owner step of a nominal atom: kind letter plus source name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomOwnerStep {
    pub kind: NominalKindTag,
    pub name: String,
}

/// The declaration data a nominal atom contributes to names. Sourced
/// from the verified declaration, never from use sites or aliases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NominalAtom {
    pub coordinate: ConeCoordinate,
    pub package: String,
    pub owner: Vec<AtomOwnerStep>,
    pub kind: NominalKindTag,
    pub name: String,
}

/// Everything the canonical printer needs.
pub struct DiagnosticNameContext<'a> {
    pub exact: &'a ExactTypeTable,
    /// Source nominal declarations keyed by `PersistentTypeId`.
    pub nominal_atoms: &'a BTreeMap<[u8; 32], NominalAtom>,
    /// Generic origin declarations (printed as nominal atoms) keyed by
    /// `PersistentGenericTypeId`.
    pub generic_atoms: &'a BTreeMap<[u8; 32], NominalAtom>,
}

impl DiagnosticNameContext<'_> {
    /// Prints the canonical diagnostic name of one exact type.
    pub fn name(&self, id: &PersistentExactTypeId) -> Result<String, ExactTypeError> {
        let mut printer = Printer {
            context: self,
            names: BTreeMap::new(),
            costs: BTreeMap::new(),
            stack: BTreeSet::new(),
        };
        let bytes = printer.name_bytes(id)?;
        String::from_utf8(bytes).map_err(|_| ExactTypeError::NameCycle)
    }
}

struct Printer<'a, 'b> {
    context: &'a DiagnosticNameContext<'b>,
    names: BTreeMap<[u8; 32], Vec<u8>>,
    costs: BTreeMap<[u8; 32], u64>,
    stack: BTreeSet<[u8; 32]>,
}

impl Printer<'_, '_> {
    /// Returns the memoized printed bytes; cost is checked per path.
    fn name_bytes(&mut self, id: &PersistentExactTypeId) -> Result<Vec<u8>, ExactTypeError> {
        let raw = *id.as_bytes();
        if let Some(name) = self.names.get(&raw) {
            return Ok(name.clone());
        }
        if !self.stack.insert(raw) {
            return Err(ExactTypeError::NameCycle);
        }
        let key = self
            .context
            .exact
            .get(id)
            .ok_or_else(|| ExactTypeError::MissingChild {
                child: id.to_string(),
            })?
            .clone();
        let children = key.children();
        let mut out = Vec::new();
        match key {
            ExactTypeKey::Nominal { declaration } => {
                let atom = self
                    .context
                    .nominal_atoms
                    .get(declaration.as_bytes())
                    .ok_or_else(|| ExactTypeError::MissingChild {
                        child: declaration.to_string(),
                    })?;
                write_atom(&mut out, atom);
            }
            ExactTypeKey::GeneratedNominal { role, identity } => {
                write_generated_atom(&mut out, role, &identity);
            }
            ExactTypeKey::NominalApplication { origin, arguments } => {
                out.push(b'a');
                out.push(b'(');
                let origin_atom = self
                    .context
                    .generic_atoms
                    .get(origin.as_bytes())
                    .ok_or_else(|| ExactTypeError::MissingChild {
                        child: origin.to_string(),
                    })?;
                write_atom(&mut out, origin_atom);
                out.push(b';');
                out.push(b'[');
                for (index, argument) in arguments.iter().enumerate() {
                    if index > 0 {
                        out.push(b',');
                    }
                    out.extend_from_slice(&self.name_bytes(argument)?);
                }
                out.push(b']');
                out.push(b')');
            }
            ExactTypeKey::Tuple { elements } => {
                out.push(b't');
                out.push(b'(');
                out.push(b'[');
                for (index, element) in elements.iter().enumerate() {
                    if index > 0 {
                        out.push(b',');
                    }
                    out.extend_from_slice(&self.name_bytes(element)?);
                }
                out.push(b']');
                out.push(b')');
            }
            ExactTypeKey::Function {
                effect,
                parameters,
                result,
            } => {
                out.push(b'f');
                out.push(b'(');
                out.push(match effect {
                    ManagedFunctionEffect::Ordinary => b'o',
                    ManagedFunctionEffect::Suspend => b's',
                });
                out.push(b';');
                out.push(b'[');
                for (index, parameter) in parameters.iter().enumerate() {
                    if index > 0 {
                        out.push(b',');
                    }
                    out.extend_from_slice(&self.name_bytes(parameter)?);
                }
                out.push(b']');
                out.push(b'-');
                out.push(b'>');
                out.extend_from_slice(&self.name_bytes(&result)?);
                out.push(b')');
            }
            ExactTypeKey::RawPointer { pointee } => {
                out.push(b'r');
                out.push(b'(');
                out.extend_from_slice(&self.name_bytes(&pointee)?);
                out.push(b')');
            }
            ExactTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => {
                out.push(b'x');
                out.push(b'(');
                match calling_convention {
                    NativeCallingConvention::C => out.push(b'c'),
                }
                out.push(b';');
                out.push(b'[');
                for (index, parameter) in parameters.iter().enumerate() {
                    if index > 0 {
                        out.push(b',');
                    }
                    out.extend_from_slice(&self.name_bytes(parameter)?);
                }
                out.push(b']');
                out.push(b'-');
                out.push(b'>');
                out.extend_from_slice(&self.name_bytes(&result)?);
                out.push(b')');
            }
        }
        self.stack.remove(&raw);
        // Per-path subtree cost: each edge into a child adds that child's
        // memoized cost, so shared children are re-counted for every
        // referencing path without exponential printing work.
        let mut cost = out.len() as u64;
        for child in children {
            cost = cost.saturating_add(
                self.costs
                    .get(child.as_bytes())
                    .copied()
                    .unwrap_or(DIAGNOSTIC_NAME_MAX_BYTES as u64 + 1),
            );
        }
        if cost > DIAGNOSTIC_NAME_MAX_BYTES as u64 {
            return Err(ExactTypeError::NameTooLarge(cost as usize));
        }
        self.costs.insert(raw, cost);
        self.names.insert(raw, out.clone());
        Ok(out)
    }
}

fn write_atom(out: &mut Vec<u8>, atom: &NominalAtom) {
    out.push(b'n');
    out.push(b'(');
    out.extend_from_slice(b"c=");
    escape_into(out, atom.coordinate.display().as_bytes());
    out.push(b';');
    out.extend_from_slice(b"p=");
    escape_into(out, atom.package.as_bytes());
    out.push(b';');
    out.extend_from_slice(b"o=");
    if atom.owner.is_empty() {
        out.push(b'-');
    } else {
        for (index, step) in atom.owner.iter().enumerate() {
            if index > 0 {
                out.push(b'/');
            }
            out.push(step.kind.letter() as u8);
            out.push(b':');
            escape_into(out, step.name.as_bytes());
        }
    }
    out.push(b';');
    out.extend_from_slice(b"k=");
    out.push(atom.kind.letter() as u8);
    out.push(b';');
    out.extend_from_slice(b"x=");
    escape_into(out, atom.name.as_bytes());
    out.push(b')');
}

fn write_generated_atom(out: &mut Vec<u8>, role: GeneratedNominalRole, identity: &[u8; 32]) {
    out.push(b'g');
    out.push(b'(');
    out.extend_from_slice(b"r=");
    let tag = format!("{:02x}", role.tag());
    out.extend_from_slice(tag.as_bytes());
    out.push(b';');
    out.extend_from_slice(b"i=");
    for byte in identity {
        out.extend_from_slice(format!("{byte:02x}").as_bytes());
    }
    out.push(b')');
}

/// `[A-Za-z0-9._-]` passes through; every other byte (including `%`)
/// becomes `%HH` with uppercase hex.
fn escape_into(out: &mut Vec<u8>, text: &[u8]) {
    for byte in text {
        let passthrough = byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-');
        if passthrough {
            out.push(*byte);
        } else {
            out.push(b'%');
            out.extend_from_slice(format!("{byte:02X}").as_bytes());
        }
    }
}
