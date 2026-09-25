//! Allocation-free Mach-O sizing before invoking the shared object validators.

use super::*;
use ::object::read::macho::{MachHeader as _, Segment as _};
use ::object::{Endianness, macho};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ObjectCosts {
    pub(super) bytes: u64,
    pub(super) sections: u64,
    pub(super) symbols: u64,
    pub(super) relocations: u64,
    pub(super) names: u64,
    pub(super) longest_name: u64,
    pub(super) stackmap_bytes: u64,
}

pub(super) fn inspect(
    bytes: &[u8],
    member: crate::SlibMemberId,
    meter: &mut BudgetMeter,
) -> Result<ObjectCosts, LayoutLinkObjectContentsError> {
    let malformed = |source| LayoutLinkObjectContentsError::ObjectEnvelope { member, source };
    let path = WirePath::root();
    meter.charge_work(bytes.len() as u64, &path)?;
    let header = macho::MachHeader64::<Endianness>::parse(bytes, 0)
        .map_err(|_| malformed(ObjectEnvelopeValidationError::MalformedHeader))?;
    let endian = header
        .endian()
        .map_err(|_| malformed(ObjectEnvelopeValidationError::WrongEncoding))?;
    let mut commands = header
        .load_commands(endian, bytes, 0)
        .map_err(|_| malformed(ObjectEnvelopeValidationError::MalformedLoadCommands))?;
    meter.check_table_entries(u64::from(header.ncmds(endian)), &path)?;
    let mut costs = ObjectCosts {
        bytes: bytes.len() as u64,
        ..ObjectCosts::default()
    };
    while let Some(command) = commands
        .next()
        .map_err(|_| malformed(ObjectEnvelopeValidationError::MalformedLoadCommands))?
    {
        meter.charge_work(u64::from(command.cmdsize()), &path)?;
        if let Some((segment, section_bytes)) = command
            .segment_64()
            .map_err(|_| malformed(ObjectEnvelopeValidationError::MalformedSegment))?
        {
            let sections = segment
                .sections(endian, section_bytes)
                .map_err(|_| malformed(ObjectEnvelopeValidationError::MalformedSegment))?;
            meter.check_table_entries(sections.len() as u64, &path)?;
            costs.sections = costs.sections.saturating_add(sections.len() as u64);
            for section in sections {
                meter.charge_work(1, &path)?;
                if section.sectname.starts_with(b"__llvm_stackmaps") {
                    costs.stackmap_bytes = costs
                        .stackmap_bytes
                        .saturating_add(section.size.get(endian));
                }
                costs.relocations = costs
                    .relocations
                    .saturating_add(u64::from(section.nreloc.get(endian)));
            }
        }
        if let Some(symtab) = command
            .symtab()
            .map_err(|_| malformed(ObjectEnvelopeValidationError::MalformedSymbolTable))?
        {
            let count = u64::from(symtab.nsyms.get(endian));
            meter.check_table_entries(count, &path)?;
            costs.symbols = costs.symbols.saturating_add(count);
            let table = symtab
                .symbols::<macho::MachHeader64<Endianness>, _>(endian, bytes)
                .map_err(|_| malformed(ObjectEnvelopeValidationError::MalformedSymbolTable))?;
            let start = u64::from(symtab.stroff.get(endian));
            let length = u64::from(symtab.strsize.get(endian));
            meter.check_semantic_leaf(length, &path)?;
            let strings = start
                .checked_add(length)
                .and_then(|end| bytes.get(usize::try_from(start).ok()?..usize::try_from(end).ok()?))
                .ok_or_else(|| malformed(ObjectEnvelopeValidationError::MalformedSymbolTable))?;
            for symbol in table.iter() {
                meter.charge_work(1, &path)?;
                let suffix = strings
                    .get(symbol.n_strx.get(endian) as usize..)
                    .ok_or_else(|| {
                        malformed(ObjectEnvelopeValidationError::MalformedSymbolTable)
                    })?;
                let length = name_length(suffix, meter)?.ok_or_else(|| {
                    malformed(ObjectEnvelopeValidationError::MalformedSymbolTable)
                })?;
                costs.names = costs.names.saturating_add(length);
                costs.longest_name = costs.longest_name.max(length);
            }
        }
    }
    meter.check_table_entries(costs.relocations, &path)?;
    Ok(costs)
}

fn name_length(bytes: &[u8], meter: &mut BudgetMeter) -> Result<Option<u64>, WireError> {
    let mut length = 0;
    for chunk in bytes.chunks(256) {
        // A repeated string-table offset cannot hide repeated name scans.
        meter.charge_work(chunk.len() as u64, &WirePath::root())?;
        if let Some(end) = chunk.iter().position(|byte| *byte == 0) {
            return Ok(Some(length + end as u64));
        }
        length += chunk.len() as u64;
    }
    Ok(None)
}

impl ObjectCosts {
    pub(super) fn envelope(self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        table::<ObservedMachOSectionV1>(self.sections.saturating_mul(3), meter)?;
        table::<ObservedMachOSymbolV1>(self.symbols, meter)?;
        table::<ObservedMachORelocationV1>(self.relocations, meter)?;
        meter.charge_owned_bytes(self.names, &WirePath::root())?;
        meter.charge_work(
            self.bytes.saturating_mul(3).saturating_add(self.names),
            &WirePath::root(),
        )
    }

    pub(super) fn strong(self, atoms: u64, meter: &mut BudgetMeter) -> Result<(), WireError> {
        self.envelope(meter)?;
        self.copy_proof(meter)?;
        // Relocations and local symbols search the actual member's atom ranges.
        let scans = self
            .relocations
            .saturating_add(self.symbols)
            .saturating_mul(atoms);
        let targets = self
            .relocations
            .saturating_mul(self.symbols)
            .saturating_mul(2);
        let name_comparisons = self
            .names
            .saturating_mul(4)
            .saturating_add(
                self.relocations
                    .saturating_mul(self.longest_name)
                    .saturating_mul(4),
            )
            .saturating_mul(log(self.symbols));
        meter.charge_work(
            scans
                .saturating_add(targets)
                .saturating_add(name_comparisons),
            &WirePath::root(),
        )
    }

    pub(super) fn copy_proof(self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        let count = self
            .sections
            .saturating_mul(3)
            .saturating_add(self.symbols.saturating_mul(4))
            .saturating_add(self.relocations.saturating_mul(4));
        // Includes observed tables, typed definitions, ranges and relocation
        // bindings. Names occur in symbols and up to two targets per relocation.
        slots::<VerifiedRelocationUseV1>(count, meter)?;
        let names = self.names.saturating_mul(4).saturating_add(
            self.relocations
                .saturating_mul(self.longest_name)
                .saturating_mul(8),
        );
        meter.charge_owned_bytes(names, &WirePath::root())?;
        meter.charge_work(names.saturating_add(count), &WirePath::root())
    }
}

#[cfg(test)]
mod tests;
