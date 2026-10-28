#include "platform/platform.h"
#include "scoop_rt.h"
#include "startup.h"
#include "thread.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern const ScoopImageDescriptorV1 image0 __asm__("scoop$test$image$0");
extern const ScoopImageDescriptorV1 image1 __asm__("scoop$test$image$1");
const ScoopRootEntryDescriptorV1 root __asm__("scoop$test$root") = {0};
const uint64_t actual_string __asm__("scoop$test$String") = 31;
extern const uint64_t scoop_td_String;
__attribute__((section(".llvm_stackmaps"))) const char stackmaps[] = "SCOOPMAP";
extern const ScoopMetadataImageOps scoop_pe_metadata_image_ops;
__declspec(dllimport) int imported_data;
__declspec(dllimport) int imported_function(int);
extern int static_function(int);
static const char *const *saved_arguments;
static const char *saved_raw_arguments[3];
static int calls;

ScoopThreadState *scoop_thread_current(void) { return NULL; }

_Noreturn void scoop_thread_fatal(const char *message) {
    fputs(message, stderr);
    abort();
}

static void check_loaded_image(void) {
    ScoopPlatformMetadataImages images;
    ScoopPlatformError error = {0};
    assert(scoop_pe_metadata_image_ops.loaded_images(&images, &error));
    assert(images.count == 1 && images.range_count > 1);
    const ScoopStackMapImage *image = &images.images[0];
    assert(image->section == (const uint8_t *)stackmaps && image->section_size == sizeof stackmaps);
    assert((uintptr_t)check_loaded_image >= image->text_start &&
           (uintptr_t)check_loaded_image < image->text_end);
    assert(scoop_image_range_contains(&images, &root, sizeof root,
                                      _Alignof(ScoopRootEntryDescriptorV1)));
    assert(scoop_image_range_contains(&images, &image0, sizeof image0,
                                      _Alignof(ScoopImageDescriptorV1)));
    assert(scoop_image_range_contains(&images, &image1, sizeof image1,
                                      _Alignof(ScoopImageDescriptorV1)));
    assert(scoop_image_range_contains(&images, &calls, sizeof calls, _Alignof(int)));
    assert(!scoop_image_range_contains(&images, &images, sizeof images,
                                       _Alignof(ScoopPlatformMetadataImages)));
    assert(
        !scoop_image_range_contains(&images, &imported_data, sizeof imported_data, _Alignof(int)));
    scoop_pe_metadata_image_ops.dispose_images(&images);
    assert(images.count == 0 && images.images == NULL && images.ranges == NULL);
}

static void check_arguments_at_exit(void) {
    if (strcmp(saved_arguments[2], "\xe5\x80\xbc\xf0\x9f\xa7\xaa") != 0) {
        _Exit(91);
    }
    assert(strcmp(saved_raw_arguments[0], "\xed\xa0\x80") == 0);
    assert(strcmp(saved_raw_arguments[1], "\xed\xb0\x80") == 0);
    assert(strcmp(saved_raw_arguments[2],
                  "\x7f\xc2\x80\xdf\xbf\xe0\xa0\x80\xed\x9f\xbf\xed\xa0\x80"
                  "-\xed\xb0\x80\xf0\x90\x80\x80\xf4\x8f\xbf\xbf\xef\xbf\xbf") == 0);
    fputs("atexit\n", stdout);
}

int scoop_rt_run_program(const ScoopImageDescriptorV1 *const *images, uint64_t count,
                         const ScoopRootEntryDescriptorV1 *entry, int32_t argc,
                         const char *const *argv) {
    const uint16_t valid[] = {'t', 0};
    const uint16_t raw_high[] = {0xd800, 0};
    const uint16_t raw_low[] = {0xdc00, 0};
    const uint16_t raw_mixed[] = {0x7f,   0x80,   0x7ff,  0x800,  0xd7ff, 0xd800, '-',
                                  0xdc00, 0xd800, 0xdc00, 0xdbff, 0xdfff, 0xffff, 0};
    const uint16_t *raw[] = {valid, raw_high, NULL};
    ++calls;
    if (calls > 1) {
        assert(calls <= 4 && argc == 2 && argv[2] == NULL && strcmp(argv[0], "t") == 0);
        saved_raw_arguments[calls - 2] = argv[1];
        return 73;
    }
    if (setvbuf(stdout, NULL, _IOFBF, 4096) != 0 || setvbuf(stderr, NULL, _IOFBF, 4096) != 0)
        return 99;
    check_loaded_image();
    if (count != 2 || images[0] != &image0 || images[1] != &image1 || entry != &root ||
        images[0]->cone.identity.bytes[0] != 1 || images[1]->cone.identity.bytes[0] != 2)
        return 92;
    if ((argc != 5 && argc != 6) || argv[argc] != NULL || argv[0] == NULL || argv[1][0] != 0 ||
        strcmp(argv[2], "\xe5\x80\xbc\xf0\x9f\xa7\xaa") != 0 ||
        strcmp(argv[3], "quoted \"value\"") != 0 || strcmp(argv[4], "end\\") != 0)
        return 93;
    if (scoop_td_String != 31 || strcmp(stackmaps, "SCOOPMAP") != 0)
        return 94;
    if (imported_data != 17 || imported_function(25) != 42 || static_function(39) != 42)
        return 95;
    if (scoop_rt_run_program_utf16(images, count, entry, 2, raw) != 73)
        return 96;
    raw[1] = raw_low;
    if (scoop_rt_run_program_utf16(images, count, entry, 2, raw) != 73)
        return 97;
    raw[1] = raw_mixed;
    if (scoop_rt_run_program_utf16(images, count, entry, 2, raw) != 73 || calls != 4)
        return 90;
    saved_arguments = argv;
    if (atexit(check_arguments_at_exit) != 0)
        return 98;
    const uint8_t output[] = {'o', 'u', 't', ':', '\n', '\r', 0x1a, 0, 0xff};
    const uint8_t error[] = {'e', 'r', 'r', ':', '\n', '\r', 0x1a, 0, 0xff};
    scoop_rt_stdout_write(output, sizeof output);
    scoop_rt_stderr_write(error, sizeof error);
    if (argc == 6 && strcmp(argv[5], "--runtime-exit") == 0)
        scoop_rt_exit((int32_t)UINT32_C(0x8ace1357));
    if (argc == 6 && strcmp(argv[5], "--runtime-trap") == 0)
        scoop_rt_trap("test");
    return argc == 6 ? (int32_t)UINT32_C(0x8ace1357) : 0;
}
