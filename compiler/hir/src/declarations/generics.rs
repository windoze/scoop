use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParamDecl {
    pub id: TypeParamId,
    pub name: String,
    /// Complete declaration-site constraint set. The sum type makes kind
    /// bounds and nominal upper bounds mutually exclusive by construction.
    pub bounds: TypeParamBounds,
    pub span: Span,
}

impl TypeParamDecl {
    /// Representation kind implied by this parameter's constraints. A class
    /// upper bound is a reference-type guarantee. Interface-only bounds remain
    /// capabilities that value types may satisfy through explicit conformance.
    pub fn kind(&self) -> TypeParamKind {
        match &self.bounds {
            TypeParamBounds::Value { .. } => TypeParamKind::Value,
            TypeParamBounds::Ref { .. } => TypeParamKind::Ref,
            TypeParamBounds::Nominal(bounds) if bounds.class.is_some() => TypeParamKind::Ref,
            TypeParamBounds::ImportedNominal(bounds) if bounds.class.is_some() => {
                TypeParamKind::Ref
            }
            TypeParamBounds::Unconstrained
            | TypeParamBounds::Nominal(_)
            | TypeParamBounds::ImportedNominal(_) => TypeParamKind::Any,
        }
    }

    pub fn class_bound(&self) -> Option<&ClassBound> {
        match &self.bounds {
            TypeParamBounds::Nominal(bounds) => bounds.class.as_ref(),
            TypeParamBounds::ImportedNominal(_)
            | TypeParamBounds::Unconstrained
            | TypeParamBounds::Value { .. }
            | TypeParamBounds::Ref { .. } => None,
        }
    }

    pub fn interface_bounds(&self) -> &[InterfaceBound] {
        match &self.bounds {
            TypeParamBounds::Nominal(bounds) => &bounds.interfaces,
            TypeParamBounds::ImportedNominal(_)
            | TypeParamBounds::Unconstrained
            | TypeParamBounds::Value { .. }
            | TypeParamBounds::Ref { .. } => &[],
        }
    }

    pub fn nominal_bounds_in_source_order(&self) -> Vec<NominalBoundRef<'_>> {
        if let TypeParamBounds::ImportedNominal(bounds) = &self.bounds {
            let mut ordered = bounds
                .class
                .iter()
                .map(NominalBoundRef::ImportedClass)
                .chain(
                    bounds
                        .interfaces
                        .iter()
                        .map(NominalBoundRef::ImportedInterface),
                )
                .collect::<Vec<_>>();
            ordered.sort_by_key(|bound| bound.span().start);
            return ordered;
        }
        let TypeParamBounds::Nominal(bounds) = &self.bounds else {
            return Vec::new();
        };
        let mut ordered =
            Vec::with_capacity(usize::from(bounds.class.is_some()) + bounds.interfaces.len());
        if let Some(bound) = &bounds.class {
            ordered.push(NominalBoundRef::Class(bound));
        }
        ordered.extend(bounds.interfaces.iter().map(NominalBoundRef::Interface));
        ordered.sort_by_key(|bound| bound.span().start);
        ordered
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeParamBounds {
    Unconstrained,
    Value { span: Span },
    Ref { span: Span },
    Nominal(NominalBounds),
    ImportedNominal(ImportedNominalBounds),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedNominalBounds {
    pub class: Option<ImportedNominalTypeBound>,
    pub interfaces: Vec<ImportedNominalTypeBound>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedNominalTypeBound {
    pub ty: TypeId,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NominalBounds {
    /// At most one complete class application, enforced structurally.
    pub class: Option<ClassBound>,
    /// Ordered, distinct, complete interface applications.
    pub interfaces: Vec<InterfaceBound>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassBound {
    pub application: ClassApplicationId,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceBound {
    /// Complete interface application.  The bound cannot name a declaration
    /// without its arguments or another nominal kind.
    pub application: InterfaceApplicationId,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NominalBoundRef<'a> {
    Class(&'a ClassBound),
    Interface(&'a InterfaceBound),
    ImportedClass(&'a ImportedNominalTypeBound),
    ImportedInterface(&'a ImportedNominalTypeBound),
}

impl NominalBoundRef<'_> {
    pub fn span(self) -> Span {
        match self {
            Self::Class(bound) => bound.span,
            Self::Interface(bound) => bound.span,
            Self::ImportedClass(bound) | Self::ImportedInterface(bound) => bound.span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeParamKind {
    Any,
    Value,
    Ref,
}
