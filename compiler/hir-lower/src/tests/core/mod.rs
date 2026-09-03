mod capabilities;
mod coroutines;
mod exceptions;
mod ffi;
mod gc;
mod intrinsic_types;
mod module;
mod testing;

pub(crate) use module::core_file;
pub(crate) use testing::*;

use capabilities::capability_interfaces;
use coroutines::coroutine_core_declarations;
use exceptions::exception_core_declarations;
use ffi::ffi_core_declarations;
use gc::gc_api_declarations;
use intrinsic_types::intrinsic_type_declarations;
