//! Internal semantic model used while producing complete HIR.

use std::collections::HashMap;

use scoop_ast as ast;
use scoop_ast::Span;
use scoop_hir as hir;
use scoop_hir::{ClassId, EnumId, FunctionId, InterfaceId, ObjectId, StructId, TypeId};

use crate::Lowerer;

/// A resolved function signature. Kept separate from `hir::Function`
/// because parameter locals can only be allocated while the body (and
/// its `locals` arena) is being lowered; signatures must be known
/// before any body, so calls resolve regardless of declaration order.
#[derive(Clone)]
pub(crate) struct FnSig {
    pub(crate) is_suspend: bool,
    /// Validated language-level operator role. It participates in override
    /// and interface matching instead of being inferred from the name.
    pub(crate) modifiers: hir::CallableModifiers,
    pub(crate) attributes: hir::FunctionAttributes,
    /// Number of owner parameters at the front of `type_params`.
    pub(crate) owner_type_param_count: usize,
    pub(crate) type_params: Vec<hir::TypeParamDecl>,
    pub(crate) params: Vec<FnParam>,
    pub(crate) return_ty: TypeId,
}

#[derive(Clone)]
pub(crate) struct FnParam {
    pub(crate) name: ast::Ident,
    /// Source-call protocol. The runtime type stays on `ty`; a vararg keeps
    /// its element type separately so argument mapping never reconstructs it
    /// from the `Array<T>` application.
    pub(crate) calling: FnParamCalling,
    pub(crate) ty: TypeId,
}

#[derive(Debug, Clone)]
pub(crate) enum FnParamCalling {
    Required,
    Default {
        expression: ast::Expr,
    },
    Vararg {
        element_ty: TypeId,
        omission: FnVarargOmission,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum FnVarargOmission {
    EmptyArray,
    Default { expression: ast::Expr },
}

/// How a variant was declared (spec 4.2). `hir::Variant` normalizes
/// the four surface forms into a field list, so the lowerer keeps the
/// form on the side: patterns must use the matching shape (positional
/// patterns for positional variants, field patterns for named ones;
/// constructor-style variants accept both).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VariantStyle {
    Unit,
    Positional,
    Named,
    Constructor,
}

/// The type a member function belongs to (M6). Method `Function`s are
/// registered directly on their nominal owner and carry the
/// owner's type and modality in `Function::method`; the receiver is `params[0]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Owner {
    Class(ClassId),
    Interface(InterfaceId),
    Struct(StructId),
    Enum(EnumId),
    Object(ObjectId),
}

/// A declaration whose instances have inline value layout. Struct and enum
/// ids remain distinct even when their arena indices happen to agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ValueLayoutTemplate {
    Struct(StructId),
    Enum(EnumId),
}

impl Owner {
    pub(crate) const fn as_nominal_owner(self) -> hir::NominalOwner {
        match self {
            Self::Class(id) => hir::NominalOwner::Class(id),
            Self::Interface(id) => hir::NominalOwner::Interface(id),
            Self::Struct(id) => hir::NominalOwner::Struct(id),
            Self::Enum(id) => hir::NominalOwner::Enum(id),
            Self::Object(id) => hir::NominalOwner::Object(id),
        }
    }

    pub(crate) const fn from_nominal_owner(owner: hir::NominalOwner) -> Self {
        match owner {
            hir::NominalOwner::Class(id) => Self::Class(id),
            hir::NominalOwner::Interface(id) => Self::Interface(id),
            hir::NominalOwner::Struct(id) => Self::Struct(id),
            hir::NominalOwner::Enum(id) => Self::Enum(id),
            hir::NominalOwner::Object(id) => Self::Object(id),
        }
    }
}

/// Lowering-time target of a nominal declaration lookup. The target kind is
/// explicit so owner qualification never probes several name maps or relies
/// on coincident arena indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NominalTarget {
    Class(ClassId),
    Interface(InterfaceId),
    Struct(StructId),
    Enum(EnumId),
    Object(ObjectId),
}

impl NominalTarget {
    pub(crate) const fn owner(self) -> Owner {
        match self {
            Self::Class(id) => Owner::Class(id),
            Self::Interface(id) => Owner::Interface(id),
            Self::Struct(id) => Owner::Struct(id),
            Self::Enum(id) => Owner::Enum(id),
            Self::Object(id) => Owner::Object(id),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum IntrinsicTypeOwner {
    Struct(StructId),
    Class(ClassId),
}

/// A member declaration together with the complete host substitution at the
/// lookup site. Inherited generic members may have host arguments different
/// from the receiver's own arguments, so carrying this relation is mandatory.
#[derive(Clone)]
pub(crate) struct CallableCandidate {
    pub(crate) function: FunctionId,
    pub(crate) owner: CallableCandidateOwner,
    pub(crate) source: CallableCandidateSource,
    pub(crate) access: CallableCandidateAccess,
}

#[derive(Clone)]
pub(crate) enum CallableCandidateAccess {
    Lookup(hir::LookupAccessWitness),
    Inheritance,
    CompilerGenerated,
}

impl CallableCandidate {
    pub(crate) fn function(
        function: FunctionId,
        owner_arguments: Vec<TypeId>,
        access: hir::LookupAccessWitness,
    ) -> Self {
        Self {
            function,
            owner: CallableCandidateOwner::Function { owner_arguments },
            source: CallableCandidateSource::Direct,
            access: CallableCandidateAccess::Lookup(access),
        }
    }

    pub(crate) fn inheritance_method(
        function: FunctionId,
        owner: hir::MethodOwnerApplication,
    ) -> Self {
        Self {
            function,
            owner: CallableCandidateOwner::Method(owner),
            source: CallableCandidateSource::Direct,
            access: CallableCandidateAccess::Inheritance,
        }
    }

    pub(crate) fn compiler_generated_method(
        function: FunctionId,
        owner: hir::MethodOwnerApplication,
    ) -> Self {
        Self {
            function,
            owner: CallableCandidateOwner::Method(owner),
            source: CallableCandidateSource::Direct,
            access: CallableCandidateAccess::CompilerGenerated,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum CallableCandidateOwner {
    Function { owner_arguments: Vec<TypeId> },
    Method(hir::MethodOwnerApplication),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallableCandidateSource {
    Direct,
    ClassBound {
        receiver_parameter: hir::TypeParamId,
        bound: hir::ClassApplicationId,
        member: hir::FunctionId,
    },
    InterfaceBound {
        receiver_parameter: hir::TypeParamId,
        bound: hir::InterfaceApplicationId,
        member: hir::InterfaceMethodId,
    },
}

/// Lexical permission to invoke a suspend callable. Keeping an explicit,
/// non-empty stack prevents declaration-owned initialization code from
/// accidentally inheriting permission from a surrounding suspend body.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuspensionContext {
    Forbidden(ForbiddenSuspendContext),
    SuspendFunction,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForbiddenSuspendContext {
    TopLevel,
    Function,
    DefaultExpression,
    ConstructorDelegation,
    ConstructorInitialization,
}

pub(crate) fn lower_type_param_decl(
    param: &ast::TypeParamDecl,
    id: hir::TypeParamId,
) -> hir::TypeParamDecl {
    hir::TypeParamDecl {
        id,
        name: param.name.text.clone(),
        // Bounds are resolved after every nominal declaration has entered the
        // type namespace. This temporary lowerer state never crosses the HIR
        // output boundary; successful lowering replaces it completely.
        bounds: hir::TypeParamBounds::Unconstrained,
        span: param.span,
    }
}

impl Owner {
    /// A human-readable host description for diagnostics
    /// ("class `C`", "struct `S`", ...).
    pub(crate) fn describe(&self, lowerer: &Lowerer) -> String {
        match *self {
            Owner::Class(_) => format!("class `{}`", self.describe_name(lowerer)),
            Owner::Interface(_) => format!("interface `{}`", self.describe_name(lowerer)),
            Owner::Struct(_) => format!("struct `{}`", self.describe_name(lowerer)),
            Owner::Enum(_) => format!("enum `{}`", self.describe_name(lowerer)),
            Owner::Object(_) => format!("object `{}`", self.describe_name(lowerer)),
        }
    }

    /// The bare host name, used to qualify method symbols
    /// (`Owner.method`).
    pub(crate) fn describe_name(&self, lowerer: &Lowerer) -> String {
        let (name, owner) = match *self {
            Owner::Class(id) => (&lowerer.classes[id].name, lowerer.classes[id].owner),
            Owner::Interface(id) => (&lowerer.interfaces[id].name, lowerer.interfaces[id].owner),
            Owner::Struct(id) => (&lowerer.structs[id].name, lowerer.structs[id].owner),
            Owner::Enum(id) => (&lowerer.enums[id].name, lowerer.enums[id].owner),
            Owner::Object(id) => (&lowerer.objects[id].name, lowerer.objects[id].owner),
        };
        match owner {
            Some(owner) => format!(
                "{}.{}",
                Owner::from_nominal_owner(owner).describe_name(lowerer),
                name
            ),
            None => name.clone(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum CaptureSource {
    /// A local in the immediately enclosing callable body.
    Local(hir::LocalId),
    /// A binding already supplied by the immediately enclosing closure.
    Capture(hir::BindingId),
    /// A value parameter of the enclosing typed constructor initializer.
    ConstructorParam(hir::ConstructorParamId),
}

#[derive(Clone)]
pub(crate) struct AvailableCapture {
    pub(crate) binding: hir::BindingId,
    pub(crate) ty: TypeId,
    pub(crate) mutable: bool,
    pub(crate) source: CaptureSource,
    pub(crate) declaration_depth: usize,
}

#[derive(Clone)]
pub(crate) struct PendingCapture {
    pub(crate) binding: hir::BindingId,
    pub(crate) name: String,
    pub(crate) ty: TypeId,
    pub(crate) first_use_span: Span,
    pub(crate) source: CaptureSource,
    pub(crate) declaration_depth: usize,
}

#[derive(Clone)]
pub(crate) struct CaptureContext {
    pub(crate) available: HashMap<String, AvailableCapture>,
    pub(crate) captures: Vec<PendingCapture>,
    pub(crate) by_binding: HashMap<hir::BindingId, usize>,
}

#[derive(Clone, Default)]
pub(crate) struct ReturnInference {
    pub(crate) value_types: Vec<TypeId>,
    pub(crate) saw_bare: bool,
}
