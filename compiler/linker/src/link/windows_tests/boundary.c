#include "startup.h"
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
__declspec(dllimport) int imported_data;
__declspec(dllimport) int imported_function(int);
extern int static_function(int);
static const char *const *saved_arguments;
static int calls;

static void check_arguments_at_exit(void) {
    if (strcmp(saved_arguments[2], "\xe5\x80\xbc\xf0\x9f\xa7\xaa") != 0) {
        _Exit(91);
    }
}

int scoop_rt_run_program(const ScoopImageDescriptorV1 *const *images, uint64_t count,
                         const ScoopRootEntryDescriptorV1 *entry, int32_t argc,
                         const char *const *argv) {
    const uint16_t valid[] = {'t', 0};
    const uint16_t invalid_high[] = {0xd800, 0};
    const uint16_t invalid_low[] = {0xdc00, 0};
    const uint16_t *invalid[] = {valid, invalid_high, NULL};
    if (++calls != 1)
        return 90;
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
    if (scoop_rt_run_program_utf16(images, count, entry, 2, invalid) != 1)
        return 96;
    invalid[1] = invalid_low;
    if (scoop_rt_run_program_utf16(images, count, entry, 2, invalid) != 1 || calls != 1)
        return 97;
    saved_arguments = argv;
    if (atexit(check_arguments_at_exit) != 0)
        return 98;
    return argc == 6 ? (int32_t)UINT32_C(0x8ace1357) : 0;
}
