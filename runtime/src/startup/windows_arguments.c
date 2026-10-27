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

int scoop_rt_run_program_utf16(const ScoopImageDescriptorV1 *const *images, uint64_t image_count,
                               const ScoopRootEntryDescriptorV1 *root_entry, int32_t argc,
                               const uint16_t *const *argv) {
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
        int size = argv[index] == NULL
                       ? 0
                       : WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS,
                                             (const WCHAR *)argv[index], -1, NULL, 0, NULL, NULL);
        if (size == 0) {
            fprintf(stderr, "scoop: cannot decode UTF-16 startup argument %d\n", (int)index);
            goto failed;
        }
        converted[index] = HeapAlloc(heap, 0, (size_t)size);
        if (converted[index] == NULL ||
            WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, (const WCHAR *)argv[index], -1,
                                converted[index], size, NULL, NULL) != size) {
            fprintf(stderr, "scoop: cannot convert Windows startup argument %d\n", (int)index);
            goto failed;
        }
    }
    /* The coordinator and native shutdown code retain argv for process lifetime. */
    return scoop_rt_run_program(images, image_count, root_entry, argc,
                                (const char *const *)converted);

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
