//! Validation and typed registration of compiler-required core declarations.
//!
//! This is the sole HIR boundary that recognizes well-known intrinsic and
//! runtime-facing core contracts. Successful validation emits typed identities;
//! later stages never recover them from declaration names.

use super::*;

impl Lowerer {
    pub(super) fn register_intrinsic_function(
        &mut self,
        function: FunctionId,
        intrinsic: hir::IntrinsicFunction,
        span: Span,
    ) {
        if let Some(&(previous, previous_provider)) = self.intrinsic_functions.get(&intrinsic.kind)
        {
            let previous_name = self.functions[previous].name.clone();
            self.error(
                span,
                format!(
                    "intrinsic `{}` is already defined by provider {} as `{previous_name}`; provider {} cannot define it again",
                    intrinsic.kind.name(),
                    previous_provider.into_raw(),
                    intrinsic.provider.into_raw(),
                ),
            );
            return;
        }
        self.intrinsic_functions
            .insert(intrinsic.kind, (function, intrinsic.provider));
    }

    pub(super) fn validate_intrinsic_type_source_shape(
        &mut self,
        spec: &'static hir::IntrinsicTypeSpec,
        name: &ast::Ident,
        parameters: &[ast::TypeParamDecl],
        where_clause: Option<&ast::WhereClause>,
        representation_omitted: bool,
        span: Span,
    ) {
        if name.text != spec.kind.source_name() {
            self.error(
                name.span,
                format!(
                    "intrinsic type `{}` must be declared with source name `{}`",
                    spec.name,
                    spec.kind.source_name()
                ),
            );
        }
        if !representation_omitted {
            self.error(
                span,
                format!(
                    "intrinsic type `{}` must omit its fields or primary constructor",
                    spec.name
                ),
            );
        }
        let valid_parameters = match spec.kind.parameters() {
            hir::IntrinsicTypeParameters::None => parameters.is_empty(),
            hir::IntrinsicTypeParameters::OneInvariantUnconstrained => {
                matches!(parameters, [parameter]
                    if parameter.variance == ast::Variance::Invariant
                        && parameter.inline_bound.is_none())
                    && where_clause.is_none()
            }
        };
        if !valid_parameters {
            self.error(
                span,
                format!(
                    "intrinsic type `{}` has an invalid type-parameter declaration",
                    spec.name
                ),
            );
        }
    }

    pub(super) fn register_intrinsic_type(
        &mut self,
        intrinsic: hir::IntrinsicTypeDeclaration,
        owner: IntrinsicTypeOwner,
        span: Span,
    ) {
        if let Some(&(_previous, previous_provider)) =
            self.intrinsic_type_owners.get(&intrinsic.kind)
        {
            self.error(
                span,
                format!(
                    "intrinsic type `{}` is already defined by provider {}; provider {} cannot define it again",
                    intrinsic.kind.name(),
                    previous_provider.into_raw(),
                    intrinsic.provider.into_raw(),
                ),
            );
            return;
        }
        self.intrinsic_type_owners
            .insert(intrinsic.kind, (owner, intrinsic.provider));
    }

    pub(super) fn validate_intrinsic_type_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::IntrinsicTypeCore> {
        let require = |this: &mut Self, kind: hir::IntrinsicTypeKind| {
            let Some(&(owner, _provider)) = this.intrinsic_type_owners.get(&kind) else {
                this.current_file = 0;
                this.error(
                    files[0].span,
                    format!(
                        "scoop.core must define exactly one `{}` intrinsic type",
                        kind.name()
                    ),
                );
                return None;
            };
            Some(owner)
        };
        let int = require(self, hir::IntrinsicTypeKind::Int)?;
        let uint = require(self, hir::IntrinsicTypeKind::UInt)?;
        let boolean = require(self, hir::IntrinsicTypeKind::Boolean)?;
        let string = require(self, hir::IntrinsicTypeKind::String)?;
        let array = require(self, hir::IntrinsicTypeKind::Array)?;
        let mutable_array = require(self, hir::IntrinsicTypeKind::MutableArray)?;
        let (
            IntrinsicTypeOwner::Struct(int),
            IntrinsicTypeOwner::Struct(uint),
            IntrinsicTypeOwner::Struct(boolean),
            IntrinsicTypeOwner::Class(string),
            IntrinsicTypeOwner::Class(array),
            IntrinsicTypeOwner::Class(mutable_array),
        ) = (int, uint, boolean, string, array, mutable_array)
        else {
            unreachable!("the intrinsic registry fixes every declaration target")
        };
        Some(hir::IntrinsicTypeCore {
            int,
            uint,
            boolean,
            string,
            array,
            mutable_array,
        })
    }

    pub(super) fn compiler_exception(
        &mut self,
        name: &str,
        files: &[ast::SourceFile],
        throwable: ClassId,
    ) -> Option<hir::CompilerException> {
        let candidate = self.classes_by_name.get(name).map(|(id, _)| *id);
        let id = candidate.filter(|id| self.class_files[id] < self.user_file_index);
        let Some(id) = id else {
            self.current_file = candidate
                .and_then(|id| self.class_files.get(&id).copied())
                .unwrap_or(0);
            self.error(
                files[0].span,
                format!("scoop.core must define class `{name}`"),
            );
            return None;
        };
        self.current_file = self.class_files[&id];
        let declaration = &self.classes[id];
        let valid = declaration.modifier == hir::ClassModifier::Final
            && declaration.type_params.is_empty()
            && matches!(
                &declaration.representation,
                hir::ClassRepresentation::Declared(constructor) if constructor.is_empty()
            )
            && self.class_descends_from(id, throwable);
        if !valid {
            self.error(
                declaration.span,
                format!(
                    "class `{name}` in scoop.core must be a non-generic final subtype of `Throwable` with a zero-argument constructor"
                ),
            );
        }
        Some(hir::CompilerException {
            constructor: hir::ZeroArgClassConstructor { class: id },
        })
    }

    pub(super) fn validate_exception_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::CompilerExceptionCore> {
        let throwable = self.throwable.map(|(id, _)| id)?;
        self.current_file = self.class_files[&throwable];
        let declaration = &self.classes[throwable];
        let valid_throwable = declaration.modifier == hir::ClassModifier::Open
            && declaration.type_params.is_empty()
            && matches!(
                &declaration.representation,
                hir::ClassRepresentation::Declared(constructor) if constructor.is_empty()
            );
        if !valid_throwable {
            self.error(
                declaration.span,
                "class `Throwable` in scoop.core must be a non-generic open class with a zero-argument constructor"
                    .to_string(),
            );
        }
        let throwable = hir::CompilerException {
            constructor: hir::ZeroArgClassConstructor { class: throwable },
        };
        Some(hir::CompilerExceptionCore {
            throwable,
            unwrap_exception: self.compiler_exception(
                "UnwrapException",
                files,
                throwable.class(),
            )?,
            class_cast_exception: self.compiler_exception(
                "ClassCastException",
                files,
                throwable.class(),
            )?,
            arithmetic_exception: self.compiler_exception(
                "ArithmeticException",
                files,
                throwable.class(),
            )?,
            index_out_of_bounds_exception: self.compiler_exception(
                "IndexOutOfBoundsException",
                files,
                throwable.class(),
            )?,
            illegal_state_exception: self.compiler_exception(
                "IllegalStateException",
                files,
                throwable.class(),
            )?,
        })
    }

    pub(super) fn validate_coroutine_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::CoroutineCore> {
        let continuation = self.require_core_interface("Continuation", files);
        let suspend_task = self.require_core_interface("SuspendTask", files);
        let suspend_registration = self.require_core_interface("SuspendRegistration", files);

        if let Some(id) = continuation {
            self.validate_continuation_contract(id);
        }
        if let Some(id) = suspend_task {
            self.validate_suspend_task_contract(id);
        }
        if let (Some(id), Some(continuation)) = (suspend_registration, continuation) {
            self.validate_suspend_registration_contract(id, continuation);
        }
        let start_coroutine =
            self.require_intrinsic(hir::IntrinsicFunctionKind::CoroutineStart, files);
        let suspend_coroutine =
            self.require_intrinsic(hir::IntrinsicFunctionKind::CoroutineSuspend, files);

        if let (Some(id), Some(continuation), Some(suspend_task)) =
            (start_coroutine, continuation, suspend_task)
        {
            self.validate_coroutine_start(id, continuation, suspend_task);
        }
        if let (Some(id), Some(suspend_registration)) = (suspend_coroutine, suspend_registration) {
            self.validate_coroutine_suspend(id, suspend_registration);
        }

        let continuation_methods = continuation
            .and_then(|id| self.interface_methods.get(&id))
            .and_then(|methods| match methods.as_slice() {
                [resume, resume_with_exception] => Some((*resume, *resume_with_exception)),
                _ => None,
            });
        let suspend_task_run = suspend_task
            .and_then(|id| self.interface_methods.get(&id))
            .and_then(|methods| matches!(methods.as_slice(), [_]).then_some(methods[0]));
        let suspend_registration_register = suspend_registration
            .and_then(|id| self.interface_methods.get(&id))
            .and_then(|methods| matches!(methods.as_slice(), [_]).then_some(methods[0]));
        let (
            Some(continuation),
            Some((continuation_resume, continuation_resume_with_exception)),
            Some(suspend_task),
            Some(suspend_task_run),
            Some(suspend_registration),
            Some(suspend_registration_register),
            Some(start_coroutine),
            Some(suspend_coroutine),
        ) = (
            continuation,
            continuation_methods,
            suspend_task,
            suspend_task_run,
            suspend_registration,
            suspend_registration_register,
            start_coroutine,
            suspend_coroutine,
        )
        else {
            return None;
        };
        Some(hir::CoroutineCore {
            continuation,
            continuation_resume,
            continuation_resume_with_exception,
            suspend_task,
            suspend_task_run,
            suspend_registration,
            suspend_registration_register,
            start_coroutine,
            suspend_coroutine,
        })
    }

    pub(super) fn validate_ffi_core(&mut self, files: &[ast::SourceFile]) -> Option<hir::FfiCore> {
        let ptr = self.ffi_ptr;
        let fun_ptr = self.ffi_fun_ptr;
        let pinned_ptr = self.ffi_pinned_ptr;
        let gc_handle = self.ffi_gc_handle;
        if let Some(id) = ptr {
            self.validate_ptr_struct(id);
        }
        if let Some(id) = fun_ptr {
            self.validate_fun_ptr_struct(id);
        }
        if let Some(id) = pinned_ptr {
            self.validate_ffi_handle_struct(id, "PinnedPtr");
        }
        if let Some(id) = gc_handle {
            self.validate_ffi_handle_struct(id, "GcHandle");
        }

        let ptr_to_uint = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::ToUInt),
            files,
        );
        let ptr_cast = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::Cast),
            files,
        );
        let ptr_load = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::Load),
            files,
        );
        let ptr_load_offset = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::LoadOffset),
            files,
        );
        let ptr_store = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::Store),
            files,
        );
        let ptr_store_offset = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::StoreOffset),
            files,
        );
        let ptr_plus = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::Plus),
            files,
        );
        let ptr_minus = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::Minus),
            files,
        );
        let address_of = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::AddressOf),
            files,
        );
        let size_of = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::SizeOf),
            files,
        );
        let align_of = self.require_intrinsic(
            hir::IntrinsicFunctionKind::Pointer(hir::PointerIntrinsic::AlignOf),
            files,
        );
        let gc_pin_raw = self.require_intrinsic(hir::IntrinsicFunctionKind::GcPinRaw, files);
        let gc_unpin_raw = self.require_intrinsic(hir::IntrinsicFunctionKind::GcUnpinRaw, files);
        let gc_get_handle_raw =
            self.require_intrinsic(hir::IntrinsicFunctionKind::GcGetHandleRaw, files);
        let gc_release_handle_raw =
            self.require_intrinsic(hir::IntrinsicFunctionKind::GcReleaseHandleRaw, files);

        for (id, kind) in [
            (gc_pin_raw, GcIntrinsic::Pin),
            (gc_unpin_raw, GcIntrinsic::Unpin),
            (gc_get_handle_raw, GcIntrinsic::GetHandle),
            (gc_release_handle_raw, GcIntrinsic::ReleaseHandle),
        ] {
            if let Some(id) = id {
                self.validate_gc_intrinsic(id, kind);
            }
        }

        if let Some(ptr) = ptr {
            for (id, kind) in [
                (ptr_to_uint, hir::PointerIntrinsic::ToUInt),
                (ptr_cast, hir::PointerIntrinsic::Cast),
                (ptr_load, hir::PointerIntrinsic::Load),
                (ptr_load_offset, hir::PointerIntrinsic::LoadOffset),
                (ptr_store, hir::PointerIntrinsic::Store),
                (ptr_store_offset, hir::PointerIntrinsic::StoreOffset),
                (ptr_plus, hir::PointerIntrinsic::Plus),
                (ptr_minus, hir::PointerIntrinsic::Minus),
            ] {
                if let Some(id) = id {
                    self.validate_ptr_method_intrinsic(id, ptr, kind);
                }
            }
        }
        if let Some(id) = address_of {
            self.validate_pointer_top_level_intrinsic(id, hir::PointerIntrinsic::AddressOf);
        }
        if let Some(id) = size_of {
            self.validate_pointer_top_level_intrinsic(id, hir::PointerIntrinsic::SizeOf);
        }
        if let Some(id) = align_of {
            self.validate_pointer_top_level_intrinsic(id, hir::PointerIntrinsic::AlignOf);
        }

        Some(hir::FfiCore {
            ptr: ptr?,
            fun_ptr: fun_ptr?,
            pinned_ptr: pinned_ptr?,
            gc_handle: gc_handle?,
            ptr_to_uint: ptr_to_uint?,
            ptr_cast: ptr_cast?,
            ptr_load: ptr_load?,
            ptr_load_offset: ptr_load_offset?,
            ptr_store: ptr_store?,
            ptr_store_offset: ptr_store_offset?,
            ptr_plus: ptr_plus?,
            ptr_minus: ptr_minus?,
            address_of: address_of?,
            size_of: size_of?,
            align_of: align_of?,
            gc_pin_raw: gc_pin_raw?,
            gc_unpin_raw: gc_unpin_raw?,
            gc_get_handle_raw: gc_get_handle_raw?,
            gc_release_handle_raw: gc_release_handle_raw?,
        })
    }

    pub(super) fn validate_foreign_callback_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::ForeignCallbackCore> {
        // The exception-core validator owns the missing-Throwable diagnostic.
        // Callback failure typing cannot be validated until that prerequisite
        // exists, so do not manufacture a second error or unwrap incomplete
        // upstream state here.
        let (_, throwable) = self.throwable?;
        let callback = self.ffi_foreign_callback?;
        let mode = self.require_core_enum("ForeignCallbackMode", files)?;
        let state = self.require_core_enum("ForeignCallbackState", files)?;
        let register =
            self.require_intrinsic(hir::IntrinsicFunctionKind::ForeignCallbackRegister, files);
        let retain =
            self.require_intrinsic(hir::IntrinsicFunctionKind::ForeignCallbackRetain, files);
        let release =
            self.require_intrinsic(hir::IntrinsicFunctionKind::ForeignCallbackRelease, files);
        let query_state =
            self.require_intrinsic(hir::IntrinsicFunctionKind::ForeignCallbackState, files);
        let failure =
            self.require_intrinsic(hir::IntrinsicFunctionKind::ForeignCallbackFailure, files);

        self.current_file = self.struct_files[&callback];
        let callback_decl = &self.structs[callback];
        let callback_valid = callback_decl.type_params.len() == 1
            && callback_decl.type_params[0].kind() == hir::TypeParamKind::Any
            && callback_decl.interfaces.is_empty()
            && callback_decl.attributes.c_layout.is_none()
            && !callback_decl.attributes.interior_mutable
            && matches!(callback_decl.semantic_fields(), [function, context]
                if function.name == "function"
                    && matches!(self.types[function.ty], hir::Type::Struct(application)
                        if self.struct_applications[application].template
                            == self.ffi_fun_ptr.expect("FunPtr core exists")
                            && matches!(self.struct_applications[application].arguments.as_slice(), [arg] if self.is_type_param(*arg, 0)))
                    && context.name == "context"
                    && matches!(self.types[context.ty], hir::Type::Ptr(pointee) if pointee == self.unit));
        if !callback_valid {
            self.error(
                callback_decl.span,
                "core `ForeignCallback<F>` must contain `function: FunPtr<F>` and `context: Ptr<Unit>`"
                    .to_string(),
            );
        }

        self.validate_unit_enum(mode, &["Reusable", "OneShot"]);
        self.validate_unit_enum(state, &["Registered", "Active", "Completed", "Failed"]);

        for (id, operation) in [
            (register, "register"),
            (retain, "retain"),
            (release, "release"),
            (query_state, "state"),
            (failure, "failure"),
        ] {
            let Some(id) = id else { continue };
            self.current_file = self.function_files[&id];
            let function = &self.functions[id];
            let signature = &self.signatures[&id];
            let common = !signature.is_suspend
                && signature.type_params.len() == 1
                && signature.type_params[0].kind() == hir::TypeParamKind::Any
                && signature.attributes.safety == hir::Safety::Unsafe
                && signature.attributes.gc_effect == hir::GcEffect::Managed
                && function.method.is_none();
            let callback_param = |ty| {
                matches!(self.types[ty], hir::Type::Struct(application)
                    if self.struct_applications[application].template == callback
                        && matches!(self.struct_applications[application].arguments.as_slice(), [arg] if self.is_type_param(*arg, 0)))
            };
            let signature_valid = match operation {
                "register" => {
                    matches!(signature.params.as_slice(), [closure, index, mode_param]
                        if closure.ty == self.any
                            && index.ty == self.int
                            && mode_param.ty == self.interned_enum_type(mode))
                        && callback_param(signature.return_ty)
                }
                "retain" => matches!(signature.params.as_slice(), [param]
                    if callback_param(param.ty) && callback_param(signature.return_ty)),
                "release" => matches!(signature.params.as_slice(), [param]
                    if callback_param(param.ty) && signature.return_ty == self.unit),
                "state" => matches!(signature.params.as_slice(), [param]
                    if callback_param(param.ty)
                        && signature.return_ty == self.interned_enum_type(state)),
                "failure" => {
                    matches!(signature.params.as_slice(), [param]
                        if callback_param(param.ty)
                            && matches!(self.types[signature.return_ty], hir::Type::Enum(application)
                                if self.enum_applications[application].template
                                    == self.option_enum.expect("Option core exists")
                                    && self.enum_applications[application].arguments.as_slice()
                                        == [throwable]))
                }
                _ => unreachable!(),
            };
            if !common || !signature_valid {
                self.error(
                    function.span,
                    format!(
                        "intrinsic `foreign_callback_{operation}` has an invalid core signature"
                    ),
                );
            }
        }

        Some(hir::ForeignCallbackCore {
            callback,
            mode,
            state,
            register: register?,
            retain: retain?,
            release: release?,
            query_state: query_state?,
            failure: failure?,
        })
    }

    pub(super) fn require_core_enum(
        &mut self,
        name: &str,
        files: &[ast::SourceFile],
    ) -> Option<EnumId> {
        let candidate = self.enums_by_name.get(name).copied();
        if let Some(id) = candidate
            && self
                .enum_files
                .get(&id)
                .copied()
                .unwrap_or(self.user_file_index)
                < self.user_file_index
        {
            return Some(id);
        }
        self.current_file = 0;
        self.error(
            files[0].span,
            format!("scoop.core must define exactly one `{name}` enum"),
        );
        None
    }

    pub(super) fn validate_unit_enum(&mut self, id: EnumId, names: &[&str]) {
        self.current_file = self.enum_files[&id];
        let declaration = &self.enums[id];
        let valid = declaration.type_params.is_empty()
            && declaration.interfaces.is_empty()
            && declaration.variants.len() == names.len()
            && declaration
                .variants
                .iter()
                .zip(names)
                .all(|(variant, name)| variant.name == *name && variant.fields.is_empty());
        if !valid {
            self.error(
                declaration.span,
                format!(
                    "core `{}` must declare unit variants `{}` in order",
                    declaration.name,
                    names.join("`, `")
                ),
            );
        }
    }

    pub(super) fn interned_enum_type(&self, enum_id: EnumId) -> TypeId {
        self.enum_applications[self.enums[enum_id].self_application].canonical_type
    }

    pub(super) fn validate_ffi_handle_struct(&mut self, id: StructId, name: &str) {
        self.current_file = self.struct_files[&id];
        let declaration = &self.structs[id];
        let valid = declaration.type_params.len() == 1
            && declaration.type_params[0].kind() == hir::TypeParamKind::Ref
            && matches!(declaration.semantic_fields(), [field] if field.name == "raw" && field.ty == self.uint)
            && declaration.interfaces.is_empty()
            && !declaration.attributes.interior_mutable
            && declaration.attributes.c_layout.is_none();
        if !valid {
            self.error(
                declaration.span,
                format!("core `{name}` must be `struct {name}<T : ref>(val raw: UInt)`"),
            );
        }
    }

    pub(super) fn validate_gc_intrinsic(&mut self, id: FunctionId, kind: GcIntrinsic) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let signature = &self.signatures[&id];
        let parameter_matches = match kind {
            GcIntrinsic::Pin | GcIntrinsic::GetHandle => {
                matches!(signature.params.as_slice(), [param] if self.is_type_param(param.ty, 0))
                    && signature.return_ty == self.uint
            }
            GcIntrinsic::Unpin | GcIntrinsic::ReleaseHandle => {
                matches!(signature.params.as_slice(), [param] if param.ty == self.uint)
                    && self.is_type_param(signature.return_ty, 0)
            }
        };
        let valid = !signature.is_suspend
            && signature.type_params.len() == 1
            && signature.type_params[0].kind() == hir::TypeParamKind::Ref
            && signature.attributes.safety == hir::Safety::Unsafe
            && signature.attributes.gc_effect == hir::GcEffect::Managed
            && function.method.is_none()
            && parameter_matches;
        if !valid {
            self.error(
                function.span,
                format!(
                    "intrinsic `{}` has an invalid core GC primitive signature",
                    kind.name()
                ),
            );
        }
    }

    pub(super) fn validate_ptr_struct(&mut self, id: StructId) {
        self.current_file = self.struct_files[&id];
        let declaration = &self.structs[id];
        let valid = declaration.type_params.len() == 1
            && declaration.type_params[0].kind() == hir::TypeParamKind::Value
            && matches!(declaration.semantic_fields(), [field] if field.name == "_rawPointer" && field.ty == self.uint)
            && declaration.interfaces.is_empty()
            && !declaration.attributes.interior_mutable
            && declaration.attributes.c_layout.is_none();
        if !valid {
            self.error(
                declaration.span,
                "core `Ptr` must be `struct Ptr<T : value>(val _rawPointer: UInt)`".to_string(),
            );
        }
    }

    pub(super) fn validate_fun_ptr_struct(&mut self, id: StructId) {
        self.current_file = self.struct_files[&id];
        let declaration = &self.structs[id];
        let valid = declaration.type_params.len() == 1
            && declaration.type_params[0].kind() == hir::TypeParamKind::Any
            && matches!(declaration.semantic_fields(), [field] if field.name == "_rawPointer" && field.ty == self.uint)
            && declaration.interfaces.is_empty()
            && declaration.methods.is_empty()
            && !declaration.attributes.interior_mutable
            && declaration.attributes.c_layout.is_none();
        if !valid {
            self.error(
                declaration.span,
                "core `FunPtr` must be `struct FunPtr<F>(val _rawPointer: UInt)`".to_string(),
            );
        }
    }

    pub(super) fn validate_ptr_method_intrinsic(
        &mut self,
        id: FunctionId,
        ptr: StructId,
        kind: hir::PointerIntrinsic,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let owner_matches = self.function_owner.get(&id) == Some(&Owner::Struct(ptr));
        let base = owner_matches
            && !sig.is_suspend
            && sig.owner_type_param_count == 1
            && sig.type_params.first().is_some_and(|param| {
                param.kind() == hir::TypeParamKind::Value && param.name == "T"
            })
            && function.attributes.safety == hir::Safety::Unsafe
            && function.attributes.gc_effect == hir::GcEffect::NoGc;
        let valid = base
            && match kind {
                hir::PointerIntrinsic::ToUInt => {
                    function.name.ends_with(".toUInt")
                        && sig.type_params.len() == 1
                        && sig.params.is_empty()
                        && sig.return_ty == self.uint
                }
                hir::PointerIntrinsic::Cast => {
                    function.name.ends_with(".cast")
                        && sig.type_params.len() == 2
                        && sig.type_params[1].kind() == hir::TypeParamKind::Value
                        && sig.params.is_empty()
                        && self.is_ptr_param(sig.return_ty, 1)
                }
                hir::PointerIntrinsic::Load => {
                    function.name.ends_with(".load")
                        && sig.type_params.len() == 1
                        && sig.params.is_empty()
                        && self.is_type_param(sig.return_ty, 0)
                }
                hir::PointerIntrinsic::LoadOffset => {
                    function.name.ends_with(".load")
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [param] if param.ty == self.int)
                        && self.is_type_param(sig.return_ty, 0)
                }
                hir::PointerIntrinsic::Store => {
                    function.name.ends_with(".store")
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [param] if self.is_type_param(param.ty, 0))
                        && sig.return_ty == self.unit
                }
                hir::PointerIntrinsic::StoreOffset => {
                    function.name.ends_with(".store")
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [offset, value] if offset.ty == self.int && self.is_type_param(value.ty, 0))
                        && sig.return_ty == self.unit
                }
                hir::PointerIntrinsic::Plus | hir::PointerIntrinsic::Minus => {
                    function
                        .name
                        .ends_with(if kind == hir::PointerIntrinsic::Plus {
                            ".plus"
                        } else {
                            ".minus"
                        })
                        && sig.type_params.len() == 1
                        && matches!(sig.params.as_slice(), [param] if param.ty == self.int)
                        && self.is_ptr_param(sig.return_ty, 0)
                }
                _ => false,
            };
        if !valid {
            let intrinsic = match &function.kind {
                FunctionKind::Intrinsic(intrinsic) => intrinsic.kind.name(),
                FunctionKind::User(_) => "pointer",
                FunctionKind::DerivedEquality => "derived equality",
                FunctionKind::Extern(_) => "extern",
            };
            self.error(
                function.span,
                format!("malformed core pointer intrinsic `{intrinsic}`"),
            );
        }
    }

    pub(super) fn validate_pointer_top_level_intrinsic(
        &mut self,
        id: FunctionId,
        kind: hir::PointerIntrinsic,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let one_value_param = sig.type_params.len() == 1
            && sig.type_params[0].kind() == hir::TypeParamKind::Value
            && sig.owner_type_param_count == 0;
        let valid = function.method.is_none()
            && !sig.is_suspend
            && one_value_param
            && match kind {
                hir::PointerIntrinsic::AddressOf => {
                    function.name == "addressOf"
                        && function.attributes.safety == hir::Safety::Unsafe
                        && function.attributes.gc_effect == hir::GcEffect::Managed
                        && matches!(sig.params.as_slice(), [param] if self.is_type_param(param.ty, 0))
                        && self.is_ptr_param(sig.return_ty, 0)
                }
                hir::PointerIntrinsic::SizeOf | hir::PointerIntrinsic::AlignOf => {
                    function.name
                        == if kind == hir::PointerIntrinsic::SizeOf {
                            "sizeOf"
                        } else {
                            "alignOf"
                        }
                        && function.attributes.safety == hir::Safety::Safe
                        && function.attributes.gc_effect == hir::GcEffect::NoGc
                        && sig.params.is_empty()
                        && sig.return_ty == self.uint
                }
                _ => false,
            };
        if !valid {
            let intrinsic = match &function.kind {
                FunctionKind::Intrinsic(intrinsic) => intrinsic.kind.name(),
                FunctionKind::User(_) => "pointer",
                FunctionKind::DerivedEquality => "derived equality",
                FunctionKind::Extern(_) => "extern",
            };
            self.error(
                function.span,
                format!("malformed core pointer intrinsic `{intrinsic}`"),
            );
        }
    }

    pub(super) fn validate_pointer_type_uses(&mut self) {
        let uses = self.pointer_type_uses.clone();
        for (ty, file, span) in uses {
            let Type::Ptr(pointee) = self.types[ty] else {
                continue;
            };
            if self.type_contains_param(pointee) {
                continue;
            }
            if !self.is_gc_free(pointee) {
                self.current_file = file;
                self.error(
                    span,
                    format!(
                        "`Ptr` pointee must be GC-free, found {}",
                        self.type_name(pointee)
                    ),
                );
            }
        }
    }

    pub(super) fn require_core_interface(
        &mut self,
        name: &str,
        files: &[ast::SourceFile],
    ) -> Option<InterfaceId> {
        let candidate = self
            .interfaces_by_name
            .get(name)
            .map(|(id, _)| *id)
            .filter(|id| self.interface_files[id] < self.user_file_index);
        if candidate.is_none() {
            self.current_file = 0;
            self.error(
                files[0].span,
                format!("scoop.core must define interface `{name}`"),
            );
        }
        candidate
    }

    pub(super) fn validate_continuation_contract(&mut self, id: InterfaceId) {
        self.current_file = self.interface_files[&id];
        let interface = &self.interfaces[id];
        let throwable = self.throwable.map(|(_, ty)| ty);
        let valid_type_param = matches!(
            interface.type_params.as_slice(),
            [hir::TypeParamDecl {
                variance: hir::Variance::In,
                ..
            }]
        );
        let valid_methods = match self.interface_methods[&id].as_slice() {
            [resume, resume_exception] => {
                let resume = &self.functions[*resume];
                let resume_exception = &self.functions[*resume_exception];
                resume.name.rsplit('.').next() == Some("resume")
                    && !resume.is_suspend
                    && resume.params.len() == 2
                    && self.is_type_param(resume.params[1].ty, 0)
                    && resume.return_ty == self.unit
                    && resume_exception.name.rsplit('.').next() == Some("resumeWithException")
                    && !resume_exception.is_suspend
                    && resume_exception.params.len() == 2
                    && throwable.is_some_and(|ty| resume_exception.params[1].ty == ty)
                    && resume_exception.return_ty == self.unit
            }
            _ => false,
        };
        if !valid_type_param || !valid_methods {
            self.error(
                interface.span,
                "interface `Continuation<in T>` in scoop.core must declare exactly `fun resume(value: T)` followed by `fun resumeWithException(exception: Throwable)`"
                    .to_string(),
            );
        }
    }

    pub(super) fn validate_suspend_task_contract(&mut self, id: InterfaceId) {
        self.current_file = self.interface_files[&id];
        let interface = &self.interfaces[id];
        let valid_type_param = matches!(
            interface.type_params.as_slice(),
            [hir::TypeParamDecl {
                variance: hir::Variance::Out,
                ..
            }]
        );
        let valid_method = match self.interface_methods[&id].as_slice() {
            [run] => {
                let run = &self.functions[*run];
                run.name.rsplit('.').next() == Some("run")
                    && run.is_suspend
                    && run.params.len() == 1
                    && self.is_type_param(run.return_ty, 0)
            }
            _ => false,
        };
        if !valid_type_param || !valid_method {
            self.error(
                interface.span,
                "interface `SuspendTask<out T>` in scoop.core must declare exactly `suspend fun run(): T`"
                    .to_string(),
            );
        }
    }

    pub(super) fn validate_suspend_registration_contract(
        &mut self,
        id: InterfaceId,
        continuation: InterfaceId,
    ) {
        self.current_file = self.interface_files[&id];
        let interface = &self.interfaces[id];
        let valid_type_param = matches!(
            interface.type_params.as_slice(),
            [hir::TypeParamDecl {
                variance: hir::Variance::Out,
                ..
            }]
        );
        let valid_method = match self.interface_methods[&id].as_slice() {
            [register] => {
                let register = &self.functions[*register];
                register.name.rsplit('.').next() == Some("register")
                    && !register.is_suspend
                    && register.params.len() == 2
                    && self.is_interface_param(register.params[1].ty, continuation, 0)
                    && register.return_ty == self.unit
            }
            _ => false,
        };
        if !valid_type_param || !valid_method {
            self.error(
                interface.span,
                "interface `SuspendRegistration<out T>` in scoop.core must declare exactly `fun register(continuation: Continuation<T>)`"
                    .to_string(),
            );
        }
    }

    pub(super) fn validate_coroutine_start(
        &mut self,
        id: FunctionId,
        continuation: InterfaceId,
        suspend_task: InterfaceId,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let valid = function.name == "startCoroutine"
            && !sig.is_suspend
            && sig.type_params.len() == 1
            && sig.params.len() == 2
            && self.is_interface_param(sig.params[0].ty, suspend_task, 0)
            && self.is_interface_param(sig.params[1].ty, continuation, 0)
            && sig.return_ty == self.unit;
        if !valid {
            self.error(
                function.span,
                "intrinsic `coroutine_start` must have signature `fun <T> startCoroutine(task: SuspendTask<T>, completion: Continuation<T>): Unit`"
                    .to_string(),
            );
        }
    }

    pub(super) fn validate_coroutine_suspend(
        &mut self,
        id: FunctionId,
        suspend_registration: InterfaceId,
    ) {
        self.current_file = self.function_files[&id];
        let function = &self.functions[id];
        let sig = &self.signatures[&id];
        let valid = function.name == "suspendCoroutine"
            && sig.is_suspend
            && sig.type_params.len() == 1
            && sig.params.len() == 1
            && self.is_interface_param(sig.params[0].ty, suspend_registration, 0)
            && self.is_type_param(sig.return_ty, 0);
        if !valid {
            self.error(
                function.span,
                "intrinsic `coroutine_suspend` must have signature `suspend fun <T> suspendCoroutine(registration: SuspendRegistration<T>): T`"
                    .to_string(),
            );
        }
    }

    pub(super) fn require_intrinsic(
        &mut self,
        kind: hir::IntrinsicFunctionKind,
        files: &[ast::SourceFile],
    ) -> Option<FunctionId> {
        if let Some(&(function, _provider)) = self.intrinsic_functions.get(&kind) {
            return Some(function);
        }
        self.current_file = 0;
        self.error(
            files[0].span,
            format!(
                "scoop.core must define exactly one `{}` intrinsic",
                kind.name()
            ),
        );
        None
    }

    pub(super) fn is_type_param(&self, ty: TypeId, index: u32) -> bool {
        matches!(
            self.types[ty],
            Type::Param(param) if param.into_raw() == index
        )
    }

    pub(super) fn is_ptr_param(&self, ty: TypeId, index: u32) -> bool {
        matches!(
            self.types[ty],
            Type::Ptr(pointee) if self.is_type_param(pointee, index)
        )
    }

    pub(super) fn is_interface_param(
        &self,
        ty: TypeId,
        interface: InterfaceId,
        index: u32,
    ) -> bool {
        let Type::Interface(application) = self.types[ty] else {
            return false;
        };
        let application = &self.interface_applications[application];
        application.template == interface
            && matches!(application.arguments.as_slice(), [arg] if self.is_type_param(*arg, index))
    }

    pub(super) fn class_descends_from(&self, class: ClassId, root: ClassId) -> bool {
        let mut current = Some(class);
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if id == root {
                return true;
            }
            if !visited.insert(id) {
                return false;
            }
            current = self.classes[id].base_class.as_ref().map(|(base, _)| {
                let Type::Class(application) = self.types[*base] else {
                    unreachable!("resolved class bases are class applications")
                };
                self.class_applications[application].template
            });
        }
        false
    }

    /// `scoop.core` must define exactly one enum named `Option` with
    /// exactly one type parameter (hir docs, spec 7.2). A second
    /// `Option` was already rejected as a duplicate enum in pass 1, so
    /// at most one candidate reaches here.
    pub(super) fn validate_option_enum(&mut self, files: &[ast::SourceFile]) {
        let Some(&(id, file_index, span, type_param_count)) = self.option_candidates.first() else {
            // Attribute to the first file: with a core library present
            // that is a core file; without one it is the user file.
            self.current_file = 0;
            self.error(
                files[0].span,
                "scoop.core must define an enum `Option<T>`".to_string(),
            );
            return;
        };
        if type_param_count != 1 {
            self.current_file = file_index;
            self.error(
                span,
                format!(
                    "enum `Option` in scoop.core must have exactly one type parameter, found {type_param_count}"
                ),
            );
            return;
        }
        self.option_enum = Some(id);
    }

    /// `scoop.core` must define a class named `Throwable` (spec 11.7,
    /// milestone8 DESIGN.md 2.1): the root of the exception hierarchy
    /// that `throw` operands and catch parameter types are checked
    /// against. A `Throwable` declared as another type kind, or only
    /// in the user file, is a core configuration error attributed to
    /// the first file (with a core library present that is a core
    /// file). A second core `Throwable` was already rejected as a
    /// duplicate class in pass 1, so at most one candidate reaches
    /// here.
    pub(super) fn validate_throwable(&mut self, files: &[ast::SourceFile]) {
        let Some(&candidate) = self.throwable_candidates.first() else {
            self.current_file = 0;
            self.error(
                files[0].span,
                "scoop.core must define a class `Throwable`".to_string(),
            );
            return;
        };
        self.throwable = Some(candidate);
    }

    /// The `Throwable` reference type of `scoop.core`, when validated.
    /// `throw` / catch lowering skips its subtype check when this is
    /// `None` (the misconfigured core was already diagnosed, so the
    /// module is rejected anyway).
    pub(crate) fn throwable_ty(&self) -> Option<TypeId> {
        self.throwable.map(|(_, ty)| ty)
    }
}
