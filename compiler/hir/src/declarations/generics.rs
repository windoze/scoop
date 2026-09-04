use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParamDecl {
    pub id: TypeParamId,
    pub name: String,
    /// Complete declaration-site constraint set. The sum type makes kind
    /// bounds and interface upper bounds mutually exclusive by construction.
    pub bounds: TypeParamBounds,
    pub span: Span,
}

impl TypeParamDecl {
    /// Representation kind implied by this parameter's constraints. Interface
    /// upper bounds are reference capabilities, but do not make the generic
    /// value itself a `ref`-kind parameter: value types may implement them and
    /// are boxed only at an actual interface crossing.
    pub fn kind(&self) -> TypeParamKind {
        match self.bounds {
            TypeParamBounds::Value { .. } => TypeParamKind::Value,
            TypeParamBounds::Ref { .. } => TypeParamKind::Ref,
            TypeParamBounds::Unconstrained | TypeParamBounds::Interfaces(_) => TypeParamKind::Any,
        }
    }

    pub fn interface_bounds(&self) -> &[InterfaceBound] {
        match &self.bounds {
            TypeParamBounds::Interfaces(bounds) => bounds,
            TypeParamBounds::Unconstrained
            | TypeParamBounds::Value { .. }
            | TypeParamBounds::Ref { .. } => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeParamBounds {
    Unconstrained,
    Value {
        span: Span,
    },
    Ref {
        span: Span,
    },
    /// Ordered, distinct, fully resolved interface applications.
    Interfaces(Vec<InterfaceBound>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceBound {
    /// Complete interface application.  The bound cannot name a declaration
    /// without its arguments or another nominal kind.
    pub application: InterfaceApplicationId,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeParamKind {
    Any,
    Value,
    Ref,
}
