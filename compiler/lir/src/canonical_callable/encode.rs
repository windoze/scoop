//! Explicit canonical LIR schema; no Rust discriminants or arena indices.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};

use scoop_wire::cbor::{EncodeError, encoded_length};
use scoop_wire::{Encoder, HashError, WireEncode};

use crate::*;

type Result = std::result::Result<(), EncodeError>;

macro_rules! record {
    ($writer:ident, $tag:expr; $($value:expr),+ $(,)?) => {{
        let fields = [$(stringify!($value)),+].len() as u64;
        $writer.e.map(fields + 1)?;
        $writer.e.field(0)?;
        $writer.e.unsigned($tag)?;
        let mut field = 0;
        $(field += 1; $writer.e.field(field)?; $value?;)+
        Ok::<(), EncodeError>(())
    }};
    ($writer:ident, $tag:expr;) => {{
        $writer.e.map(1)?;
        $writer.e.field(0)?;
        $writer.e.unsigned($tag)
    }};
}

mod abi;
mod calls;
mod instructions;
mod references;
mod roots;
mod values;

#[derive(Clone, Default)]
struct LocalIds {
    locals: BTreeMap<LocalId, u64>,
    temps: BTreeMap<TempId, u64>,
}

pub(super) struct CallableProjection<'a> {
    module: &'a Module,
    function: &'a Function,
    blocks: Vec<BlockId>,
    block_ids: BTreeMap<BlockId, u64>,
    local_ids: LocalIds,
}

impl<'a> CallableProjection<'a> {
    pub(super) fn new(
        module: &'a Module,
        function: &'a Function,
    ) -> std::result::Result<Self, HashError> {
        let mut blocks = Vec::new();
        let mut block_ids = BTreeMap::new();
        let mut pending = VecDeque::from([function.entry]);
        while let Some(id) = pending.pop_front() {
            if block_ids.contains_key(&id) {
                continue;
            }
            block_ids.insert(id, blocks.len() as u64);
            blocks.push(id);
            let block = &function.blocks[id];
            for instruction in &block.instructions {
                if let Instruction::Invoke { site } = instruction {
                    pending.extend([site.normal(), site.unwind()]);
                }
            }
            match block.terminator {
                Terminator::Br(target) => pending.push_back(target),
                Terminator::CondBr {
                    then_block,
                    else_block,
                    ..
                } => pending.extend([then_block, else_block]),
                Terminator::Return { .. } | Terminator::Resume { .. } | Terminator::Unreachable => {
                }
            }
        }
        let mut projection = Self {
            module,
            function,
            blocks,
            block_ids,
            local_ids: LocalIds::default(),
        };
        // Rank ordinary definitions/uses before visiting root sets, whose
        // source ordering is local to the input arena. This uses the same
        // encoder walk with a counting sink and never allocates body bytes.
        let ids = RefCell::new(LocalIds::default());
        encoded_length(&OrderingPass {
            projection: &projection,
            ids: &ids,
        })
        .map_err(|_| HashError::CborEncoding)?;
        projection.local_ids = ids.into_inner();
        Ok(projection)
    }

    fn encode_with_ids(&self, e: &mut Encoder, ids: &mut LocalIds, ordering: bool) -> Result {
        let mut writer = Writer {
            module: self.module,
            function: self.function,
            e,
            block_ids: &self.block_ids,
            ids,
            ordering,
        };
        writer.e.map(4)?;
        writer.e.field(1)?;
        self.function.callable_body.id().encode(writer.e)?;
        writer.e.field(2)?;
        writer.gc_effect(self.function.gc_effect)?;
        writer.e.field(3)?;
        writer.signature(&self.function.signature)?;
        writer.e.field(4)?;
        writer.e.array(self.blocks.len() as u64)?;
        for id in &self.blocks {
            let block = &self.function.blocks[*id];
            writer.e.map(2)?;
            writer.e.field(1)?;
            writer.e.array(block.instructions.len() as u64)?;
            for instruction in &block.instructions {
                writer.instruction(instruction)?;
            }
            writer.e.field(2)?;
            writer.terminator(&block.terminator)?;
        }
        Ok(())
    }
}

struct OrderingPass<'a, 'm> {
    projection: &'a CallableProjection<'m>,
    ids: &'a RefCell<LocalIds>,
}

impl WireEncode for OrderingPass<'_, '_> {
    fn encode(&self, encoder: &mut Encoder) -> Result {
        self.projection
            .encode_with_ids(encoder, &mut self.ids.borrow_mut(), true)
    }
}

impl WireEncode for CallableProjection<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result {
        self.encode_with_ids(encoder, &mut self.local_ids.clone(), false)
    }
}

pub(super) struct CallableAbiProjection<'a> {
    pub(super) module: &'a Module,
    pub(super) function: &'a Function,
    pub(super) group: scoop_identity::OdrGroupId,
    pub(super) member: scoop_identity::OdrMemberId,
    pub(super) role: scoop_identity::OdrMemberRole,
}

impl WireEncode for CallableAbiProjection<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result {
        // Signature encoding never uses body-local IDs or visits the CFG.
        let mut writer = Writer {
            module: self.module,
            function: self.function,
            e: encoder,
            block_ids: &BTreeMap::new(),
            ids: &mut LocalIds::default(),
            ordering: false,
        };
        writer.e.map(4)?;
        writer.e.field(1)?;
        self.group.encode(writer.e)?;
        writer.e.field(2)?;
        self.member.encode(writer.e)?;
        writer.e.field(3)?;
        self.role.encode(writer.e)?;
        writer.e.field(4)?;
        writer.e.map(2)?;
        writer.e.field(1)?;
        writer.gc_effect(self.function.gc_effect)?;
        writer.e.field(2)?;
        writer.signature(&self.function.signature)
    }
}

struct Writer<'m, 'e> {
    module: &'m Module,
    function: &'m Function,
    e: &'e mut Encoder,
    block_ids: &'e BTreeMap<BlockId, u64>,
    ids: &'e mut LocalIds,
    ordering: bool,
}

impl Writer<'_, '_> {
    fn u(&mut self, value: u64) -> Result {
        self.e.unsigned(value)
    }
    fn boolean(&mut self, value: bool) -> Result {
        self.u(u64::from(value))
    }
    fn id(&mut self, value: &impl WireEncode) -> Result {
        value.encode(self.e)
    }
    fn text(&mut self, value: &str) -> Result {
        self.e.text(value)
    }
    fn block(&mut self, value: BlockId) -> Result {
        self.u(self.block_ids[&value])
    }

    fn gc_effect(&mut self, effect: GcEffect) -> Result {
        self.u(match effect {
            GcEffect::Managed => 1,
            GcEffect::NoGc => 2,
        })
    }

    fn terminator(&mut self, value: &Terminator) -> Result {
        match value {
            Terminator::Br(target) => record!(self, 1; self.block(*target)),
            Terminator::CondBr {
                cond,
                then_block,
                else_block,
            } => {
                record!(self, 2; self.value(*cond), self.block(*then_block), self.block(*else_block))
            }
            Terminator::Return { value: None } => record!(self, 3;),
            Terminator::Return { value: Some(value) } => record!(self, 4; self.value(*value)),
            Terminator::Resume { exception } => record!(self, 5; self.value(*exception)),
            Terminator::Unreachable => record!(self, 6;),
        }
    }
}
