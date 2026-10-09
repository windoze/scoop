use super::*;

impl Expr {
    pub(crate) fn is_diverging(&self) -> bool {
        matches!(self.kind, ExprKind::Diverging { .. })
    }

    pub(crate) fn diverging(prefix: Vec<Self>, terminal: Self) -> Self {
        Self::new(
            terminal.ty.clone(),
            ExprKind::Diverging {
                prefix,
                terminal: Box::new(terminal),
            },
        )
    }

    pub(super) fn returning_operands<const N: usize>(
        operands: [Self; N],
        build: impl FnOnce([Self; N]) -> Self,
    ) -> Self {
        if let Some(index) = operands.iter().position(Self::is_diverging) {
            let mut operands = operands.into_iter();
            let prefix = operands.by_ref().take(index).collect();
            let terminal = operands.next().expect("the diverging operand was found");
            Self::diverging(prefix, terminal)
        } else {
            build(operands)
        }
    }
}
