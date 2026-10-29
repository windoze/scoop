use inkwell::module::Module;

unsafe extern "C" {
    fn scoop_debug_finalize_values(module: inkwell::llvm_sys::prelude::LLVMModuleRef);
}

pub(crate) fn finalize_values(module: &Module<'_>) {
    // The module and its debug records remain owned by this LLVM context.
    unsafe { scoop_debug_finalize_values(module.as_mut_ptr()) };
}
