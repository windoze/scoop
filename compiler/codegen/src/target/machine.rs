//! LLVM options for Scoop's GC, arithmetic, and machine layout.

use std::sync::Once;

pub(super) fn initialize_options() {
    static INITIALIZE: Once = Once::new();
    INITIALIZE.call_once(|| {
        // X86 call-frame optimization can replace the reserved argument area
        // with pushes. Stack maps then report the fixed frame size while the
        // call-site SP includes extra arguments, violating the runtime profile.
        // Both the post-register-allocation tail-duplication pass and block
        // placement can copy a STATEPOINT after the site plan is finalized.
        // LLVM 22.1 does not preserve noduplicate on that machine instruction,
        // so keep the shared tail and its single return PC in both passes.
        // A vectorization hint must not implicitly authorize FP reassociation.
        // LLVM otherwise reorders reductions even without fast-math flags.
        // Prefer 32-byte innermost X86 loop entries, keeping small kernels in
        // one fetch block even when preceding code grows. Pad before the loop.
        let arguments = [
            c"scoop-codegen".as_ptr(),
            c"--no-x86-call-frame-opt".as_ptr(),
            c"--disable-tail-duplicate".as_ptr(),
            c"--tail-dup-placement=false".as_ptr(),
            c"--hints-allow-reordering=false".as_ptr(),
            c"--x86-experimental-pref-innermost-loop-alignment=5".as_ptr(),
        ];
        // SAFETY: the constant argument strings and pointer array remain valid
        // for the call. Initialization runs once before any target machine.
        unsafe {
            inkwell::llvm_sys::support::LLVMParseCommandLineOptions(
                arguments.len() as i32,
                arguments.as_ptr(),
                c"".as_ptr(),
            );
        }
    });
}
