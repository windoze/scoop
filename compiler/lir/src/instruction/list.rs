use std::ops::{Index, IndexMut, RangeBounds};

use scoop_source::DebugOrigin;

use super::Instruction;

/// An instruction and its source observation move together during CFG edits.
#[derive(Debug)]
pub struct InstructionRecord {
    instruction: Instruction,
    origin: DebugOrigin,
}

#[derive(Debug, Default)]
pub struct InstructionList {
    records: Vec<InstructionRecord>,
}

type Iter<'a> = std::iter::Map<
    std::slice::Iter<'a, InstructionRecord>,
    fn(&'a InstructionRecord) -> &'a Instruction,
>;
type IterMut<'a> = std::iter::Map<
    std::slice::IterMut<'a, InstructionRecord>,
    fn(&'a mut InstructionRecord) -> &'a mut Instruction,
>;

impl InstructionList {
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn iter(&self) -> Iter<'_> {
        self.records.iter().map(|record| &record.instruction)
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        self.records
            .iter_mut()
            .map(|record| &mut record.instruction)
    }

    pub fn origin(&self, index: usize) -> &DebugOrigin {
        &self.records[index].origin
    }

    pub fn first(&self) -> Option<&Instruction> {
        self.records.first().map(|record| &record.instruction)
    }

    pub fn last(&self) -> Option<&Instruction> {
        self.records.last().map(|record| &record.instruction)
    }

    /// Source-producing lowering supplies the complete current observation.
    pub fn push_located(&mut self, instruction: Instruction, origin: DebugOrigin) {
        self.records.push(InstructionRecord {
            instruction,
            origin,
        });
    }

    /// Protocol, poll and other generated insertions have no user location.
    pub fn push(&mut self, instruction: Instruction) {
        self.push_located(instruction, DebugOrigin::Generated);
    }

    pub fn insert(&mut self, index: usize, instruction: Instruction) {
        self.records.insert(
            index,
            InstructionRecord {
                instruction,
                origin: DebugOrigin::Generated,
            },
        );
    }

    pub fn pop(&mut self) -> Option<Instruction> {
        self.records.pop().map(|record| record.instruction)
    }

    pub fn truncate(&mut self, length: usize) {
        self.records.truncate(length);
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&Instruction) -> bool) {
        self.records.retain(|record| keep(&record.instruction));
    }

    pub fn splice(
        &mut self,
        range: impl RangeBounds<usize>,
        replacements: impl IntoIterator<Item = Instruction>,
    ) {
        self.records.splice(
            range,
            replacements
                .into_iter()
                .map(|instruction| InstructionRecord {
                    instruction,
                    origin: DebugOrigin::Generated,
                }),
        );
    }
}

impl From<Vec<Instruction>> for InstructionList {
    fn from(instructions: Vec<Instruction>) -> Self {
        let mut result = Self::default();
        result.extend(instructions);
        result
    }
}

impl Extend<Instruction> for InstructionList {
    fn extend<T: IntoIterator<Item = Instruction>>(&mut self, instructions: T) {
        self.records.extend(
            instructions
                .into_iter()
                .map(|instruction| InstructionRecord {
                    instruction,
                    origin: DebugOrigin::Generated,
                }),
        );
    }
}

impl Index<usize> for InstructionList {
    type Output = Instruction;

    fn index(&self, index: usize) -> &Self::Output {
        &self.records[index].instruction
    }
}

impl IndexMut<usize> for InstructionList {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.records[index].instruction
    }
}

impl<'a> IntoIterator for &'a InstructionList {
    type Item = &'a Instruction;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut InstructionList {
    type Item = &'a mut Instruction;
    type IntoIter = IterMut<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
