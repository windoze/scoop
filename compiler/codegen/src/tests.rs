use inkwell::OptimizationLevel;
use inkwell::targets::FileType;
use la_arena::Arena;
use scoop_lir::{
    BasicBlock, CallSite, CallTargets, DirectCallSignature, DispatchKind, DispatchSlot, EnumDef,
    EnumFieldRepr, EnumRepr, EnumVariantRepr, GcEffect, Global, GlobalInit,
    IndirectResultCallSignature, ItableRecord, Layout, LayoutKind, LirMeta, Local, MANAGED_PTR,
    METADATA_PTR, NativeBorrowedResultRoot, PointerKind, RAW_PTR, ResultStorage, Temp,
    TypeDescriptor, TypeDescriptorRef, TypeDescriptorScan, TypedCall, VoidCallSignature,
    WellKnownLayouts, WellKnownTypeDescriptors,
};

use super::*;

mod support;

use support::*;

#[path = "runtime_collector_tests.rs"]
mod runtime_collector_tests;

#[path = "runtime_eh_tests.rs"]
mod runtime_eh_tests;

mod arrays;
mod c_layout;
mod closures;
mod enums;
mod exceptions;
mod initialization;
mod moving_gc;
mod objects;
mod platform;
mod smoke;
mod statepoints;

use enums::enum_module;
use exceptions::exceptions_module;
use objects::heap_module;
