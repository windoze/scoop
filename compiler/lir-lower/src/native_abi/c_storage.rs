use super::*;

impl CanonicalCAbiBuilder<'_> {
    pub(super) fn storage(
        &mut self,
        ty: &mir::Type,
    ) -> StorageResult<identity::CanonicalCStorageType> {
        Ok(match ty {
            mir::Type::Unit => {
                return Err(StorageLoweringError::InvalidRepresentation(
                    "Unit has no C object representation",
                ));
            }
            mir::Type::Integer(kind) => {
                let exact_type = self.exact_type(ty);
                identity::CanonicalCStorageType::Integer {
                    exact_type,
                    signedness: match kind.signedness() {
                        mir::IntegerSignedness::Signed => identity::Signedness::Signed,
                        mir::IntegerSignedness::Unsigned => identity::Signedness::Unsigned,
                    },
                    bit_width: match kind.width() {
                        mir::IntegerWidth::W8 => identity::IntegerBitWidth::Bits8,
                        mir::IntegerWidth::W16 => identity::IntegerBitWidth::Bits16,
                        mir::IntegerWidth::W32 => identity::IntegerBitWidth::Bits32,
                        mir::IntegerWidth::W64 => identity::IntegerBitWidth::Bits64,
                    },
                }
            }
            mir::Type::Boolean => identity::CanonicalCStorageType::Boolean {
                exact_type: self.exact_type(ty),
            },
            mir::Type::Ptr(pointee) => identity::CanonicalCStorageType::DataPointer {
                exact_type: self.exact_type(ty),
                pointee: self.data_pointee(pointee),
                storage: identity::CPointerStorage::Direct,
            },
            mir::Type::FunPtr(_) => identity::CanonicalCStorageType::CodePointer {
                exact_type: self.exact_type(ty),
                storage: identity::CPointerStorage::Direct,
            },
            mir::Type::Struct(id) => {
                if matches!(
                    self.module.structs[*id].representation,
                    mir::StructRepresentation::Declared {
                        c_abi: mir::StructCAbi::UInt64Field { .. },
                        ..
                    }
                ) {
                    return Ok(identity::CanonicalCStorageType::Integer {
                        exact_type: self.exact_type(ty),
                        signedness: identity::Signedness::Unsigned,
                        bit_width: identity::IntegerBitWidth::Bits64,
                    });
                }
                let layout = self.layout(*id)?;
                identity::CanonicalCStorageType::Struct {
                    exact_type: self.exact_type(ty),
                    layout,
                }
            }
            mir::Type::Enum(id, arguments) if self.module.option_core(*id).is_some() => {
                let exact_type = self.exact_type(ty);
                match exact_option_payload(self.module, *id, arguments) {
                    mir::Type::Ptr(pointee) => identity::CanonicalCStorageType::DataPointer {
                        exact_type,
                        pointee: self.data_pointee(pointee),
                        storage: identity::CPointerStorage::NullableWrapper(exact_type),
                    },
                    mir::Type::FunPtr(_) => identity::CanonicalCStorageType::CodePointer {
                        exact_type,
                        storage: identity::CPointerStorage::NullableWrapper(exact_type),
                    },
                    _ => {
                        return Err(StorageLoweringError::InvalidRepresentation(
                            "a C-nullable option must wrap a pointer",
                        ));
                    }
                }
            }
            _ => {
                return Err(StorageLoweringError::InvalidRepresentation(
                    "the source type has no C object representation",
                ));
            }
        })
    }

    fn data_pointee(&self, pointee: &mir::Type) -> identity::CDataPointee {
        if pointee == &mir::Type::Unit {
            identity::CDataPointee::OpaqueUnit
        } else {
            identity::CDataPointee::ExactObject(self.exact_type(pointee))
        }
    }

    fn layout(
        &mut self,
        id: mir::StructId,
    ) -> StorageResult<identity::CanonicalCAbiLayoutFingerprint> {
        if let Some(fingerprint) = self.layout_fingerprints.get(&id) {
            return Ok(*fingerprint);
        }
        if !self.visiting_layouts.insert(id) {
            return Err(StorageLoweringError::InvalidRepresentation(
                "C layouts cannot contain a by-value cycle",
            ));
        }
        if id.into_raw().into_u32() as usize >= self.module.structs.len()
            || struct_def_id(id).into_raw().into_u32() as usize >= self.structs.len()
        {
            return Err(StorageLoweringError::InvalidRepresentation(
                "unknown C struct layout",
            ));
        }

        let ty = mir::Type::Struct(id);
        let exact_type = self.exact_type(&ty);
        let definition = &self.structs[struct_def_id(id)];
        let geometry =
            lir::StorageGeometryV1::new(self.target_profile, definition.size, definition.align)?;
        let contract = definition
            .c_layout()
            .ok_or(StorageLoweringError::InvalidRepresentation(
                "C ABI storage requires a C-layout struct",
            ))?;
        let source_fields = self.module.structs[id].declared_fields();
        let physical_fields =
            definition
                .c_fields()
                .ok_or(StorageLoweringError::InvalidRepresentation(
                    "missing physical C fields",
                ))?;
        if source_fields.len() != physical_fields.len() {
            return Err(StorageLoweringError::InvalidRepresentation(
                "C-layout source and physical field sequences must align",
            ));
        }
        if physical_fields.is_empty() {
            return Err(lir::StorageReplayError::EmptyCLayout.into());
        }
        let context = LoweringContext::new(self.target_profile);
        let mut cursor = lir::StorageLayoutCursorV1::new(
            self.target_profile,
            lir::StoragePlacementPolicyV1::CLayout(contract),
        )?;
        let fields = source_fields
            .iter()
            .zip(physical_fields)
            .map(|(source, physical)| {
                if source.identity != physical.identity {
                    return Err(StorageLoweringError::InvalidRepresentation(
                        "C-layout field identity changed during lowering",
                    ));
                }
                let storage = self.storage(&source.ty)?;
                let (size, alignment) = safepoints::lir_size_align(
                    &context,
                    &physical.ty.storage_type(),
                    self.structs,
                    self.enums,
                )?;
                let placement = cursor.push(lir::StorageGeometryV1::new(
                    self.target_profile,
                    size,
                    alignment,
                )?)?;
                if physical.layout.offset != placement.offset()
                    || physical.layout.access_align != placement.access_alignment().get()
                {
                    return Err(StorageLoweringError::InvalidRepresentation(
                        "C field placement differs from checked layout replay",
                    ));
                }
                Ok(identity::CanonicalCAbiLayoutField::new(
                    source.identity,
                    physical.layout.offset,
                    storage,
                ))
            })
            .collect::<StorageResult<Vec<_>>>()?;
        if cursor.finish()? != geometry {
            return Err(StorageLoweringError::InvalidRepresentation(
                "C struct extent differs from checked layout replay",
            ));
        }
        let layout = identity::CanonicalCAbiLayout::new(
            exact_type,
            geometry.size(),
            NonZeroU64::new(geometry.alignment().get()).ok_or(
                StorageLoweringError::InvalidRepresentation("C layout alignment is zero"),
            )?,
            layout_override(contract.aligned),
            layout_override(contract.packed),
            fields,
        );
        let record = identity::CanonicalCAbiLayoutFingerprintRecord::new(layout)
            .expect("canonical C ABI layouts have encodable identities");
        let fingerprint = record.fingerprint();
        self.layouts.push(record);
        self.layout_fingerprints.insert(id, fingerprint);
        self.visiting_layouts.remove(&id);
        Ok(fingerprint)
    }
}
