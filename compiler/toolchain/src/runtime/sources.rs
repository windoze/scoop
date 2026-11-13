pub(super) const COMMON: &[&str] = &[
    "runtime/src/rt.c",
    "runtime/src/io.c",
    "runtime/src/process.c",
    "runtime/src/floating_parse.c",
    "vendor/ryu/ryu/f2s.c",
    "vendor/ryu/ryu/d2s.c",
    "runtime/src/strings.c",
    "runtime/src/startup.c",
    "runtime/src/startup/failure.c",
    "runtime/src/startup/gateway.c",
    "runtime/src/boxing.c",
    "runtime/src/task_context.c",
    "runtime/src/value_shape.c",
    "runtime/src/eh.c",
    "runtime/src/throwable.c",
    "runtime/src/stack_trace/capture.c",
    "runtime/src/stack_trace/render.c",
    "runtime/src/stack_trace/throwable_report.c",
    "runtime/src/eh_personality.c",
    "runtime/src/eh/lsda.c",
    "runtime/src/initialization.c",
    "runtime/src/image/ranges.c",
    "runtime/src/image/checks.c",
    "runtime/src/image/dependencies.c",
    "runtime/src/image/records.c",
    "runtime/src/image/registry.c",
    "runtime/src/image/lookup.c",
    "runtime/src/image/active.c",
    "runtime/src/image/scan_ranges.c",
    "runtime/src/image/types.c",
    "runtime/src/image/context_keys.c",
    "runtime/src/image/type_relations.c",
    "runtime/src/image/storage.c",
    "runtime/src/image/immortals.c",
    "runtime/src/image/units.c",
    "runtime/src/image/allocation_ranges.c",
    "runtime/src/image/stackmaps.c",
    "runtime/src/gc_binding/entries.c",
    "runtime/src/gc_binding/pressure.c",
    "runtime/src/gc/pressure.c",
    "runtime/src/gc/allocation.c",
    "runtime/src/gc/mutator.c",
    "runtime/src/gc_binding/threads.c",
    "runtime/src/gc_binding/values.c",
    "runtime/src/gc/collector.c",
    "runtime/src/gc/mark.c",
    "runtime/src/gc/mark_objects.c",
    "runtime/src/gc/mark_queue.c",
    "runtime/src/gc/mark_pool.c",
    "runtime/src/gc/update.c",
    "runtime/src/gc/time.c",
    "runtime/src/gc_binding/collector_roots.c",
    "runtime/src/gc_binding/scan.c",
    "runtime/src/gc/remembered.c",
    "runtime/src/gc/evacuation_plan.c",
    "runtime/src/gc/old_slots.c",
    "runtime/src/gc/evacuation_sources.c",
    "runtime/src/gc/statistics.c",
    "runtime/src/gc/evacuation.c",
    "runtime/src/gc/reclamation.c",
    "runtime/src/gc/block_release.c",
    "runtime/src/gc/block_sweep.c",
    "runtime/src/gc/memory_return.c",
    "runtime/src/gc/heap.c",
    "runtime/src/gc/regions.c",
    "runtime/src/gc/page_map.c",
    "runtime/src/gc/cards.c",
    "runtime/src/gc/heap_objects.c",
    "runtime/src/gc/handles.c",
    "runtime/src/gc_binding/root_frames.c",
    "runtime/src/gc_binding/roots.c",
    "runtime/src/gc_binding/stackmap.c",
    "runtime/src/gc_binding/stack_roots.c",
    "runtime/src/thread.c",
    "runtime/src/thread/collection.c",
    "runtime/src/thread/debug.c",
    "runtime/src/thread/roots.c",
    "runtime/src/thread/transitions.c",
    "runtime/src/callback.c",
    "runtime/src/platform/common.c",
];

pub(super) const PLATFORM_BUNDLE: &str = "runtime/src/platform/bundle.c";
pub(super) const DARWIN_THREAD_VM_OPS: &str =
    "-DSCOOP_PLATFORM_THREAD_VM_OPS=scoop_darwin_thread_vm_ops";
pub(super) const LINUX_THREAD_VM_OPS: &str =
    "-DSCOOP_PLATFORM_THREAD_VM_OPS=scoop_linux_thread_vm_ops";
pub(super) const WINDOWS_THREAD_VM_OPS: &str =
    "-DSCOOP_PLATFORM_THREAD_VM_OPS=scoop_windows_thread_vm_ops";
pub(super) const MACHO_METADATA_IMAGE_OPS: &str =
    "-DSCOOP_PLATFORM_METADATA_IMAGE_OPS=scoop_macho_metadata_image_ops";
pub(super) const LINUX_ELF_METADATA_IMAGE_OPS: &str =
    "-DSCOOP_PLATFORM_METADATA_IMAGE_OPS=scoop_linux_elf_metadata_image_ops";
pub(super) const PE_METADATA_IMAGE_OPS: &str =
    "-DSCOOP_PLATFORM_METADATA_IMAGE_OPS=scoop_pe_metadata_image_ops";
pub(super) const PE_AMD64_IMAGE_MACHINE: &str = "-DSCOOP_PE_IMAGE_MACHINE=IMAGE_FILE_MACHINE_AMD64";
pub(super) const OUT_OF_LINE_GC_FORWARDING: &str = "-DSCOOP_PLATFORM_INLINE_GC_FORWARDING=0";
pub(super) const INLINE_GC_FORWARDING: &str = "-DSCOOP_PLATFORM_INLINE_GC_FORWARDING=1";
pub(super) const DARWIN_AARCH64_FRAME_OPS: &str =
    "-DSCOOP_PLATFORM_MANAGED_FRAME_OPS=scoop_darwin_aarch64_managed_frame_ops";
pub(super) const SYSV_AMD64_FRAME_OPS: &str =
    "-DSCOOP_PLATFORM_MANAGED_FRAME_OPS=scoop_sysv_amd64_managed_frame_ops";
pub(super) const WINDOWS_AMD64_FRAME_OPS: &str =
    "-DSCOOP_PLATFORM_MANAGED_FRAME_OPS=scoop_windows_amd64_managed_frame_ops";

pub(super) const UNIX_TRACE: &[&str] = &[
    "runtime/src/stack_trace/symbolize.c",
    "vendor/libbacktrace/dwarf.c",
    "vendor/libbacktrace/fileline.c",
    "vendor/libbacktrace/posix.c",
    "vendor/libbacktrace/sort.c",
    "vendor/libbacktrace/state.c",
    "vendor/libbacktrace/mmapio.c",
    "vendor/libbacktrace/mmap.c",
];
pub(super) const WINDOWS_TRACE: &[&str] = &[
    "runtime/src/stack_trace/windows.c",
    "runtime/src/stack_trace/windows_symbols.c",
];

pub(super) const DARWIN_OS: &[&str] = &[
    "runtime/src/platform/os/darwin.c",
    "runtime/src/platform/primitives/posix.c",
    "runtime/src/platform/numeric/posix.c",
];
pub(super) const LINUX_OS: &[&str] = &[
    "runtime/src/platform/os/linux.c",
    "runtime/src/platform/primitives/posix.c",
    "runtime/src/platform/numeric/posix.c",
];
pub(super) const LINUX_ELF_LOADED_IMAGE: &[&str] = &["runtime/src/platform/loader/linux_elf.c"];
pub(super) const LINUX_ELF_TRACE_LOADER: &[&str] = &["runtime/src/stack_trace/linux.c"];
pub(super) const MACHO_IMAGE: &[&str] = &[
    "runtime/src/platform/image/macho.c",
    "runtime/src/stack_trace/macho.c",
    "vendor/libbacktrace/macho.c",
];
pub(super) const ELF_IMAGE: &[&str] = &[
    "runtime/src/platform/image/elf.c",
    "runtime/src/stack_trace/elf.c",
    "vendor/libbacktrace/elf.c",
];
pub(super) const PE_IMAGE: &[&str] = &["runtime/src/platform/image/pe.c"];
pub(super) const WINDOWS_OS: &[&str] = &[
    "runtime/src/platform/os/windows.c",
    "runtime/src/platform/primitives/windows.c",
    "runtime/src/platform/numeric/windows.c",
    "runtime/src/startup/windows_arguments.c",
    "vendor/gdtoa/strtodg.c",
    "vendor/gdtoa/strtof.c",
    "vendor/gdtoa/strtodnrp.c",
    "vendor/gdtoa/misc.c",
    "vendor/gdtoa/dmisc.c",
    "vendor/gdtoa/smisc.c",
    "vendor/gdtoa/gmisc.c",
    "vendor/gdtoa/sum.c",
    "vendor/gdtoa/ulp.c",
    "vendor/gdtoa/gethex.c",
    "vendor/gdtoa/hexnan.c",
    "vendor/gdtoa/hd_init.c",
];
pub(super) const AARCH64: &[&str] = &["runtime/src/platform/arch/aarch64_float_environment.c"];
pub(super) const X86_64: &[&str] = &["runtime/src/platform/arch/x86_64_float_environment.c"];
pub(super) const DARWIN_AARCH64_FRAME: &[&str] = &["runtime/src/platform/frame/darwin_aarch64.c"];
pub(super) const SYSV_AMD64_FRAME: &[&str] = &["runtime/src/platform/frame/sysv_amd64.c"];
pub(super) const WINDOWS_AMD64_VIRTUAL_UNWIND: &[&str] =
    &["runtime/src/platform/frame/windows_amd64_virtual_unwind.c"];

pub(super) const DARWIN_AARCH64_MACHO_MANAGED_ENTRY: &[&str] =
    &["runtime/src/platform/frame/darwin_aarch64_macho.S"];
pub(super) const SYSV_AMD64_ELF_MANAGED_ENTRY: &[&str] =
    &["runtime/src/platform/frame/sysv_amd64_elf.S"];
pub(super) const SYSV_AMD64_MACHO_MANAGED_ENTRY: &[&str] =
    &["runtime/src/platform/frame/sysv_amd64_macho.S"];
pub(super) const WIN64_COFF_MANAGED_ENTRY: &[&str] =
    &["runtime/src/platform/frame/windows_amd64_coff_managed_entry.S"];

pub(super) const DARWIN_AARCH64_MACHO_ABI_SHIM: &[&str] =
    &["runtime/src/platform/abi/darwin_aarch64_macho_floating.S"];
pub(super) const X86_64_TAGGED_FLOATING_ABI_SHIM: &[&str] =
    &["runtime/src/platform/abi/x86_64_tagged_floating.c"];

pub(super) const PROFILING: &[&str] = &[
    "runtime/src/profiling.c",
    "runtime/src/profiling/path.c",
    "runtime/src/profiling/gc.c",
];
