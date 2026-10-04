//! Callable declarations, effects, generic identity, and native boundaries.

use super::*;

#[derive(Debug, Clone)]
pub struct Function {
    pub signature: CallableSignature,
    pub access: DeclarationAccess,
    /// Complete declaration identity. Generic functions carry their distinct
    /// template id directly; consumers never recover it by scanning the
    /// `generic_functions` arena or by inspecting `type_params`.
    pub genericity: FunctionGenericity,
    pub kind: FunctionKind,
    /// Member metadata; the receiver of a method is the first entry of
    /// `params` (named `this`). Top-level functions have `None`.
    pub method: Option<Method>,
}

impl std::ops::Deref for Function {
    type Target = CallableSignature;

    fn deref(&self) -> &Self::Target {
        &self.signature
    }
}

impl std::ops::DerefMut for Function {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.signature
    }
}

/// The complete callable signature shared by source and decoded definitions.
#[derive(Debug, Clone)]
pub struct CallableSignature {
    pub context_parameters: Vec<crate::ContextParameter>,
    pub name: String,
    /// Whether calls use the coroutine ABI rather than the ordinary ABI.
    pub is_suspend: bool,
    /// The operator/infix contract shared by free functions, extensions,
    /// local functions and members.
    pub modifiers: CallableModifiers,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub attributes: FunctionAttributes,
    /// Inferred from this implementation, independent of its source NoGC ABI.
    pub release_callability: ReleaseCallability,
    pub span: Span,
}

impl CallableSignature {
    pub fn from_source_effects(
        name: String,
        params: Vec<Param>,
        return_ty: TypeId,
        effects: CallableSourceEffectsV1,
        release_callability: ReleaseCallability,
        span: Span,
    ) -> Self {
        Self {
            context_parameters: Vec::new(),
            name,
            is_suspend: effects.execution() == scoop_identity::Effect::Suspend,
            modifiers: effects.callable_modifiers(),
            params,
            return_ty,
            attributes: effects.function_attributes(),
            release_callability,
            span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CallableModifiers {
    pub operator: Option<OperatorKind>,
    /// Property-delegation protocol identity. These roles are intentionally
    /// disjoint from the ordinary operator table: source-name equality alone
    /// never makes a callable participate in delegation.
    pub property_delegate_operator: Option<PropertyDelegateOperatorKind>,
    pub is_infix: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyDelegateOperatorKind {
    ProvideDelegate,
    GetValue,
    SetValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionGenericity {
    Plain,
    Generic {
        definition: GenericFunctionId,
        parameters: Vec<TypeParamDecl>,
    },
    /// An ordinary method whose signature/body depends only on its nominal
    /// owner's parameters. Its concrete identity comes from a
    /// `MethodApplication`, not a generic-function application.
    OwnerParameterizedMethod {
        owner_parameters: Vec<TypeParamDecl>,
        no_gc_type_params: Vec<TypeParamId>,
        gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
    },
    /// A non-virtual method with its own parameters. The two groups are
    /// structurally separate; no downstream consumer receives a merged
    /// argument vector and an index at which it is expected to split it.
    GenericMethod {
        definition: GenericMethodId,
        owner_parameters: Vec<TypeParamDecl>,
        method_parameters: NonEmptyVec<TypeParamDecl>,
    },
}

impl Function {
    pub fn type_param_count(&self) -> usize {
        match &self.genericity {
            FunctionGenericity::Plain => 0,
            FunctionGenericity::Generic { parameters, .. } => parameters.len(),
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => owner_parameters.len(),
            FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => owner_parameters.len() + method_parameters.len(),
        }
    }

    pub fn owner_type_param_count(&self) -> usize {
        match &self.genericity {
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            }
            | FunctionGenericity::GenericMethod {
                owner_parameters, ..
            } => owner_parameters.len(),
            FunctionGenericity::Plain | FunctionGenericity::Generic { .. } => 0,
        }
    }

    pub fn method_type_param_count(&self) -> usize {
        match &self.genericity {
            FunctionGenericity::GenericMethod {
                method_parameters, ..
            } => method_parameters.len(),
            FunctionGenericity::Plain
            | FunctionGenericity::Generic { .. }
            | FunctionGenericity::OwnerParameterizedMethod { .. } => 0,
        }
    }

    pub fn type_param(&self, id: TypeParamId) -> &TypeParamDecl {
        match &self.genericity {
            FunctionGenericity::Plain => panic!("plain function has no type parameters"),
            FunctionGenericity::Generic { parameters, .. } => parameters
                .iter()
                .find(|parameter| parameter.id == id)
                .expect("generic function owns the referenced type parameter"),
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => owner_parameters
                .iter()
                .find(|parameter| parameter.id == id)
                .expect("method owner owns the referenced type parameter"),
            FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => owner_parameters
                .iter()
                .chain(method_parameters.iter())
                .find(|parameter| parameter.id == id)
                .expect("generic method owns the referenced type parameter"),
        }
    }

    pub fn type_params(&self) -> Vec<&TypeParamDecl> {
        match &self.genericity {
            FunctionGenericity::Plain => Vec::new(),
            FunctionGenericity::Generic { parameters, .. } => parameters.iter().collect(),
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => owner_parameters.iter().collect(),
            FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => owner_parameters
                .iter()
                .chain(method_parameters.iter())
                .collect(),
        }
    }

    pub fn generic_definition(&self) -> Option<GenericFunctionId> {
        match &self.genericity {
            FunctionGenericity::Plain
            | FunctionGenericity::OwnerParameterizedMethod { .. }
            | FunctionGenericity::GenericMethod { .. } => None,
            FunctionGenericity::Generic { definition, .. } => Some(*definition),
        }
    }

    pub fn generic_method_definition(&self) -> Option<GenericMethodId> {
        match self.genericity {
            FunctionGenericity::GenericMethod { definition, .. } => Some(definition),
            FunctionGenericity::Plain
            | FunctionGenericity::Generic { .. }
            | FunctionGenericity::OwnerParameterizedMethod { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionAttributes {
    pub safety: Safety,
    pub gc_effect: GcEffect,
    /// M12 currently supports only cdecl for native-addressable functions.
    pub calling_convention: CallingConvention,
}

impl Default for FunctionAttributes {
    fn default() -> Self {
        Self {
            safety: Safety::Safe,
            gc_effect: GcEffect::Managed,
            calling_convention: CallingConvention::Cdecl,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Safety {
    Safe,
    Unsafe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcEffect {
    Managed,
    NoGc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    Cdecl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternAbi {
    C,
    Scoop,
}

#[derive(Debug, Clone)]
pub struct ExternFunction {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub abi: ExternAbi,
    pub calling_convention: CallingConvention,
    pub gc_effect: GcEffect,
    pub safety: Safety,
    pub params: Vec<TypeId>,
    pub return_type: TypeId,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeId,
    /// Parameters are immutable locals.
    pub local: LocalId,
}

#[derive(Debug, Clone)]
pub enum FunctionKind {
    User(Body),
    /// A declaration-only slot retains its parameter bindings, without an
    /// executable source body. MIR emits the slot's abstract trap.
    Abstract {
        locals: Arena<Local>,
    },
    /// MIR generates the coordinator from its associated initialization unit.
    InitializationEnsure,
    /// Stable source identity for a conditional derived equality method. Its
    /// application-specific ordinary body lives on
    /// `DerivedEqualityApplication` and must be present before concretization
    /// requests this function.
    DerivedEquality,
    /// A validated compiler intrinsic. Raw annotation text does not cross the
    /// AST/HIR boundary: kind and defining provider are both typed and
    /// mandatory.
    Intrinsic(IntrinsicFunction),
    /// A bodyless native declaration. Complete ABI metadata lives in the
    /// independent extern arena and is referenced by a typed id.
    Extern(ExternFunctionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicFunction {
    pub kind: IntrinsicFunctionKind,
    pub provider: IntrinsicProviderId,
}

/// A validated reference to a non-generic `@NoGC` function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoGcCallableRef(FunctionId);

impl NoGcCallableRef {
    pub fn try_from_function(function: FunctionId, functions: &Arena<Function>) -> Option<Self> {
        let declaration = &functions[function];
        (declaration.attributes.gc_effect == GcEffect::NoGc
            && !declaration.is_suspend
            && matches!(&declaration.genericity, FunctionGenericity::Plain)
            && declaration.method.is_some()
            && matches!(
                &declaration.kind,
                FunctionKind::Intrinsic(intrinsic)
                    if intrinsic.kind.integer_gc_effect() == Some(GcEffect::NoGc)
            ))
        .then_some(Self(function))
    }

    pub const fn function(self) -> FunctionId {
        self.0
    }
}

/// A validated reference to a non-generic managed function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagedCallableRef(FunctionId);

impl ManagedCallableRef {
    pub fn try_from_function(function: FunctionId, functions: &Arena<Function>) -> Option<Self> {
        let declaration = &functions[function];
        (declaration.attributes.gc_effect == GcEffect::Managed
            && !declaration.is_suspend
            && matches!(&declaration.genericity, FunctionGenericity::Plain)
            && declaration.method.is_some()
            && matches!(
                &declaration.kind,
                FunctionKind::Intrinsic(intrinsic)
                    if intrinsic.kind.integer_gc_effect() == Some(GcEffect::Managed)
            ))
        .then_some(Self(function))
    }

    pub const fn function(self) -> FunctionId {
        self.0
    }
}

pub type HirIntegerOperation = IntegerOperation;
pub type HirIntegerConversion = IntegerConversion;

#[cfg(test)]
mod tests {
    use super::*;

    fn integer_intrinsic(ty: TypeId, effect: GcEffect) -> Function {
        Function {
            signature: CallableSignature {
                context_parameters: Vec::new(),
                release_callability: Default::default(),
                name: "Int.plus".to_string(),
                is_suspend: false,
                modifiers: CallableModifiers::default(),
                params: Vec::new(),
                return_ty: ty,
                attributes: FunctionAttributes {
                    gc_effect: effect,
                    ..FunctionAttributes::default()
                },
                span: Span::new(0, 0),
            },
            access: DeclarationAccess::public(),
            genericity: FunctionGenericity::Plain,
            kind: FunctionKind::Intrinsic(IntrinsicFunction {
                kind: IntrinsicFunctionKind::Integer(match effect {
                    GcEffect::NoGc => IntegerIntrinsicKind::NoGcOperation {
                        kind: IntegerKind::SIGNED_32,
                        operation: NoGcIntegerOperation::Add,
                    },
                    GcEffect::Managed => IntegerIntrinsicKind::ManagedOperation {
                        kind: IntegerKind::SIGNED_32,
                        operation: IntegerDivRem::Div,
                    },
                }),
                provider: IntrinsicProviderId::from_raw(0),
            }),
            method: Some(Method {
                owner: ty,
                modifier: MethodModifier::Final,
                dispatch: MethodDispatch::Direct,
            }),
        }
    }

    #[test]
    fn effect_refined_targets_are_minted_only_for_matching_functions() {
        let mut types = Arena::new();
        let ty = types.alloc(Type::Integer(IntegerKind::SIGNED_32));
        let mut functions = Arena::new();
        let no_gc = functions.alloc(integer_intrinsic(ty, GcEffect::NoGc));
        let managed = functions.alloc(integer_intrinsic(ty, GcEffect::Managed));

        assert_eq!(
            NoGcCallableRef::try_from_function(no_gc, &functions).map(NoGcCallableRef::function),
            Some(no_gc)
        );
        assert!(ManagedCallableRef::try_from_function(no_gc, &functions).is_none());
        assert_eq!(
            ManagedCallableRef::try_from_function(managed, &functions)
                .map(ManagedCallableRef::function),
            Some(managed)
        );
        assert!(NoGcCallableRef::try_from_function(managed, &functions).is_none());

        functions[no_gc].kind = FunctionKind::Intrinsic(IntrinsicFunction {
            kind: IntrinsicFunctionKind::Integer(IntegerIntrinsicKind::ManagedOperation {
                kind: IntegerKind::SIGNED_32,
                operation: IntegerDivRem::Rem,
            }),
            provider: IntrinsicProviderId::from_raw(0),
        });
        assert!(NoGcCallableRef::try_from_function(no_gc, &functions).is_none());
        assert!(ManagedCallableRef::try_from_function(no_gc, &functions).is_none());
    }
}
