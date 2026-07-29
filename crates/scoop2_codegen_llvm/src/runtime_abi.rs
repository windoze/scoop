//! 运行时 C 符号声明（`runtime/c`）。
//!
//! 所有 codegen 调用的 C 运行时符号在此声明为 LLVM External 函数。
//! 签名与 `runtime/c/include/scoop_runtime.h` / `scoop_gc.h` 对齐。
//!
//! 地址空间约定：
//! - GC-managed 指针（对象、String、Array 等）= `ptr addrspace(1)`；
//! - native / C-ABI 指针（void*、type_desc、descriptor 等）= `ptr addrspace(0)`。
//!
//! 注意：`scoop_gc_write_barrier(slot_addr, value)` 的 `slot_addr` 是 native 地址，
//! `value` 是 GC 引用。运行时返回写入后的指针。

use inkwell::context::Context;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, FunctionType, IntType, PointerType};
use inkwell::values::FunctionValue;

use crate::context::{gc_address_space, native_address_space, CodegenContext};

/// 运行时符号名表（与 `runtime/c/scoop_runtime_api.h` allowlist 对齐）。
pub mod sym {
    pub const RUNTIME_INIT: &str = "scoop_runtime_init";
    pub const GC_THREAD_ATTACH: &str = "scoop_gc_thread_attach_current";
    pub const GC_THREAD_DETACH: &str = "scoop_gc_thread_detach_current";
    pub const ENTER_NATIVE: &str = "scoop_enter_native";
    pub const LEAVE_NATIVE: &str = "scoop_leave_native";
    pub const GC_SAFEPOINT_POLL: &str = "scoop_gc_safepoint_poll";
    pub const GC_COLLECT: &str = "scoop_gc_collect";
    pub const ALLOC: &str = "scoop_alloc";
    pub const ALLOC_TYPED: &str = "scoop_alloc_typed";
    pub const GC_WRITE_BARRIER: &str = "scoop_gc_write_barrier";
    pub const GC_REGISTER_GLOBAL_ROOT: &str = "scoop_gc_register_global_root";
    pub const PIN: &str = "scoop_pin";
    pub const UNPIN: &str = "scoop_unpin";
    pub const HANDLE_NEW: &str = "scoop_handle_new";
    pub const HANDLE_GET: &str = "scoop_handle_get";
    pub const HANDLE_DROP: &str = "scoop_handle_drop";
    pub const ONCE_BEGIN: &str = "scoop_once_begin";
    pub const ONCE_END: &str = "scoop_once_end";
    pub const PRINT: &str = "scoop_print";
    pub const PRINTLN: &str = "scoop_println";
    pub const PANIC: &str = "scoop_panic";
    pub const RUNTIME_ERROR_FATAL: &str = "scoop_runtime_error_fatal";
    pub const ENTRY_ARGV_ARRAY: &str = "scoop_entry_argv_array";
    pub const STRING_CONCAT: &str = "scoop_string_concat";
    pub const STRING_EQUALS: &str = "scoop_string_equals";
    pub const STRING_BYTE_LENGTH: &str = "scoop_string_byte_length";
    pub const STRING_BYTES: &str = "scoop_string_bytes";
    pub const STRING_FROM_OWNED_BYTES: &str = "scoop_string_from_owned_bytes";
    pub const INT_TO_STRING: &str = "scoop_int_to_string";
    pub const BOOL_TO_STRING: &str = "scoop_bool_to_string";
    pub const CHAR_TO_STRING: &str = "scoop_char_to_string";
    pub const FLOAT32_TO_STRING: &str = "scoop_float32_to_string";
    pub const FLOAT64_TO_STRING: &str = "scoop_float64_to_string";
    pub const FLOAT32_TO_INT: &str = "scoop_float32_to_int";
    pub const FLOAT64_TO_INT: &str = "scoop_float64_to_int";
    pub const MUTABLE_ARRAY_NEW: &str = "scoop_mutable_array_new";
    pub const MUTABLE_ARRAY_LEN: &str = "scoop_mutable_array_len";
    pub const MUTABLE_ARRAY_ELEM_KIND: &str = "scoop_mutable_array_elem_kind";
    pub const MUTABLE_ARRAY_ELEM_SIZE: &str = "scoop_mutable_array_elem_size";
    pub const MUTABLE_ARRAY_PUSH_WORD: &str = "scoop_mutable_array_push_word";
    pub const MUTABLE_ARRAY_PUSH_REF: &str = "scoop_mutable_array_push_ref";
    pub const MUTABLE_ARRAY_PUSH_COMPOSITE: &str = "scoop_mutable_array_push_composite";
    pub const MUTABLE_ARRAY_TO_ARRAY_DATA: &str = "scoop_mutable_array_to_array_data";
    pub const MUTABLE_ARRAY_FREEZE: &str = "scoop_mutable_array_freeze";
    pub const COMPOSITE_TRACE: &str = "scoop_composite_trace";
    pub const COMPOSITE_COPY: &str = "scoop_composite_copy";
    pub const COMPOSITE_DROP: &str = "scoop_composite_drop";
    pub const MEMSET: &str = "llvm.memset.p0.i64";
    pub const MEMCPY: &str = "llvm.memcpy.p0.p0.i64";
    /// TLS 全局：explicit root frame 链表顶。
    pub const EXPLICIT_ROOT_FRAME_TOP: &str = "__scoop_explicit_root_frame_top";
    /// ScoopString 的运行时 type descriptor（extern，由运行时提供）。
    pub const STRING_TYPE_DESC: &str = "__scoop_type_desc_runtime__ScoopString";
}

/// 运行时类型集合（为声明符号提供 LLVM 类型）。
pub struct RuntimeTypes<'ctx> {
    pub ctx: &'ctx Context,
    /// native ptr (addrspace 0)。
    pub void_ptr: PointerType<'ctx>,
    /// gc ptr (addrspace 1)。
    pub gc_ptr: PointerType<'ctx>,
    pub i8: IntType<'ctx>,
    pub i32: IntType<'ctx>,
    pub i64: IntType<'ctx>,
    pub void: inkwell::types::VoidType<'ctx>,
}

impl<'ctx> RuntimeTypes<'ctx> {
    pub fn new(ctx: &'ctx Context) -> Self {
        let void_ptr = ctx.ptr_type(native_address_space());
        let gc_ptr = ctx.ptr_type(gc_address_space());
        RuntimeTypes {
            ctx,
            void_ptr,
            gc_ptr,
            i8: ctx.i8_type(),
            i32: ctx.i32_type(),
            i64: ctx.i64_type(),
            void: ctx.void_type(),
        }
    }

    pub fn md_basic(&self, b: BasicTypeEnum<'ctx>) -> BasicMetadataTypeEnum<'ctx> {
        b.into()
    }
}

/// 声明/获取所有常用 runtime 符号。返回一个 RuntimeFns 集合（按需使用）。
///
/// 调用此函数确保所有符号在 module 中有声明。各 lowering 模块按需通过
/// `ctx.lookup_runtime_fn` 取回（或直接用返回的集合）。
pub struct RuntimeFns<'ctx> {
    pub runtime_init: FunctionValue<'ctx>,
    pub gc_thread_attach: FunctionValue<'ctx>,
    pub gc_thread_detach: FunctionValue<'ctx>,
    pub alloc: FunctionValue<'ctx>,
    pub alloc_typed: FunctionValue<'ctx>,
    pub gc_write_barrier: FunctionValue<'ctx>,
    pub gc_register_global_root: FunctionValue<'ctx>,
    pub gc_safepoint_poll: FunctionValue<'ctx>,
    pub gc_collect: FunctionValue<'ctx>,
    pub println: FunctionValue<'ctx>,
    pub print: FunctionValue<'ctx>,
    pub panic: FunctionValue<'ctx>,
    pub runtime_error_fatal: FunctionValue<'ctx>,
    pub entry_argv_array: FunctionValue<'ctx>,
    pub string_concat: FunctionValue<'ctx>,
    pub string_equals: FunctionValue<'ctx>,
    pub string_byte_length: FunctionValue<'ctx>,
    pub string_bytes: FunctionValue<'ctx>,
    pub int_to_string: FunctionValue<'ctx>,
    pub bool_to_string: FunctionValue<'ctx>,
    pub char_to_string: FunctionValue<'ctx>,
    pub float32_to_string: FunctionValue<'ctx>,
    pub float64_to_string: FunctionValue<'ctx>,
    pub float32_to_int: FunctionValue<'ctx>,
    pub float64_to_int: FunctionValue<'ctx>,
    pub mutable_array_new: FunctionValue<'ctx>,
    pub mutable_array_len: FunctionValue<'ctx>,
    pub mutable_array_push_word: FunctionValue<'ctx>,
    pub mutable_array_push_ref: FunctionValue<'ctx>,
    pub mutable_array_push_composite: FunctionValue<'ctx>,
    pub mutable_array_freeze: FunctionValue<'ctx>,
    pub pin: FunctionValue<'ctx>,
    pub unpin: FunctionValue<'ctx>,
    pub handle_new: FunctionValue<'ctx>,
    pub handle_get: FunctionValue<'ctx>,
    pub handle_drop: FunctionValue<'ctx>,
    pub once_begin: FunctionValue<'ctx>,
    pub once_end: FunctionValue<'ctx>,
}

/// 从 `BasicTypeEnum` 构造函数类型（分发到具体类型的 `fn_type`）。
/// `ctx` 仅用于 scalable vector 不可达分支的兜底。
fn basic_fn_type<'ctx>(
    ret: BasicTypeEnum<'ctx>,
    params: &[BasicMetadataTypeEnum<'ctx>],
    ctx: &'ctx inkwell::context::Context,
) -> FunctionType<'ctx> {
    match ret {
        BasicTypeEnum::IntType(t) => t.fn_type(params, false),
        BasicTypeEnum::FloatType(t) => t.fn_type(params, false),
        BasicTypeEnum::PointerType(t) => t.fn_type(params, false),
        BasicTypeEnum::StructType(t) => t.fn_type(params, false),
        BasicTypeEnum::ArrayType(t) => t.fn_type(params, false),
        BasicTypeEnum::VectorType(t) => t.fn_type(params, false),
        BasicTypeEnum::ScalableVectorType(_) => {
            // runtime 符号不会用到 scalable vector 类型；此分支不可达。保守回退。
            ctx.i8_type().fn_type(params, false)
        }
    }
}

impl<'ctx> CodegenContext<'ctx> {
    /// 声明并缓存全部 runtime 符号，返回 `RuntimeFns`。
    pub fn declare_runtime(&self) -> RuntimeFns<'ctx> {
        let rt = RuntimeTypes::new(self.context);

        // void_fn: 返回 void
        let void_fn = |params: &[BasicMetadataTypeEnum<'ctx>]| -> FunctionType<'ctx> {
            rt.void.fn_type(params, false)
        };
        // ret_fn: 返回 BasicTypeEnum
        let ret_fn = |ret: BasicTypeEnum<'ctx>, params: &[BasicMetadataTypeEnum<'ctx>]| -> FunctionType<'ctx> {
            basic_fn_type(ret, params, self.context)
        };

        let gc_ptr_basic: BasicTypeEnum<'ctx> = rt.gc_ptr.into();
        let void_ptr_basic: BasicTypeEnum<'ctx> = rt.void_ptr.into();
        let gc: BasicMetadataTypeEnum<'ctx> = rt.md_basic(gc_ptr_basic);
        let vp: BasicMetadataTypeEnum<'ctx> = rt.md_basic(void_ptr_basic);
        let i32m: BasicMetadataTypeEnum<'ctx> = rt.md_basic(rt.i32.into());
        let i64m: BasicMetadataTypeEnum<'ctx> = rt.md_basic(rt.i64.into());

        // void scoop_runtime_init()
        let runtime_init =
            self.decl(sym::RUNTIME_INIT, void_fn(&[]));
        // void scoop_gc_thread_attach_current()
        let gc_thread_attach = self.decl(sym::GC_THREAD_ATTACH, void_fn(&[]));
        let gc_thread_detach = self.decl(sym::GC_THREAD_DETACH, void_fn(&[]));

        // void* scoop_alloc(u64)
        let alloc = self.decl(sym::ALLOC, ret_fn(rt.gc_ptr.into(), &[i64m]));
        // void* scoop_alloc_typed(ptr type_desc, u64 size)
        let alloc_typed = self.decl(
            sym::ALLOC_TYPED,
            ret_fn(rt.gc_ptr.into(), &[vp, i64m]),
        );
        // ptr scoop_gc_write_barrier(ptr slot_addr, ptr value)
        //   value 是 GC 引用，但运行时签名用 native void*；slot_addr native。
        let gc_write_barrier = self.decl(
            sym::GC_WRITE_BARRIER,
            ret_fn(rt.gc_ptr.into(), &[vp, vp]),
        );
        // void scoop_gc_register_global_root(ptr base, ptr type_desc)
        let gc_register_global_root =
            self.decl(sym::GC_REGISTER_GLOBAL_ROOT, void_fn(&[vp, vp]));
        // void scoop_gc_safepoint_poll()
        let gc_safepoint_poll = self.decl(sym::GC_SAFEPOINT_POLL, void_fn(&[]));
        // void scoop_gc_collect()
        let gc_collect = self.decl(sym::GC_COLLECT, void_fn(&[]));

        // void scoop_print(ptr string)  / scoop_println
        let print = self.decl(sym::PRINT, void_fn(&[vp]));
        let println = self.decl(sym::PRINTLN, void_fn(&[vp]));
        // void scoop_panic(ptr message)
        let panic = self.decl(sym::PANIC, void_fn(&[vp]));
        // void scoop_runtime_error_fatal(ptr)
        let runtime_error_fatal = self.decl(sym::RUNTIME_ERROR_FATAL, void_fn(&[vp]));
        // ptr scoop_entry_argv_array(i32 argc, ptr argv)
        let entry_argv_array =
            self.decl(sym::ENTRY_ARGV_ARRAY, ret_fn(rt.gc_ptr.into(), &[i32m, vp]));

        // string ops
        let string_concat = self.decl(
            sym::STRING_CONCAT,
            ret_fn(rt.gc_ptr.into(), &[gc, gc]),
        );
        let string_equals = self.decl(sym::STRING_EQUALS, ret_fn(rt.i64.into(), &[gc, gc]));
        let string_byte_length = self.decl(sym::STRING_BYTE_LENGTH, ret_fn(rt.i64.into(), &[gc]));
        let string_bytes = self.decl(
            sym::STRING_BYTES,
            ret_fn(rt.gc_ptr.into(), &[gc]),
        );
        let int_to_string = self.decl(sym::INT_TO_STRING, ret_fn(rt.gc_ptr.into(), &[i64m]));
        let bool_to_string = self.decl(sym::BOOL_TO_STRING, ret_fn(rt.gc_ptr.into(), &[i64m]));
        let char_to_string = self.decl(sym::CHAR_TO_STRING, ret_fn(rt.gc_ptr.into(), &[rt.md_basic(rt.i32.into())]));
        let float32_to_string_ = self.decl(
            sym::FLOAT32_TO_STRING,
            ret_fn(rt.gc_ptr.into(), &[rt.md_basic(self.context.f32_type().into())]),
        );
        let float64_to_string = self.decl(
            sym::FLOAT64_TO_STRING,
            ret_fn(rt.gc_ptr.into(), &[rt.md_basic(self.context.f64_type().into())]),
        );
        let float32_to_int = self.decl(
            sym::FLOAT32_TO_INT,
            ret_fn(rt.i64.into(), &[rt.md_basic(self.context.f32_type().into())]),
        );
        let float64_to_int = self.decl(
            sym::FLOAT64_TO_INT,
            ret_fn(rt.i64.into(), &[rt.md_basic(self.context.f64_type().into())]),
        );

        // arrays
        let mutable_array_new = self.decl(
            sym::MUTABLE_ARRAY_NEW,
            ret_fn(rt.gc_ptr.into(), &[i32m, i64m, i64m, vp, i64m]),
        );
        let mutable_array_len = self.decl(sym::MUTABLE_ARRAY_LEN, ret_fn(rt.i64.into(), &[gc]));
        let mutable_array_push_word = self.decl(
            sym::MUTABLE_ARRAY_PUSH_WORD,
            void_fn(&[gc, i64m]),
        );
        let mutable_array_push_ref = self.decl(
            sym::MUTABLE_ARRAY_PUSH_REF,
            void_fn(&[gc, gc]),
        );
        let mutable_array_push_composite = self.decl(
            sym::MUTABLE_ARRAY_PUSH_COMPOSITE,
            void_fn(&[gc, vp, i64m]),
        );
        let mutable_array_freeze = self.decl(
            sym::MUTABLE_ARRAY_FREEZE,
            ret_fn(rt.gc_ptr.into(), &[gc]),
        );

        // pin/handle
        let pin = self.decl(sym::PIN, ret_fn(rt.i32.into(), &[vp]));
        let unpin = self.decl(sym::UNPIN, ret_fn(rt.i32.into(), &[vp]));
        let handle_new = self.decl(sym::HANDLE_NEW, ret_fn(rt.i64.into(), &[vp]));
        let handle_get = self.decl(sym::HANDLE_GET, ret_fn(rt.gc_ptr.into(), &[i64m]));
        let handle_drop = self.decl(sym::HANDLE_DROP, ret_fn(rt.i32.into(), &[i64m]));

        // once
        let once_begin = self.decl(sym::ONCE_BEGIN, ret_fn(rt.i32.into(), &[vp]));
        let once_end = self.decl(sym::ONCE_END, void_fn(&[vp]));

        // 给关键 alloc/call 函数加 noundef 等不是必须；保持简单。

        RuntimeFns {
            runtime_init,
            gc_thread_attach,
            gc_thread_detach,
            alloc,
            alloc_typed,
            gc_write_barrier,
            gc_register_global_root,
            gc_safepoint_poll,
            gc_collect,
            println,
            print,
            panic,
            runtime_error_fatal,
            entry_argv_array,
            string_concat,
            string_equals,
            string_byte_length,
            string_bytes,
            int_to_string,
            bool_to_string,
            char_to_string,
            float32_to_string: float32_to_string_,
            float64_to_string,
            float32_to_int,
            float64_to_int,
            mutable_array_new,
            mutable_array_len,
            mutable_array_push_word,
            mutable_array_push_ref,
            mutable_array_push_composite,
            mutable_array_freeze,
            pin,
            unpin,
            handle_new,
            handle_get,
            handle_drop,
            once_begin,
            once_end,
        }
    }

    fn decl(&self, name: &'static str, fn_ty: FunctionType<'ctx>) -> FunctionValue<'ctx> {
        if let Some(cached) = self.lookup_runtime_fn(name) {
            return cached;
        }
        let fv = self.declare_external_fn(name, fn_ty);
        self.cache_runtime_fn(name, fv);
        fv
    }
}
