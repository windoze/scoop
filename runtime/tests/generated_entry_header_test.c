#include "generated_entries.h"

void generated_entry_header_probe(void) {
    ScoopCallerRootEntry entry = {0};
    ScoopCallerRootFrame caller = {0};
    ScoopCompilerRootFrame compiler = {0};
    ScoopThreadTransition transition = {0};
    ScoopAllocationContext allocation = {0};

    (void)entry;
    (void)caller;
    (void)compiler;
    (void)transition;
    (void)allocation;
    (void)scoop_image_managed_globals;
    (void)scoop_image_managed_global_count;
    (void)scoop_image_immortal_objects;
    (void)scoop_image_immortal_object_count;
    (void)scoop_rt_allocation_context;
    (void)scoop_runtime_finish_tlab_alloc;
    (void)scoop_runtime_alloc_slow;
    (void)scoop_rt_safepoint;
    (void)scoop_rt_string_concat;
    (void)scoop_rt_array_clone;
    (void)scoop_rt_box;
    (void)scoop_rt_gc_collect;
    (void)scoop_rt_materialize_exception;
    (void)scoop_rt_push_caller_roots;
    (void)scoop_rt_pop_caller_roots;
    (void)scoop_rt_push_compiler_roots;
    (void)scoop_rt_pop_compiler_roots;
    (void)scoop_rt_pop_top_compiler_roots;
    (void)scoop_rt_enter_native_safe;
    (void)scoop_rt_leave_native_safe;
    (void)scoop_rt_enter_native_borrowed;
    (void)scoop_rt_leave_native_borrowed;
    (void)scoop_gc_card_table;
    (void)scoop_rt_gc_init;
    (void)scoop_rt_gc_add_root;
    (void)scoop_rt_gc_add_root_object;
    (void)scoop_rt_gc_remove_root_object;
    (void)scoop_rt_init_eh;
    (void)scoop_eh_personality;
    (void)scoop_main;
}
