use std::fmt;

use scoop_identity::{
    CborIdentityRecord, DefinitionAtomRole, DefinitionAtomSubkey, ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey, ObjectDefinitionIdentityError, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PersistentCallableBodyId, StrongDefinitionEntity,
    StrongDefinitionRole, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

use crate::{CallSite, Function, Instruction, InvokeSite, Module, RefScan};

/// One callable-owned recursive scan-program node in physical emission order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongCallableRuntimeScanAtomV1 {
    atom: ObjectDefinitionAtomId,
    scan: RefScan,
}

impl StrongCallableRuntimeScanAtomV1 {
    pub const fn atom(&self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn scan(&self) -> &RefScan {
        &self.scan
    }

    pub(crate) const fn from_artifact(atom: ObjectDefinitionAtomId, scan: RefScan) -> Self {
        Self { atom, scan }
    }
}

/// Complete callable-local runtime-scan closure for one body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongCallableRuntimeScanPlanV1 {
    body: PersistentCallableBodyId,
    atoms: Vec<StrongCallableRuntimeScanAtomV1>,
}

impl StrongCallableRuntimeScanPlanV1 {
    pub const fn body(&self) -> PersistentCallableBodyId {
        self.body
    }

    pub fn atoms(&self) -> &[StrongCallableRuntimeScanAtomV1] {
        &self.atoms
    }

    pub(crate) const fn from_artifact(
        body: PersistentCallableBodyId,
        atoms: Vec<StrongCallableRuntimeScanAtomV1>,
    ) -> Self {
        Self { body, atoms }
    }
}

/// Canonical runtime-scan plans for every callable in one Cone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongCallableRuntimeScanPlanSetV1 {
    producer: scoop_identity::ConeIdentity,
    callables: Vec<StrongCallableRuntimeScanPlanV1>,
}

impl StrongCallableRuntimeScanPlanSetV1 {
    pub fn from_foundation_without_scans(
        foundation: &crate::OdrFreeLirFoundation,
    ) -> Result<Self, StrongCallableRuntimeScanPlanError> {
        let mut callables = foundation
            .callable_bodies()
            .iter()
            .map(|body| {
                let body = body.id();
                let plan = callable_definition_plan(foundation.producer(), body)?;
                let actual = foundation
                    .definition_atoms()
                    .iter()
                    .filter(|atom| {
                        atom.key().plan() == plan
                            && atom.key().role() == DefinitionAtomRole::RuntimeRecord
                    })
                    .map(|atom| atom.id())
                    .collect::<Vec<_>>();
                if !actual.is_empty() {
                    return Err(
                        StrongCallableRuntimeScanPlanError::UnexpectedFoundationAtoms {
                            body,
                            actual,
                        },
                    );
                }
                Ok(StrongCallableRuntimeScanPlanV1 {
                    body,
                    atoms: Vec::new(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        callables.sort_unstable_by_key(StrongCallableRuntimeScanPlanV1::body);
        Self::from_artifact(foundation.producer(), callables)
    }

    pub fn from_module(module: &Module) -> Result<Self, StrongCallableRuntimeScanPlanError> {
        let mut callables = module
            .functions
            .iter()
            .map(|function| callable_plan(module.cone, function))
            .collect::<Result<Vec<_>, _>>()?;
        callables.sort_unstable_by_key(StrongCallableRuntimeScanPlanV1::body);
        if let Some(pair) = callables
            .windows(2)
            .find(|pair| pair[0].body == pair[1].body)
        {
            return Err(StrongCallableRuntimeScanPlanError::DuplicateBody(
                pair[0].body,
            ));
        }
        Ok(Self {
            producer: module.cone,
            callables,
        })
    }

    pub(crate) fn from_artifact(
        producer: scoop_identity::ConeIdentity,
        callables: Vec<StrongCallableRuntimeScanPlanV1>,
    ) -> Result<Self, StrongCallableRuntimeScanPlanError> {
        if !callables.windows(2).all(|pair| pair[0].body < pair[1].body) {
            return Err(StrongCallableRuntimeScanPlanError::NonCanonicalBodyOrder);
        }
        for callable in &callables {
            validate_artifact_callable(producer, callable)?;
        }
        Ok(Self {
            producer,
            callables,
        })
    }

    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.producer
    }

    pub fn callables(&self) -> &[StrongCallableRuntimeScanPlanV1] {
        &self.callables
    }

    pub fn callable(
        &self,
        body: PersistentCallableBodyId,
    ) -> Option<&StrongCallableRuntimeScanPlanV1> {
        self.callables
            .binary_search_by_key(&body, StrongCallableRuntimeScanPlanV1::body)
            .ok()
            .map(|index| &self.callables[index])
    }
}

fn callable_plan(
    producer: scoop_identity::ConeIdentity,
    function: &Function,
) -> Result<StrongCallableRuntimeScanPlanV1, StrongCallableRuntimeScanPlanError> {
    let body = function.callable_body.id();
    let plan = callable_definition_plan(producer, body)?;
    let mut scans = Vec::new();
    for (_, scan) in function.call_targets.root_scans.iter() {
        append_scan_tree(scan, body, &mut scans)?;
    }
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            append_instruction_scans(function, instruction, body, &mut scans)?;
        }
    }
    let atoms = scans
        .into_iter()
        .enumerate()
        .map(|(ordinal, scan)| {
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| StrongCallableRuntimeScanPlanError::TooManyAtoms(body))?;
            let atom = runtime_scan_atom(plan, ordinal)?;
            Ok(StrongCallableRuntimeScanAtomV1 { atom, scan })
        })
        .collect::<Result<Vec<_>, StrongCallableRuntimeScanPlanError>>()?;
    Ok(StrongCallableRuntimeScanPlanV1 { body, atoms })
}

fn append_instruction_scans(
    function: &Function,
    instruction: &Instruction,
    body: PersistentCallableBodyId,
    output: &mut Vec<RefScan>,
) -> Result<(), StrongCallableRuntimeScanPlanError> {
    match instruction {
        Instruction::BoxValue {
            payload: crate::BoxPayload::NonZero(place),
            ..
        } => {
            if let crate::BoxPayloadRooting::RecursiveRegion(scan) = place.rooting() {
                append_scan_tree(scan.as_ref_scan(), body, output)?;
            }
        }

        Instruction::NativeGlobalLoad { roots, .. }
        | Instruction::NativeGlobalStore { roots, .. }
        | Instruction::NativeGlobalAddress { roots, .. } => {
            append_roots(roots.as_slice(), body, output)?;
        }
        Instruction::Call { site } => match site {
            CallSite::NativeSafe(site) => append_roots(site.roots.as_slice(), body, output)?,
            CallSite::NativeBorrowed(site) => {
                append_roots(site.roots.as_slice(), body, output)?;
                let view = site.call.view(&function.call_targets);
                match view.result {
                    crate::NativeBorrowedResultPublication::DirectRooted { scan, .. }
                    | crate::NativeBorrowedResultPublication::IndirectResultRooted {
                        scan, ..
                    } => {
                        append_scan_tree(scan.as_ref_scan(), body, output)?;
                    }
                    crate::NativeBorrowedResultPublication::Void
                    | crate::NativeBorrowedResultPublication::ElidedZst
                    | crate::NativeBorrowedResultPublication::DirectGcFree
                    | crate::NativeBorrowedResultPublication::IndirectResultGcFree => {}
                }
            }
            CallSite::Managed(_) | CallSite::NoGc(_) => {}
        },
        Instruction::Invoke {
            site: InvokeSite::Managed(site),
        } => {
            for exceptional in site.roots.as_slice() {
                append_scan_tree(exceptional.root.scan.as_ref_scan(), body, output)?;
            }
        }
        Instruction::Invoke {
            site: InvokeSite::NoGc(_),
        } => {}
        _ => {}
    }
    Ok(())
}

fn append_roots(
    roots: &[crate::CallerRoot],
    body: PersistentCallableBodyId,
    output: &mut Vec<RefScan>,
) -> Result<(), StrongCallableRuntimeScanPlanError> {
    for root in roots {
        append_scan_tree(root.scan.as_ref_scan(), body, output)?;
    }
    Ok(())
}

fn append_scan_tree(
    scan: &RefScan,
    body: PersistentCallableBodyId,
    output: &mut Vec<RefScan>,
) -> Result<bool, StrongCallableRuntimeScanPlanError> {
    let materialized = match scan {
        RefScan::None => false,
        RefScan::References(offsets) => !offsets.is_empty(),
        RefScan::Sequence(parts) => {
            let mut any = false;
            for part in parts {
                any |= append_scan_tree(part, body, output)?;
            }
            any
        }
        RefScan::Array { element, .. } => {
            if !append_scan_tree(element.as_ref_scan(), body, output)? {
                return Err(StrongCallableRuntimeScanPlanError::EmptyArrayElement(body));
            }
            true
        }
    };
    if materialized {
        output.push(scan.clone());
    }
    Ok(materialized)
}

fn validate_artifact_callable(
    producer: scoop_identity::ConeIdentity,
    callable: &StrongCallableRuntimeScanPlanV1,
) -> Result<(), StrongCallableRuntimeScanPlanError> {
    let plan = callable_definition_plan(producer, callable.body)?;
    for (ordinal, atom) in callable.atoms.iter().enumerate() {
        let ordinal = u32::try_from(ordinal)
            .map_err(|_| StrongCallableRuntimeScanPlanError::TooManyAtoms(callable.body))?;
        let expected = runtime_scan_atom(plan, ordinal)?;
        if atom.atom != expected {
            return Err(StrongCallableRuntimeScanPlanError::AtomIdentity {
                body: callable.body,
                ordinal,
                expected,
                actual: atom.atom,
            });
        }
        if !atom.scan.contains_reference() {
            return Err(StrongCallableRuntimeScanPlanError::EmptyAtom {
                body: callable.body,
                ordinal,
            });
        }
    }
    Ok(())
}

fn callable_definition_plan(
    producer: scoop_identity::ConeIdentity,
    body: PersistentCallableBodyId,
) -> Result<ObjectDefinitionPlanId, StrongCallableRuntimeScanPlanError> {
    let key = ObjectDefinitionPlanKey::strong(
        producer,
        StrongDefinitionEntity::callable_body(body),
        StrongDefinitionRole::CallableBody,
    )
    .map_err(StrongCallableRuntimeScanPlanError::DefinitionIdentity)?;
    ObjectDefinitionPlanId::from_key(&key).map_err(StrongCallableRuntimeScanPlanError::Hash)
}

fn runtime_scan_atom(
    plan: ObjectDefinitionPlanId,
    ordinal: u32,
) -> Result<ObjectDefinitionAtomId, StrongCallableRuntimeScanPlanError> {
    let path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::SyntheticValue, ordinal),
        [],
    );
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::RuntimeRecord,
        DefinitionAtomSubkey::StructuralPath(path),
    ))
    .map(|record| record.id())
    .map_err(StrongCallableRuntimeScanPlanError::Hash)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongCallableRuntimeScanPlanError {
    DuplicateBody(PersistentCallableBodyId),
    NonCanonicalBodyOrder,
    TooManyAtoms(PersistentCallableBodyId),
    EmptyArrayElement(PersistentCallableBodyId),
    EmptyAtom {
        body: PersistentCallableBodyId,
        ordinal: u32,
    },
    UnexpectedFoundationAtoms {
        body: PersistentCallableBodyId,
        actual: Vec<ObjectDefinitionAtomId>,
    },
    AtomIdentity {
        body: PersistentCallableBodyId,
        ordinal: u32,
        expected: ObjectDefinitionAtomId,
        actual: ObjectDefinitionAtomId,
    },
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Hash(scoop_wire::HashError),
}

impl fmt::Display for StrongCallableRuntimeScanPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid callable runtime scan plan: {self:?}")
    }
}

impl std::error::Error for StrongCallableRuntimeScanPlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DefinitionIdentity(source) => Some(source),
            Self::Hash(source) => Some(source),
            _ => None,
        }
    }
}
