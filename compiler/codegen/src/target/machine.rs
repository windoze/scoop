//! Machine options that preserve the managed stack-map contract.

use std::sync::Once;

pub(super) fn initialize_options() {
    static INITIALIZE: Once = Once::new();
    INITIALIZE.call_once(|| {
        // X86 call-frame optimization can replace the reserved argument area
        // with pushes. Stack maps then report the fixed frame size while the
        // call-site SP includes extra arguments, violating the runtime profile.
        let arguments = [
            c"scoop-codegen".as_ptr(),
            c"--no-x86-call-frame-opt".as_ptr(),
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
