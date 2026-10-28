use super::*;

impl Types<'_> {
    pub(super) fn header_fields(&self) -> Vec<DebugField> {
        let pointer = self.context.pointer_layout(lir::PointerKind::Metadata);
        let word = self.context.scalar_layout(lir::BackendScalarKind::I64);
        vec![
            field(
                "__scoop_type",
                self.context.object_type_descriptor_offset(),
                self.address(),
            ),
            field(
                "__scoop_gc",
                pointer.size.next_multiple_of(word.align),
                basic("uint64_t", word.size, word.align, DebugEncoding::Unsigned),
            ),
        ]
    }

    pub(super) fn header_object(&self, name: &str) -> DebugType {
        let layout = self.context.object_header_layout();
        DebugType {
            name: name.into(),
            size: layout.size,
            align: layout.align,
            kind: DebugTypeKind::Record(self.header_fields()),
        }
    }

    pub(super) fn string_object(&self) -> DebugType {
        let layout = self.context.string_layout();
        let mut fields = self.header_fields();
        let word = self.context.scalar_layout(lir::BackendScalarKind::I64);
        fields.push(field(
            "length",
            self.context.object_header_layout().size,
            basic("Long", word.size, word.align, DebugEncoding::Signed),
        ));
        fields.push(field(
            "data",
            layout.size,
            inline(DebugType {
                name: String::new(),
                size: 0,
                align: 1,
                kind: DebugTypeKind::Array {
                    element: basic("uint8_t", 1, 1, DebugEncoding::Unsigned),
                    count: 0,
                },
            }),
        ));
        DebugType {
            name: "String".into(),
            size: layout.size,
            align: layout.align,
            kind: DebugTypeKind::Record(fields),
        }
    }

    pub(super) fn class_object(&mut self, id: mir::ClassId) -> StorageResult<DebugType> {
        let definition = &self.module.classes[id];
        let name = &definition.name;
        let exact = exact_type_record(self.module, &mir::Type::Class(id)).id();
        match &definition.representation {
            mir::ClassRepresentation::Declared(declared) => {
                let (offsets, size, align) = match self.layouts.get(&exact) {
                    Some(layout) => (
                        layout.fields.iter().map(|field| field.offset).collect(),
                        layout.size,
                        layout.align,
                    ),
                    None => class_shape(self.context, self.module, self.enums, declared)?,
                };
                let mut fields = self.header_fields();
                for (member, offset) in declared.fields.iter().zip(offsets) {
                    fields.push(field(&member.name, offset, self.source(&member.ty)?));
                }
                Ok(DebugType {
                    name: name.clone(),
                    size,
                    align,
                    kind: DebugTypeKind::Record(fields),
                })
            }
            mir::ClassRepresentation::Intrinsic(representation) => match representation {
                mir::IntrinsicClassRepresentation::Any
                | mir::IntrinsicClassRepresentation::Nothing => Ok(self.header_object(name)),
                mir::IntrinsicClassRepresentation::String => Ok(self.string_object()),
                mir::IntrinsicClassRepresentation::Array { element }
                | mir::IntrinsicClassRepresentation::MutableArray { element } => {
                    let instance = match self.arrays.get(&exact) {
                        Some(array) => array.layout.instance().clone(),
                        None => {
                            let (size, align) = self.shape(element)?;
                            let storage = value_storage(
                                self.context,
                                size,
                                align,
                                ref_scan(self.context, self.module, self.enums, element, 0)?,
                            )?;
                            lir::TypeInstanceShapeV1::inline_array(
                                self.context.target_profile(),
                                lir::ArrayElementStorageV1::from_value(&storage),
                            )?
                        }
                    };
                    let mut fields = self.header_fields();
                    let word = self.context.scalar_layout(lir::BackendScalarKind::I64);
                    fields.push(field(
                        "length",
                        self.context.object_header_layout().size,
                        basic("Long", word.size, word.align, DebugEncoding::Signed),
                    ));
                    fields.push(field(
                        "data",
                        instance.inline_offset(),
                        inline(DebugType {
                            name: String::new(),
                            size: 0,
                            align: instance.inline_alignment(),
                            kind: DebugTypeKind::Array {
                                element: self.source(element)?,
                                count: 0,
                            },
                        }),
                    ));
                    Ok(DebugType {
                        name: name.clone(),
                        size: instance.minimum_size(),
                        align: instance.instance_alignment(),
                        kind: DebugTypeKind::Record(fields),
                    })
                }
                mir::IntrinsicClassRepresentation::Atomic(storage) => {
                    let layout = atomic_object_layout(self.context, storage.kind())?;
                    let ty = match storage {
                        scoop_identity::AtomicStorage::Int => {
                            basic("Int", 4, 4, DebugEncoding::Signed)
                        }
                        scoop_identity::AtomicStorage::Long => {
                            basic("Long", 8, 8, DebugEncoding::Signed)
                        }
                        scoop_identity::AtomicStorage::Boolean => {
                            basic("Boolean", 1, 1, DebugEncoding::Boolean)
                        }
                        scoop_identity::AtomicStorage::Reference(ty) => self.source(ty)?,
                    };
                    let mut fields = self.header_fields();
                    fields.push(field("value", layout.value.offset, ty));
                    Ok(DebugType {
                        name: name.clone(),
                        size: layout.size,
                        align: layout.align,
                        kind: DebugTypeKind::Record(fields),
                    })
                }
            },
        }
    }
}
