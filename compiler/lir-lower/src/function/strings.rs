use super::*;

impl FunctionLowerer<'_> {
    pub(super) fn lower_string_copy(
        &mut self,
        operation: &mir::StringCopyOperation<mir::Expr>,
        result: &mir::Type,
    ) -> StorageResult<lir::Value> {
        use lir::StringCopySource as Source;
        use mir::StringCopyOperation as Copy;
        let source = match operation {
            Copy::Zeroed(length) => Source::Zeroed(self.lower_expr(length)?),
            Copy::Utf8 { pointer, length } => Source::Utf8 {
                pointer: self.lower_expr(pointer)?,
                length: self.lower_expr(length)?,
            },
            Copy::Slice { string, start, end } => Source::Slice {
                string: self.lower_expr(string)?,
                start: self.lower_expr(start)?,
                end: self.lower_expr(end)?,
            },
            Copy::FromBytes(array) => {
                let mir::Type::Class(class) = array.ty else {
                    unreachable!("String byte copies have an exact array source");
                };
                Source::Bytes {
                    array: self.lower_expr(array)?,
                    array_type: self.array_type_id(class),
                }
            }
            Copy::ToBytes(string) => Source::String(self.lower_expr(string)?),
            Copy::FromBytePrefix { array, length } => Source::BytePrefix {
                array_type: self.byte_copy_array_type(array),
                array: self.lower_expr(array)?,
                length: self.lower_expr(length)?,
            },
            Copy::StringIntoBytes {
                string,
                target,
                start,
            } => {
                let source = lir::ByteCopySource::String(self.lower_expr(string)?);
                let array_type = self.byte_copy_array_type(target);
                let target = self.lower_expr(target)?;
                let start = self.lower_expr(start)?;
                self.push(lir::Instruction::ByteCopy {
                    source,
                    target,
                    start,
                    array_type,
                });
                return Ok(self.unit_value());
            }
            Copy::BytePrefixIntoBytes {
                source,
                target,
                length,
            } => {
                let source_type = self.byte_copy_array_type(source);
                let array = self.lower_expr(source)?;
                let array_type = self.byte_copy_array_type(target);
                let target = self.lower_expr(target)?;
                let length = self.lower_expr(length)?;
                let source = lir::ByteCopySource::Prefix {
                    array,
                    array_type: source_type,
                    length,
                };
                self.push(lir::Instruction::ByteCopy {
                    source,
                    target,
                    start: lir::Value::IntegerConst(lir::LirIntegerConstant::Signed64(0)),
                    array_type,
                });
                return Ok(self.unit_value());
            }
            Copy::RawIntoBytes {
                pointer,
                length,
                target,
                start,
            } => {
                let pointer = self.lower_expr(pointer)?;
                let length = self.lower_expr(length)?;
                let array_type = self.byte_copy_array_type(target);
                let target = self.lower_expr(target)?;
                let start = self.lower_expr(start)?;
                self.push(lir::Instruction::ByteCopy {
                    source: lir::ByteCopySource::Raw { pointer, length },
                    target,
                    start,
                    array_type,
                });
                return Ok(self.unit_value());
            }
        };
        let target = match result {
            mir::Type::String => {
                lir::StringCopyTarget::String(self.type_descriptors.for_type(result))
            }
            mir::Type::Class(class) => lir::StringCopyTarget::Bytes(self.array_type_id(*class)),
            _ => unreachable!("String byte copies produce a String or exact byte array"),
        };
        let out = self.new_temp(lir::MANAGED_PTR);
        let safepoint = self.new_safepoint(lir::SafepointSiteRole::ManagedCall);
        let overflow_message = self.trap_message("String copy size overflow");
        self.push(lir::Instruction::StringCopy {
            out,
            source,
            target,
            overflow_message,
            safepoint,
            live: lir::StatepointLiveSet::default(),
        });
        Ok(lir::Value::Temp(out))
    }

    fn byte_copy_array_type(&self, array: &mir::Expr) -> lir::ArrayTypeId {
        let mir::Type::Class(class) = array.ty else {
            unreachable!("byte copies consume an exact byte array");
        };
        self.array_type_id(class)
    }

    pub(super) fn lower_string_byte(
        &mut self,
        operation: &mir::StringByteOperation<mir::Expr>,
    ) -> StorageResult<lir::Value> {
        Ok(match operation {
            mir::StringByteOperation::Read { string, index } => {
                let string = self.lower_expr(string)?;
                let index = self.lower_expr(index)?;
                let out = self.new_temp(lir::LirType::I8);
                self.push(lir::Instruction::StringByteLoad { out, string, index });
                lir::Value::Temp(out)
            }
            mir::StringByteOperation::Write {
                string,
                index,
                value,
            } => {
                let string = self.lower_expr(string)?;
                let index = self.lower_expr(index)?;
                let value = self.lower_expr(value)?;
                self.push(lir::Instruction::StringByteStore {
                    string,
                    index,
                    value,
                });
                self.unit_value()
            }
        })
    }

    pub(super) fn lower_string_concat(
        &mut self,
        parts: &[mir::StringConcatPart],
    ) -> StorageResult<lir::Value> {
        let parts = parts
            .iter()
            .map(|part| {
                Ok(match part {
                    mir::StringConcatPart::Value(value) => {
                        lir::StringConcatPart::Value(self.lower_expr(value)?)
                    }
                    mir::StringConcatPart::Literal(text) => {
                        lir::StringConcatPart::Literal(text.clone())
                    }
                })
            })
            .collect::<StorageResult<Vec<_>>>()?;
        let out = self.new_temp(lir::MANAGED_PTR);
        let safepoint = self.new_safepoint(lir::SafepointSiteRole::ManagedCall);
        let overflow_message = self.trap_message("String size overflow");
        self.push(lir::Instruction::StringConcat {
            out,
            parts,
            overflow_message,
            descriptor: self.type_descriptors.for_type(&mir::Type::String),
            safepoint,
            live: lir::StatepointLiveSet::default(),
        });
        Ok(lir::Value::Temp(out))
    }
}
