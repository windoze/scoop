use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpressionTypeSiteV1 {
    position: ExecutableExpressionPosition,
    origin: ConcreteExpressionOrigin,
    role: HirExpressionTypeRoleV1,
    exact: PersistentExactTypeId,
}

impl HirExpressionTypeSiteV1 {
    pub const fn new(
        position: ExecutableExpressionPosition,
        origin: ConcreteExpressionOrigin,
        role: HirExpressionTypeRoleV1,
        exact: PersistentExactTypeId,
    ) -> Self {
        Self {
            position,
            origin,
            role,
            exact,
        }
    }
    pub const fn position(&self) -> ExecutableExpressionPosition {
        self.position
    }
    pub const fn origin(&self) -> &ConcreteExpressionOrigin {
        &self.origin
    }
    pub const fn role(&self) -> HirExpressionTypeRoleV1 {
        self.role
    }
    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }
}
