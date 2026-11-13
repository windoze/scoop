#define _POSIX_C_SOURCE 200809L
#include <inttypes.h>
#include <stdatomic.h>
#include <stdio.h>

#include "callback.h"
#include "eh_internal.h"
#include "floating.h"
#include "gc.h"
#include "profiling.h"
#include "gc_binding/internal.h"
#include "stack_trace.h"
#include "startup/internal.h"
#include "thread.h"

extern const ScoopTypeDescriptor scoop_td_String;

static atomic_flag program_started = ATOMIC_FLAG_INIT;
/* The registry borrows these ranges until program shutdown or process exit. */
static ScoopPlatformMetadataImages loaded;
static int32_t program_argc;
static const char *const *program_argv;

#if (defined(SCOOP_ENABLE_STARTUP_STATS) && SCOOP_ENABLE_STARTUP_STATS) || \
    (defined(SCOOP_ENABLE_PROFILING) && SCOOP_ENABLE_PROFILING)
typedef enum StartupPhase {
    STARTUP_LOADED_IMAGES,
    STARTUP_COLLECT_RECORDS,
    STARTUP_CODE_AND_TYPES,
    STARTUP_STORAGE_AND_UNITS,
    STARTUP_STACKMAPS,
    STARTUP_RUNTIME_INIT,
    STARTUP_EAGER_INIT,
    STARTUP_PHASE_COUNT
} StartupPhase;

typedef struct StartupMetrics {
    uint64_t start;
    uint64_t previous;
    uint64_t phases[STARTUP_PHASE_COUNT];
} StartupMetrics;

static uint64_t startup_time_ns(void) {
    uint64_t value;
    if (!scoop_runtime_platform_monotonic_ns(&value)) {
        scoop_startup_fatal("cannot measure startup time");
    }
    return value;
}

static StartupMetrics startup_metrics_begin(void) {
    StartupMetrics metrics = {0};
    metrics.start = metrics.previous = startup_time_ns();
    return metrics;
}

static void startup_measure(StartupMetrics *metrics, StartupPhase phase) {
    uint64_t now = startup_time_ns();
    metrics->phases[phase] = now - metrics->previous;
    metrics->previous = now;
}

static void startup_write(FILE *output, const StartupMetrics *metrics, const ScoopImageRegistry *registry) {
    fprintf(output,
            "{\"scoop_startup\":1,\"total_ns\":%" PRIu64 ",\"phases_ns\":{"
            "\"loaded_images\":%" PRIu64 ",\"collect_records\":%" PRIu64
            ",\"code_and_types\":%" PRIu64 ",\"storage_and_units\":%" PRIu64
            ",\"stackmaps\":%" PRIu64 ",\"runtime_init\":%" PRIu64 ",\"eager_init\":%" PRIu64
            "},\"counts\":{"
            "\"images\":%zu,\"loaded_images\":%zu,\"types\":%zu,\"callables\":%zu,"
            "\"safepoints\":%zu,\"stackmap_records\":%zu,\"scan_programs\":%zu,"
            "\"static_storages\":%zu,\"static_roots\":%zu,\"immortals\":%zu,"
            "\"initialization_units\":%zu,\"eager_units\":%zu,\"context_keys\":"
            "%" PRIu64 "}}\n",
            metrics->previous - metrics->start, metrics->phases[STARTUP_LOADED_IMAGES],
            metrics->phases[STARTUP_COLLECT_RECORDS], metrics->phases[STARTUP_CODE_AND_TYPES],
            metrics->phases[STARTUP_STORAGE_AND_UNITS], metrics->phases[STARTUP_STACKMAPS],
            metrics->phases[STARTUP_RUNTIME_INIT], metrics->phases[STARTUP_EAGER_INIT],
            registry->image_count, registry->loaded->code_range_count,
            registry->tables[SCOOP_RECORD_TYPE].count,
            registry->tables[SCOOP_RECORD_CALLABLE].count, registry->stackmaps.record_count,
            registry->stackmaps.record_count, scoop_image_scan_program_count(registry),
            registry->tables[SCOOP_RECORD_STORAGE].count, registry->static_root_count,
            registry->tables[SCOOP_RECORD_IMMORTAL].count,
            registry->tables[SCOOP_RECORD_UNIT].count, registry->eager_unit_count,
            registry->context_key_count);
}

static void startup_report(const StartupMetrics *metrics, const ScoopImageRegistry *registry) {
#if defined(SCOOP_ENABLE_STARTUP_STATS) && SCOOP_ENABLE_STARTUP_STATS
    startup_write(stderr, metrics, registry);
#endif
    FILE *output = scoop_profile_metrics_file();
    if (output != NULL) startup_write(output, metrics, registry);
}

#define STARTUP_BEGIN() StartupMetrics metrics = startup_metrics_begin()
#define STARTUP_MEASURE(phase) startup_measure(&metrics, phase)
#define STARTUP_REPORT(registry) startup_report(&metrics, registry)
#else
#define STARTUP_BEGIN() ((void)0)
#define STARTUP_MEASURE(phase) ((void)0)
#define STARTUP_REPORT(registry) ((void)0)
#endif

int32_t scoop_rt_program_argc(void) { return program_argc; }

const char *scoop_rt_program_argv(int32_t index) {
    return index >= 0 && index < program_argc ? program_argv[index] : NULL;
}

static const ScoopPlatformBundle *require_platform(void) {
    const ScoopPlatformBundle *bundle = scoop_platform_bundle();
    if (bundle == NULL || bundle->metadata_images == NULL || bundle->thread_vm == NULL ||
        bundle->managed_frames == NULL || bundle->metadata_images->loaded_images == NULL ||
        bundle->metadata_images->dispose_images == NULL ||
        bundle->thread_vm->stack_bounds == NULL ||
        bundle->thread_vm->reserve_read_write == NULL || bundle->thread_vm->page_size == NULL ||
        bundle->thread_vm->protect_none == NULL ||
        bundle->managed_frames->validate_record == NULL ||
        bundle->managed_frames->frame_from_anchor == NULL ||
        bundle->managed_frames->resolve_root == NULL ||
        bundle->managed_frames->next_frame == NULL) {
        scoop_startup_fatal("selected platform bundle is incomplete");
    }
    return bundle;
}

typedef enum StartupKind { STARTUP_PROGRAM, STARTUP_NATIVE_APP } StartupKind;
typedef struct StartupEntry {
    StartupKind kind;
    union {
        const ScoopRootEntryDescriptorV1 *program;
        const ScoopNativeAppEntryDescriptorV1 *native_app;
    } root;
} StartupEntry;
typedef struct StartupResult {
    ScoopImageRegistry *registry;
    const ScoopInitializationUnitDescriptorV1 *failed_unit;
    bool owned_attachment;
} StartupResult;

static StartupResult initialize(const ScoopImageDescriptorV1 *const *images,
                                uint64_t image_count, StartupEntry entry,
                                uint32_t enable_stack_trace) {
    if (atomic_flag_test_and_set_explicit(&program_started, memory_order_relaxed))
        scoop_startup_fatal("program runtime may only be started once");
    scoop_stack_trace_enabled = enable_stack_trace != 0;
#ifdef _WIN32
    if (entry.kind == STARTUP_NATIVE_APP)
        scoop_trace_use_native_host();
#endif
    scoop_profile_initialize();
    STARTUP_BEGIN();
    const ScoopPlatformBundle *platform = require_platform();
    ScoopPlatformError error = {0};
    if (!platform->metadata_images->loaded_images(&loaded, &error)) {
        scoop_startup_fatal(scoop_platform_error_message(error.code));
    }
    STARTUP_MEASURE(STARTUP_LOADED_IMAGES);
    ScoopImageRegistry *registry =
        entry.kind == STARTUP_PROGRAM
            ? scoop_image_collect(&loaded, images, image_count, entry.root.program)
            : scoop_image_collect_native_app(&loaded, images, image_count,
                                             entry.root.native_app);
    STARTUP_MEASURE(STARTUP_COLLECT_RECORDS);
    scoop_image_validate_code_and_types(registry);
    STARTUP_MEASURE(STARTUP_CODE_AND_TYPES);
    scoop_image_validate_storage_and_units(registry);
    STARTUP_MEASURE(STARTUP_STORAGE_AND_UNITS);
    scoop_image_stackmaps(registry, platform->managed_frames);
    STARTUP_MEASURE(STARTUP_STACKMAPS);
    if (scoop_image_type(registry, &scoop_td_String) == NULL ||
        scoop_td_String.instance_shape.instance_kind != SCOOP_TYPE_INSTANCE_INLINE_BYTES_V1) {
        scoop_startup_fatal("String binding is not a registered InlineBytes type");
    }
    scoop_image_publish(registry);
    scoop_thread_runtime_init();
    scoop_callback_runtime_init();
    scoop_rt_gc_init(registry);
    scoop_profile_runtime_ready();
    bool owned_attachment = false;
    if (entry.kind == STARTUP_PROGRAM)
        scoop_thread_attach_main();
    else
        owned_attachment = scoop_rt_attach_foreign_thread();
    STARTUP_MEASURE(STARTUP_RUNTIME_INIT);
    for (size_t index = 0; index < registry->eager_unit_count; index++) {
        const ScoopInitializationUnitDescriptorV1 *unit = registry->eager_units[index];
        if (scoop_startup_call_gateway(unit->startup_gateway) != 0) {
            return (StartupResult){registry, unit, owned_attachment};
        }
    }
    STARTUP_MEASURE(STARTUP_EAGER_INIT);
    STARTUP_REPORT(registry);
    return (StartupResult){registry, NULL, owned_attachment};
}

int scoop_rt_run_program(const ScoopImageDescriptorV1 *const *images, uint64_t image_count,
                         const ScoopRootEntryDescriptorV1 *root_entry, int32_t argc,
                         const char *const *argv, uint32_t enable_stack_trace) {
    program_argc = argc;
    program_argv = argv;
    StartupResult result =
        initialize(images, image_count,
                   (StartupEntry){.kind = STARTUP_PROGRAM, .root.program = root_entry},
                   enable_stack_trace);
    ScoopImageRegistry *registry = result.registry;
    if (result.failed_unit != NULL)
        scoop_startup_report_failure(result.failed_unit->failure_root, result.failed_unit);
    int32_t exit_code = 0;
    if (scoop_startup_call_root(root_entry->gateway, argc, argv, &exit_code) != 0)
        scoop_startup_report_failure(root_entry->failure_root, NULL);
    uint64_t attached = scoop_thread_prepare_shutdown();
    ScoopCallbackShutdownCounts callbacks = scoop_callback_prepare_shutdown();
    if (attached != 0 || callbacks.active != 0 || callbacks.owned_tokens != 0) {
        fprintf(stderr,
                "scoop: shutdown failed: non-main threads=%" PRIu64
                ", active callbacks=%" PRIu64 ", owned tokens=%" PRIu64 "\n",
                attached, callbacks.active, callbacks.owned_tokens);
        scoop_rt_exit(1);
    }
    scoop_eh_prepare_shutdown();
    scoop_gc_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    scoop_gc_report_metrics();
    scoop_profile_finish("return", exit_code);
    scoop_image_unpublish();
    scoop_image_registry_dispose(registry);
    scoop_platform_bundle()->metadata_images->dispose_images(&loaded);
    return exit_code;
}

/* Only the successful CAS owner writes the report. Terminal acquire loads make
 * its native bytes visible without a lock or any managed object reference. */
enum { APP_UNINITIALIZED, APP_INITIALIZING, APP_READY, APP_FAILED };
static _Atomic(uint32_t) app_state;
static ScoopAppError app_error;

int32_t scoop_rt_init_native_app(const ScoopImageDescriptorV1 *const *images,
                                 uint64_t image_count,
                                 const ScoopNativeAppEntryDescriptorV1 *root_entry,
                                 const void *host_api, void *out_app_api,
                                 ScoopAppError *out_error, uint32_t enable_stack_trace) {
    if (out_error != NULL)
        *out_error = (ScoopAppError){0};
    if (host_api == NULL || out_app_api == NULL)
        return SCOOP_APP_INIT_INVALID_ARGUMENT;
    uint32_t state = APP_UNINITIALIZED;
    if (!atomic_compare_exchange_strong_explicit(&app_state, &state, APP_INITIALIZING,
                                                 memory_order_acq_rel, memory_order_acquire)) {
        switch (state) {
        case APP_INITIALIZING:
            return SCOOP_APP_INIT_IN_PROGRESS;
        case APP_READY:
            return SCOOP_APP_INIT_ALREADY_INITIALIZED;
        case APP_FAILED:
            if (out_error != NULL)
                *out_error = app_error;
            return SCOOP_APP_INIT_FAILED;
        default:
            scoop_startup_fatal("invalid native-app initialization state");
        }
    }
    fenv_t saved_environment;
    uint64_t control = scoop_runtime_float_enter(&saved_environment);
    program_argc = 0;
    program_argv = NULL;
    StartupResult result =
        initialize(images, image_count,
                   (StartupEntry){.kind = STARTUP_NATIVE_APP, .root.native_app = root_entry},
                   enable_stack_trace);
    const ScoopStaticStorageDescriptorV1 *failure = NULL;
    if (result.failed_unit != NULL)
        failure = result.failed_unit->failure_root;
    else if (scoop_startup_call_native_app(root_entry->gateway, host_api, out_app_api) != 0)
        failure = root_entry->failure_root;
    if (failure != NULL)
        app_error = scoop_startup_copy_failure(failure, result.failed_unit);
    if (result.owned_attachment)
        scoop_rt_detach_foreign_thread();
    scoop_runtime_float_leave(&saved_environment, control);
    atomic_store_explicit(&app_state, failure == NULL ? APP_READY : APP_FAILED,
                          memory_order_release);
    if (failure != NULL) {
        if (out_error != NULL)
            *out_error = app_error;
        return SCOOP_APP_INIT_FAILED;
    }
    return SCOOP_APP_INIT_OK;
}
