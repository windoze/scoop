#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <fcntl.h>
#include <io.h>
#include <stdint.h>
#include <stdio.h>
#include <windows.h>

#include "startup.h"

static int binary_stream(FILE *stream) {
    int descriptor = _fileno(stream);
    /* GUI processes may have no standard stream attached to their CRT. */
    return descriptor < 0 || _setmode(descriptor, _O_BINARY) != -1;
}

static char *copy_argument(HANDLE heap, const uint16_t *text) {
    size_t units = 0;
    while (text[units] != 0)
        ++units;
    /* Three bytes per code unit also cover a pair's four-byte encoding. */
    if (units > (SIZE_MAX - 1) / 3)
        return NULL;
    uint8_t *bytes = HeapAlloc(heap, 0, units * 3 + 1);
    if (bytes == NULL)
        return NULL;
    size_t written = 0;
    for (size_t index = 0; index < units; ++index) {
        uint32_t point = text[index];
        if (point >= 0xd800 && point <= 0xdbff && index + 1 < units && text[index + 1] >= 0xdc00 &&
            text[index + 1] <= 0xdfff) {
            point = 0x10000 + ((point - 0xd800) << 10) + (text[++index] - 0xdc00);
        }
        /* Unpaired surrogates retain their code point in canonical WTF-8. */
        if (point < 0x80) {
            bytes[written++] = (uint8_t)point;
        } else if (point < 0x800) {
            bytes[written++] = (uint8_t)(0xc0 | (point >> 6));
            bytes[written++] = (uint8_t)(0x80 | (point & 0x3f));
        } else if (point < 0x10000) {
            bytes[written++] = (uint8_t)(0xe0 | (point >> 12));
            bytes[written++] = (uint8_t)(0x80 | ((point >> 6) & 0x3f));
            bytes[written++] = (uint8_t)(0x80 | (point & 0x3f));
        } else {
            bytes[written++] = (uint8_t)(0xf0 | (point >> 18));
            bytes[written++] = (uint8_t)(0x80 | ((point >> 12) & 0x3f));
            bytes[written++] = (uint8_t)(0x80 | ((point >> 6) & 0x3f));
            bytes[written++] = (uint8_t)(0x80 | (point & 0x3f));
        }
    }
    bytes[written] = 0;
    return (char *)bytes;
}

int scoop_rt_run_program_utf16(const ScoopImageDescriptorV1 *const *images, uint64_t image_count,
                               const ScoopRootEntryDescriptorV1 *root_entry, int32_t argc,
                               const uint16_t *const *argv, uint32_t enable_stack_trace) {
    HANDLE heap = GetProcessHeap();
    char **converted = NULL;
    int32_t index = 0;
    if (!binary_stream(stdout) || !binary_stream(stderr)) {
        fputs("scoop: cannot set Windows standard streams to binary mode\n", stderr);
        return 1;
    }
    if (argc < 1 || argv == NULL || argv[argc] != NULL ||
        (uint64_t)argc + 1 > SIZE_MAX / sizeof(char *)) {
        fputs("scoop: invalid Windows startup arguments\n", stderr);
        return 1;
    }
    converted = HeapAlloc(heap, HEAP_ZERO_MEMORY, ((size_t)argc + 1) * sizeof(char *));
    if (converted == NULL) {
        fputs("scoop: cannot allocate Windows startup arguments\n", stderr);
        return 1;
    }
    for (; index < argc; ++index) {
        if (argv[index] == NULL) {
            fprintf(stderr, "scoop: invalid Windows startup argument %d\n", (int)index);
            goto failed;
        }
        converted[index] = copy_argument(heap, argv[index]);
        if (converted[index] == NULL) {
            fprintf(stderr, "scoop: cannot allocate Windows startup argument %d\n", (int)index);
            goto failed;
        }
    }
    /* The coordinator and native shutdown code retain argv for process lifetime.
     */
    return scoop_rt_run_program(images, image_count, root_entry, argc,
                                (const char *const *)converted, enable_stack_trace);

failed:
    for (int32_t item = 0; item <= index; ++item) {
        if (converted[item] != NULL) {
            HeapFree(heap, 0, converted[item]);
        }
    }
    HeapFree(heap, 0, converted);
    return 1;
}
#endif
