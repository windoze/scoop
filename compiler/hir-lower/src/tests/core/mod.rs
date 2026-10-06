mod arrays;
mod capabilities;
mod coroutines;
mod exceptions;
mod ffi;
mod floating;
mod gc;
mod intrinsic_types;
mod module;
mod sysroot;
mod testing;

pub(crate) use module::{complete_core_file, core_file, make_core_public};
pub(super) use sysroot::lower_with_sysroot;
pub(crate) use testing::*;

use capabilities::capability_interfaces;
use coroutines::coroutine_core_declarations;
use exceptions::exception_core_declarations;
use ffi::ffi_core_declarations;
use gc::gc_api_declarations;
use intrinsic_types::intrinsic_type_declarations;
