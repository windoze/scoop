//! Resolve retained program uses through their already checked object atoms.
use super::*;
use scoop_identity::{DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId};
use scoop_slib::{
    PlannedStrongObjectSymbolRoleV1 as SymbolRole, VerifiedDefinitionAtomRangeV1,
    VerifiedMemberObjectRelocationIndexV1, VerifiedObjectRelocationShapeV1 as Shape,
    VerifiedRelocationTargetV1 as Target,
};

mod instructions;
mod native;
mod odr;
mod resolved;
use resolved::ResolvedShape;

pub(super) fn check(
    image: &FinalImage<'_>,
    inputs: &ProgramInputs<'_>,
    map: &crate::link::map::LinkMap,
) -> Result<(), LinkError> {
    odr::check(image, inputs, map)?;
    for closure in &inputs.strong_relocations {
        for member in closure.members() {
            if !inputs.selected.contains(member.producer(), member.member()) {
                continue;
            }
            Member::new(member).check(image, inputs).map_err(|err| {
                error(format!(
                    "Cone {} member {}: {err}",
                    member.producer(),
                    member.member()
                ))
            })?;
        }
    }
    native::check(image, inputs, map)
}

struct Member<'a> {
    source: &'a VerifiedMemberObjectRelocationIndexV1,
    atoms: BTreeMap<ObjectDefinitionAtomId, (VerifiedDefinitionAtomRangeV1, &'a [u8])>,
    primaries: BTreeMap<ObjectDefinitionPlanId, &'a [u8]>,
}
impl<'a> Member<'a> {
    fn new(source: &'a VerifiedMemberObjectRelocationIndexV1) -> Self {
        let symbols = source.definitions().symbols();
        let starts: BTreeMap<_, _> = symbols
            .iter()
            .filter_map(|symbol| match symbol.role() {
                SymbolRole::AtomBoundaryStart { atom, .. } => Some((atom, symbol.macho_name())),
                _ => None,
            })
            .collect();
        let primaries = symbols
            .iter()
            .filter_map(|symbol| match symbol.role() {
                SymbolRole::PrimaryDefinition { definition, .. } => {
                    Some((definition, symbol.macho_name()))
                }
                _ => None,
            })
            .collect();
        let atoms = source
            .definitions()
            .definitions()
            .iter()
            .flat_map(|definition| definition.atoms())
            .map(|atom| (atom.atom(), (*atom, starts[&atom.atom()])))
            .collect();
        Self {
            source,
            atoms,
            primaries,
        }
    }

    fn atom_address(
        &self,
        image: &FinalImage<'_>,
        atom: ObjectDefinitionAtomId,
    ) -> Result<u64, LinkError> {
        let (_, symbol) = self
            .atoms
            .get(&atom)
            .ok_or_else(|| error(format!("missing input atom {atom}")))?;
        image.symbol(std::str::from_utf8(symbol).map_err(error)?)
    }

    fn target(&self, image: &FinalImage<'_>, target: &Target) -> Result<Expected, LinkError> {
        match target {
            Target::StrongDefinition { definition } => {
                let name = self
                    .primaries
                    .get(definition)
                    .ok_or_else(|| error("missing Strong definition primary"))?;
                Ok(Expected::Symbol(
                    std::str::from_utf8(name).map_err(error)?.to_owned(),
                ))
            }
            Target::ExternalUndefined { name, .. } => Ok(Expected::Symbol(
                std::str::from_utf8(name).map_err(error)?.to_owned(),
            )),
            Target::LocalDefinition {
                owner_atom: Some(atom),
                value,
                ..
            } => {
                let (range, _) = self
                    .atoms
                    .get(atom)
                    .ok_or_else(|| error("missing local target atom"))?;
                let offset = value
                    .checked_sub(range.start())
                    .ok_or_else(|| error("local target is before its atom"))?;
                Ok(Expected::Address(
                    self.atom_address(image, *atom)?
                        .checked_add(offset)
                        .ok_or_else(|| error("local target address overflow"))?,
                ))
            }
            Target::LocalDefinition { name, .. } => Ok(Expected::Symbol(
                std::str::from_utf8(name).map_err(error)?.to_owned(),
            )),
            Target::SectionBase { .. } => {
                Err(error("controlled reference has an unmapped section target"))
            }
        }
    }

    fn check(&self, image: &FinalImage<'_>, inputs: &ProgramInputs<'_>) -> Result<(), LinkError> {
        let mut pages: BTreeMap<ObjectDefinitionAtomId, BTreeMap<u32, u64>> = BTreeMap::new();
        let mut uses: Vec<_> = self.source.relocations().iter().collect();
        uses.sort_by_key(|use_| (use_.containing_atom(), use_.offset_within_atom()));
        for use_ in uses {
            // ld rebuilds unwind data; stackmap blobs have their own exact
            // section/relocation check. Neither uses object boundary addresses.
            if matches!(
                use_.containing_atom_role(),
                DefinitionAtomRole::EhFrame
                    | DefinitionAtomRole::CompactUnwind
                    | DefinitionAtomRole::Stackmap
            ) {
                continue;
            }
            if !controlled(use_.shape()) {
                continue;
            }
            let place = self
                .atom_address(image, use_.containing_atom())?
                .checked_add(use_.offset_within_atom())
                .ok_or_else(|| error("reference address overflow"))?;
            instructions::check(
                image,
                inputs,
                &ResolvedShape::scoop(use_.shape(), |target| self.target(image, target))?,
                use_.encoded_value(),
                place,
                pages.entry(use_.containing_atom()).or_default(),
            )
            .map_err(|err| {
                error(format!(
                    "atom {} + {:#x}: {err}",
                    use_.containing_atom(),
                    use_.offset_within_atom()
                ))
            })?;
        }
        Ok(())
    }
}

fn controlled(shape: &Shape) -> bool {
    let is_controlled = |target: &Target| {
        matches!(
            target,
            Target::StrongDefinition { .. } | Target::ExternalUndefined { .. }
        )
    };
    match shape {
        Shape::ElfRela { target, .. }
        | Shape::Unsigned64 { target }
        | Shape::Branch26 { target }
        | Shape::Page21 { target, .. }
        | Shape::PageOffset12 { target, .. }
        | Shape::GotLoadPage21 { target }
        | Shape::GotLoadPageOffset12 { target }
        | Shape::PointerToGot32 { target }
        | Shape::TlvpLoadPage21 { target }
        | Shape::TlvpLoadPageOffset12 { target } => is_controlled(target),
        Shape::Subtractor64 {
            minuend,
            subtrahend,
        } => is_controlled(minuend) || is_controlled(subtrahend),
    }
}

#[derive(Debug)]
enum Expected {
    Symbol(String),
    Address(u64),
}
impl Expected {
    fn address(&self, image: &FinalImage<'_>) -> Result<u64, LinkError> {
        match self {
            Self::Address(address) => Ok(*address),
            Self::Symbol(name) => image.symbol(name),
        }
    }
    fn pointer(&self, image: &FinalImage<'_>, place: u64, addend: i64) -> Result<(), LinkError> {
        if let Self::Symbol(name) = self
            && let Some(binding) = image.bindings.get(&place)
            && &binding.symbol == name
            && binding.addend == addend
        {
            return Ok(());
        }
        let expected = self
            .address(image)?
            .checked_add_signed(addend)
            .ok_or_else(|| error("reference target overflow"))?;
        let actual = image.pointer(place)?;
        if actual != expected {
            return Err(error(format!(
                "pointer at {place:#x} binds to {actual:#x}, expected {self:?} + {addend}"
            )));
        }
        Ok(())
    }
}
