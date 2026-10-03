use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_pointer_expression(
        &mut self,
        kind: &hir::DefaultExpressionKindV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        use hir::DefaultExpressionKindV1 as Kind;
        Ok(match kind {
            Kind::PtrFromNonZeroULong(operand)
            | Kind::PtrToULong(operand)
            | Kind::PtrCast(operand) => {
                let operand =
                    Box::new(self.materialize_imported_default_expression(operand, context)?);
                match kind {
                    Kind::PtrFromNonZeroULong(_) => hir::ExprKind::PtrFromNonZeroULong(operand),
                    Kind::PtrToULong(_) => hir::ExprKind::PtrToULong(operand),
                    _ => hir::ExprKind::PtrCast(operand),
                }
            }
            Kind::PtrLoad { pointer, offset }
            | Kind::PtrStore {
                pointer, offset, ..
            } => {
                let pointer =
                    Box::new(self.materialize_imported_default_expression(pointer, context)?);
                let offset = offset
                    .as_ref()
                    .map(|offset| {
                        self.materialize_imported_default_expression(offset, context)
                            .map(Box::new)
                    })
                    .transpose()?;
                match kind {
                    Kind::PtrStore { value, .. } => hir::ExprKind::PtrStore {
                        pointer,
                        offset,
                        value: Box::new(
                            self.materialize_imported_default_expression(value, context)?,
                        ),
                    },
                    _ => hir::ExprKind::PtrLoad { pointer, offset },
                }
            }
            Kind::PtrOffset {
                pointer,
                offset,
                subtract,
            } => hir::ExprKind::PtrOffset {
                pointer: Box::new(self.materialize_imported_default_expression(pointer, context)?),
                offset: Box::new(self.materialize_imported_default_expression(offset, context)?),
                subtract: (*subtract).into(),
            },
            Kind::AddressOf(hir::DefaultPlaceV1::Local { local }) => {
                let value = context.locals.get(local).ok_or_else(|| {
                    ImportedDefaultMaterializationError::UnknownLocal(local.clone())
                })?;
                let hir::ExprKind::Local(local) = value.kind else {
                    return Err(ImportedDefaultMaterializationError::Plan(
                        "an imported address operand must retain its local storage".into(),
                    ));
                };
                hir::ExprKind::AddressOf(hir::Place::Local(local))
            }
            Kind::AddressOf(hir::DefaultPlaceV1::Global { property }) => hir::ExprKind::AddressOf(
                self.external_global_place(*property)
                    .map_err(ImportedDefaultMaterializationError::Plan)?,
            ),
            Kind::SizeOf(ty) | Kind::AlignOf(ty) => {
                let ty = self
                    .imported_default_type_with_bindings(ty, context.bindings)
                    .map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(error.to_string())
                    })?;
                match kind {
                    Kind::SizeOf(_) => hir::ExprKind::SizeOf(ty),
                    _ => hir::ExprKind::AlignOf(ty),
                }
            }
            _ => unreachable!("pointer materialization receives a pointer or layout node"),
        })
    }
}
