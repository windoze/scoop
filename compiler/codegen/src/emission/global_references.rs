//! Typed data references required by one physical LLVM module.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

pub(super) struct ConeDeclarations {
    cstrings: BTreeMap<scoop_lir::PersistentCallableBodyId, Vec<scoop_lir::GlobalId>>,
    pub initialization_units:
        BTreeMap<scoop_lir::InitializationUnitId, scoop_lir::PersistentSymbolRequest>,
}

impl ConeDeclarations {
    pub fn new<D, C, I>(
        module: &Module,
        production: &scoop_lir::ConeProductionSection<D, C, I>,
    ) -> Result<Self, CodegenError> {
        let mut cstrings: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (id, global) in module.globals.iter() {
            if let GlobalInit::CString { identity, .. } = &global.init {
                cstrings.entry(identity.owner()).or_default().push(id);
            }
        }
        Ok(Self {
            cstrings,
            initialization_units: initialization::initialization_unit_requests(module, production)?,
        })
    }
}

#[derive(Default)]
pub(super) struct GlobalReferences {
    pub globals: BTreeSet<scoop_lir::GlobalId>,
    pub local_descriptors: BTreeSet<scoop_lir::TypeDescriptorId>,
    pub external_descriptors: BTreeSet<scoop_lir::ExternalTypeDescriptorId>,
    pub arrays: BTreeSet<ArrayTypeId>,
    pub initialization_units: BTreeSet<scoop_lir::InitializationUnitId>,
}

impl GlobalReferences {
    pub fn all(module: &Module) -> Self {
        Self {
            globals: module.globals.iter().map(|(id, _)| id).collect(),
            local_descriptors: module
                .meta
                .type_descriptors
                .iter()
                .map(|(id, _)| id)
                .collect(),
            external_descriptors: module
                .meta
                .external_type_descriptors
                .iter()
                .map(|(id, _)| id)
                .collect(),
            arrays: module.meta.arrays.iter().map(|(id, _)| id).collect(),
            initialization_units: module
                .initialization_units
                .iter()
                .map(|(id, _)| id)
                .collect(),
        }
    }

    pub fn for_function(module: &Module, function: &Function, cone: &ConeDeclarations) -> Self {
        let mut references = Self::default();
        // Callable-owned constants remain definitions even when ordinary
        // optimization later removes the instruction that uses them.
        if let Some(cstrings) = cone.cstrings.get(&function.callable_body.id()) {
            references.globals.extend(cstrings);
        }
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                for value in dataflow::instruction_uses(instruction, function) {
                    references.value(value);
                }
                references.instruction(instruction);
            }
            dataflow::terminator_uses(&block.terminator, |value| references.value(value));
        }
        for id in references.arrays.clone() {
            references.descriptor(module.meta.arrays[id].type_descriptor);
        }
        references
    }

    fn value(&mut self, value: Value) {
        match value {
            Value::TypeDescriptor(reference) => self.descriptor(reference),
            Value::Global(id) => {
                self.globals.insert(id);
            }
            Value::InitializationUnit(id) => {
                self.initialization_units.insert(id);
            }
            _ => {}
        }
    }

    fn descriptor(&mut self, reference: TypeDescriptorRef) {
        match reference {
            TypeDescriptorRef::Local(id) => {
                self.local_descriptors.insert(id);
            }
            TypeDescriptorRef::External(id) => {
                self.external_descriptors.insert(id);
            }
        }
    }

    fn instruction(&mut self, instruction: &Instruction) {
        match instruction {
            Instruction::GlobalLoad { global, .. }
            | Instruction::GlobalStore { global, .. }
            | Instruction::GlobalAddress { global, .. } => {
                self.globals.insert(*global);
            }
            Instruction::StringConcat {
                descriptor,
                overflow_message,
                ..
            } => {
                self.descriptor(*descriptor);
                self.globals.insert(*overflow_message);
            }
            Instruction::StringCopy {
                source,
                target,
                overflow_message,
                ..
            } => {
                if let scoop_lir::StringCopySource::Bytes { array_type, .. } = source {
                    self.arrays.insert(*array_type);
                }
                match target {
                    scoop_lir::StringCopyTarget::String(descriptor) => self.descriptor(*descriptor),
                    scoop_lir::StringCopyTarget::Bytes(array_type) => {
                        self.arrays.insert(*array_type);
                    }
                }
                self.globals.insert(*overflow_message);
            }
            Instruction::BoxValue { payload, .. } => self.descriptor(payload.descriptor()),
            Instruction::UnboxValue { result, .. } => self.descriptor(result.descriptor()),
            Instruction::ArrayAllocDynamic {
                array_type,
                overflow_message,
                ..
            }
            | Instruction::ArrayAssembly {
                array_type,
                overflow_message,
                ..
            } => {
                self.arrays.insert(*array_type);
                self.globals.insert(*overflow_message);
            }
            Instruction::ArrayAlloc { array_type, .. }
            | Instruction::ArrayLen { array_type, .. }
            | Instruction::ArrayGet { array_type, .. }
            | Instruction::ArraySet { array_type, .. }
            | Instruction::ArrayDataPointer { array_type, .. } => {
                self.arrays.insert(*array_type);
            }
            Instruction::ArrayClone {
                array_type,
                source_type,
                ..
            } => {
                self.arrays.insert(*array_type);
                self.arrays.insert(*source_type);
            }
            _ => {}
        }
    }
}
