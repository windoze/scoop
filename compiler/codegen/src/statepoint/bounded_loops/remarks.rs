//! Missed vectorization is an ordinary fallback, not child protocol output.

use std::ffi::{CStr, c_void};
use std::io::Write;

use inkwell::context::{AsContextRef, ContextRef};
use inkwell::llvm_sys::core::{
    LLVMContextGetDiagnosticContext, LLVMContextGetDiagnosticHandler,
    LLVMContextSetDiagnosticHandler, LLVMDisposeMessage, LLVMGetDiagInfoDescription,
    LLVMGetDiagInfoSeverity,
};
use inkwell::llvm_sys::prelude::LLVMDiagnosticInfoRef;
use inkwell::llvm_sys::{LLVMDiagnosticHandler, LLVMDiagnosticSeverity};

pub(super) struct Quiet<'ctx> {
    context: ContextRef<'ctx>,
    previous: Box<Previous>,
}

struct Previous {
    handler: LLVMDiagnosticHandler,
    data: *mut c_void,
}

impl<'ctx> Quiet<'ctx> {
    pub(super) fn new(context: ContextRef<'ctx>) -> Self {
        // SAFETY: the per-module context lives throughout this scoped pass;
        // the boxed callback state remains stable until its handler is removed.
        unsafe {
            let raw = context.as_ctx_ref();
            let mut previous = Box::new(Previous {
                handler: LLVMContextGetDiagnosticHandler(raw),
                data: LLVMContextGetDiagnosticContext(raw),
            });
            LLVMContextSetDiagnosticHandler(
                raw,
                Some(diagnostic),
                (&mut *previous as *mut Previous).cast(),
            );
            Self { context, previous }
        }
    }
}

impl Drop for Quiet<'_> {
    fn drop(&mut self) {
        // SAFETY: restore the original handler before releasing callback state.
        unsafe {
            LLVMContextSetDiagnosticHandler(
                self.context.as_ctx_ref(),
                self.previous.handler,
                self.previous.data,
            );
        }
    }
}

extern "C" fn diagnostic(info: LLVMDiagnosticInfoRef, data: *mut c_void) {
    // SAFETY: LLVM supplies a live diagnostic, and Quiet owns this callback's
    // state throughout synchronous pass execution. Preserve non-remark output.
    unsafe {
        let severity = LLVMGetDiagInfoSeverity(info);
        if severity == LLVMDiagnosticSeverity::LLVMDSRemark {
            return;
        }
        let previous = &*data.cast::<Previous>();
        if let Some(handler) = previous.handler {
            handler(info, previous.data);
        } else {
            let message = LLVMGetDiagInfoDescription(info);
            let label = match severity {
                LLVMDiagnosticSeverity::LLVMDSError => "error",
                LLVMDiagnosticSeverity::LLVMDSWarning => "warning",
                _ => "note",
            };
            let _ = writeln!(
                std::io::stderr(),
                "{label}: {}",
                CStr::from_ptr(message).to_string_lossy()
            );
            LLVMDisposeMessage(message);
        }
    }
}
