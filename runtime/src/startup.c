#include <stdatomic.h>

#include "eh_internal.h"
#include "gc/gc_internal.h"
#include "startup/internal.h"
#include "thread.h"

extern const ScoopTypeDescriptor scoop_td_String;

static atomic_flag program_started = ATOMIC_FLAG_INIT;
static int32_t program_argc;
static const char *const *program_argv;

int32_t scoop_rt_program_argc(void) { return program_argc; }

const char *scoop_rt_program_argv(int32_t index) {
    return index >= 0 && index < program_argc ? program_argv[index] : NULL;
}

static const ScoopPlatformBundle *require_platform(void) {
    const ScoopPlatformBundle *bundle = scoop_platform_bundle();
    if (bundle == NULL || bundle->metadata_images == NULL || bundle->thread_vm == NULL ||
        bundle->managed_frames == NULL || bundle->metadata_images->loaded_images == NULL ||
        bundle->metadata_images->dispose_images == NULL ||
        bundle->thread_vm->stack_bounds == NULL || bundle->thread_vm->reserve_read_write == NULL ||
        bundle->thread_vm->page_size == NULL || bundle->thread_vm->protect_none == NULL ||
        bundle->managed_frames->validate_record == NULL ||
        bundle->managed_frames->frame_from_anchor == NULL ||
        bundle->managed_frames->resolve_root == NULL ||
        bundle->managed_frames->next_frame == NULL) {
        scoop_startup_fatal("selected platform bundle is incomplete");
    }
    return bundle;
}

int scoop_rt_run_program(const ScoopImageDescriptorV1 *const *images, uint64_t image_count,
                         const ScoopRootEntryDescriptorV1 *root_entry, int32_t argc,
                         const char *const *argv) {
    if (atomic_flag_test_and_set_explicit(&program_started, memory_order_relaxed)) {
        scoop_startup_fatal("program runtime may only be started once");
    }
    program_argc = argc;
    program_argv = argv;
    const ScoopPlatformBundle *platform = require_platform();
    ScoopPlatformMetadataImages loaded = {0};
    ScoopPlatformError error = {0};
    if (!platform->metadata_images->loaded_images(&loaded, &error)) {
        scoop_startup_fatal(scoop_platform_error_message(error.code));
    }
    ScoopImageRegistry *registry = scoop_image_collect(&loaded, images, image_count, root_entry);
    scoop_image_validate_code_and_types(registry);
    scoop_image_validate_storage_and_units(registry);
    scoop_image_stackmaps(registry, platform->managed_frames);
    if (scoop_image_type(registry, &scoop_td_String) == NULL ||
        scoop_td_String.instance_shape.instance_kind != SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1) {
        scoop_startup_fatal("String binding is not a registered InlineBytes type");
    }
    scoop_image_publish(registry);
    scoop_thread_runtime_init();
    scoop_callback_runtime_init();
    scoop_rt_gc_init(registry);
    scoop_thread_attach_main();
    for (size_t index = 0; index < registry->eager_unit_count; index++) {
        const ScoopInitializationUnitDescriptorV1 *unit = registry->eager_units[index];
        if (scoop_startup_call_gateway(unit->startup_gateway) != 0) {
            scoop_startup_report_failure(unit->failure_root, unit);
        }
    }
    int32_t exit_code = 0;
    if (scoop_startup_call_root(root_entry->gateway, argc, argv, &exit_code) != 0) {
        scoop_startup_report_failure(root_entry->failure_root, NULL);
    }
    scoop_callback_prepare_shutdown();
    scoop_eh_prepare_shutdown();
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    scoop_gc_report_metrics();
    scoop_image_unpublish();
    scoop_image_registry_dispose(registry);
    platform->metadata_images->dispose_images(&loaded);
    return exit_code;
}
