use super::*;

mod storage;

impl FunctionLowerer<'_> {
    pub(super) fn lower_pointer_expr(
        &mut self,
        ty: &mir::Type,
        kind: &mir::ExprKind,
    ) -> StorageResult<lir::Value> {
        Ok(match kind {
            mir::ExprKind::PtrFromNonZeroULong { operand, pointee } => {
                assert_eq!(
                    operand.ty,
                    mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
                    "raw pointer construction consumes canonical ULong",
                );
                let mir::Type::Ptr(result_pointee) = ty else {
                    panic!("PtrFromNonZeroULong result must be a raw pointer")
                };
                assert_eq!(
                    result_pointee.as_ref(),
                    pointee.as_ref(),
                    "PtrFromNonZeroULong result carries its declared pointee",
                );
                let value = self.lower_expr(operand)?;
                let out = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::ULongToPtr { out, value });
                lir::Value::Temp(out)
            }
            mir::ExprKind::PtrToULong(operand) => {
                assert_eq!(
                    *ty,
                    mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
                    "raw pointer bits are exposed as canonical ULong",
                );
                assert!(
                    matches!(&operand.ty, mir::Type::Ptr(_)),
                    "PtrToULong consumes only a raw data pointer",
                );
                let value = self.lower_expr(operand)?;
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::PtrToULong { out, value });
                lir::Value::Temp(out)
            }
            mir::ExprKind::PtrCast { operand, pointee } => {
                let mir::Type::Ptr(result_pointee) = ty else {
                    panic!("PtrCast result must remain a raw pointer")
                };
                assert_eq!(
                    result_pointee.as_ref(),
                    pointee.as_ref(),
                    "PtrCast result carries its declared pointee"
                );
                assert!(
                    matches!(&operand.ty, mir::Type::Ptr(_)),
                    "PtrCast operand must be a raw pointer"
                );
                self.lower_expr(operand)?
            }
            mir::ExprKind::PtrLoad {
                pointer,
                pointee,
                offset,
            } => {
                assert_eq!(
                    ty,
                    pointee.as_ref(),
                    "PtrLoad result must match its pointee"
                );
                let mir::Type::Ptr(pointer_pointee) = &pointer.ty else {
                    panic!("PtrLoad operand must be a raw pointer")
                };
                assert_eq!(
                    pointer_pointee.as_ref(),
                    pointee.as_ref(),
                    "PtrLoad operand pointer must have the declared pointee"
                );
                let pointer = self.lower_expr(pointer)?;
                let pointer = if let Some(offset) = offset {
                    let offset = self.lower_pointer_offset(offset, false)?;
                    self.offset_pointer(pointer, pointee, offset, false)?
                } else {
                    pointer
                };
                match self.pointer_storage(pointee)? {
                    lir::AbiArgument::ElidedZst(representation) => {
                        self.logical_zst_value(pointee, representation)
                    }
                    lir::AbiArgument::Direct(pointee) | lir::AbiArgument::Indirect(pointee) => {
                        let out = self.new_temp(pointee.storage_type().clone());
                        self.push(lir::Instruction::RawLoad {
                            out,
                            pointer,
                            pointee,
                        });
                        lir::Value::Temp(out)
                    }
                }
            }
            mir::ExprKind::PtrStore {
                pointer,
                pointee,
                offset,
                value,
            } => {
                assert_eq!(*ty, mir::Type::Unit, "PtrStore result must be Unit");
                let mir::Type::Ptr(pointer_pointee) = &pointer.ty else {
                    panic!("PtrStore operand must be a raw pointer")
                };
                assert_eq!(
                    pointer_pointee.as_ref(),
                    pointee.as_ref(),
                    "PtrStore operand pointer must have the declared pointee"
                );
                assert_eq!(
                    &value.ty,
                    pointee.as_ref(),
                    "PtrStore value must match its pointee"
                );
                let pointer = self.lower_expr(pointer)?;
                let pointer = if let Some(offset) = offset {
                    let offset = self.lower_pointer_offset(offset, false)?;
                    self.offset_pointer(pointer, pointee, offset, false)?
                } else {
                    pointer
                };
                let value = self.lower_expr(value)?;
                match self.pointer_storage(pointee)? {
                    lir::AbiArgument::ElidedZst(_) => self.unit_value(),
                    lir::AbiArgument::Direct(pointee) | lir::AbiArgument::Indirect(pointee) => {
                        self.push(lir::Instruction::RawStore {
                            pointer,
                            value,
                            pointee,
                        });
                        self.unit_value()
                    }
                }
            }
            mir::ExprKind::PtrOffset {
                pointer,
                pointee,
                offset,
                subtract,
            } => {
                assert_eq!(ty, &pointer.ty, "PtrOffset preserves its pointer type");
                assert_eq!(
                    pointer.ty,
                    mir::Type::Ptr(pointee.clone()),
                    "PtrOffset operand must have the declared pointee",
                );
                let pointer = self.lower_expr(pointer)?;
                let offset = self.lower_pointer_offset(offset, *subtract)?;
                self.offset_pointer(pointer, pointee, offset, *subtract)?
            }
            mir::ExprKind::AddressOf { local, .. } => {
                let local = self.local_slot(*local);
                let out = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::LocalAddress { out, local });
                lir::Value::Temp(out)
            }
            _ => unreachable!("pointer lowering receives only pointer expressions"),
        })
    }
}
