use crate::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub text: String,
    pub span: Span,
}

/// A type annotation, e.g. in struct fields and `val x: T = ...`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeRef {
    pub kind: TypeRefKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeRefKind {
    Named(Ident),
    /// `Name<T1, T2>` — a generic type application (e.g. `Box<Int>`).
    Generic(Ident, Vec<TypeRef>),
    /// `Outer.Nested` / `Outer.Nested<T>` — a declaration-qualified static
    /// nested nominal. Only the final segment may carry type arguments;
    /// generic owners are declaration qualifiers rather than applications.
    Qualified {
        path: Vec<Ident>,
        arguments: Vec<TypeRef>,
    },
    /// A member qualified by an applied host, e.g. `Box<Int>.Companion`.
    /// Ordinary declaration-only paths retain the compact `Qualified` form.
    AppliedMember {
        owner: Box<TypeRef>,
        name: Ident,
        arguments: Vec<TypeRef>,
    },
    Tuple(Vec<TypeRef>),
    /// The `Unit` type name (also written `()` in type position).
    Unit,
    /// `(P1, P2, ...) -> R` / `suspend (P1, P2, ...) -> R`.
    Function(FunctionTypeRef),
    /// `T?` — desugars to `Option<T>` in HIR (spec 7.1).
    Nullable(Box<TypeRef>),
}

impl TypeRef {
    /// Extension declarations share the token prefix `Receiver.name` with a
    /// qualified nominal type. When the parser has greedily consumed the
    /// whole dotted path, split the final identifier back into the callable
    /// name. A generic final segment is a type application and therefore
    /// cannot be split.
    pub fn split_qualified_tail(self) -> Result<(Self, Ident), Self> {
        let Self { kind, span } = self;
        let (mut path, arguments) = match kind {
            TypeRefKind::AppliedMember {
                owner,
                name,
                arguments,
            } if arguments.is_empty() => {
                return Ok((*owner, name));
            }
            TypeRefKind::Qualified { path, arguments } => (path, arguments),
            kind => return Err(Self { kind, span }),
        };
        if !arguments.is_empty() || path.len() < 2 {
            return Err(Self {
                kind: TypeRefKind::Qualified { path, arguments },
                span,
            });
        }
        let name = path.pop().expect("a qualified path has a final segment");
        let start = path
            .first()
            .expect("a split qualified path retains its owner")
            .span
            .start;
        let end = path
            .last()
            .expect("a split qualified path retains its owner")
            .span
            .end;
        let kind = if path.len() == 1 {
            TypeRefKind::Named(path.pop().expect("one owner segment"))
        } else {
            TypeRefKind::Qualified {
                path,
                arguments: Vec::new(),
            }
        };
        Ok((
            Self {
                kind,
                span: Span::new(start, end),
            },
            name,
        ))
    }

    pub fn with_member(self, name: Ident, arguments: Vec<TypeRef>, end: u32) -> Self {
        let span = Span::new(self.span.start, end);
        let kind = match self.kind {
            TypeRefKind::Named(first) => TypeRefKind::Qualified {
                path: vec![first, name],
                arguments,
            },
            TypeRefKind::Qualified {
                mut path,
                arguments: previous,
            } if previous.is_empty() => {
                path.push(name);
                TypeRefKind::Qualified { path, arguments }
            }
            kind => TypeRefKind::AppliedMember {
                owner: Box::new(Self {
                    kind,
                    span: self.span,
                }),
                name,
                arguments,
            },
        };
        Self { kind, span }
    }
}

/// The structural source form of a function type. Parameter names,
/// defaults and `vararg` are deliberately absent from function type
/// identity (spec 8.1.1).
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionTypeRef {
    pub is_suspend: bool,
    pub parameters: Vec<TypeRef>,
    pub return_type: Box<TypeRef>,
}

/// Parser-local identity of one lambda expression.  This is deliberately
/// distinct from every named/anonymous callable identity; HIR remaps it into
/// the Cone-wide lambda arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LambdaId(pub u32);

/// Parser-local identity of one anonymous-function expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnonymousFunctionId(pub u32);

/// Parser-local identity of one callable-reference expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallableReferenceId(pub u32);
