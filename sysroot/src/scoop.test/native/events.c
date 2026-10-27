#ifdef _WIN32
#define _CRT_SECURE_NO_WARNINGS
#else
#define _POSIX_C_SOURCE 200809L
#endif
#include "test_io.h"

#include <errno.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <wchar.h>
static wchar_t *run_directory;
#else
static char *run_directory;
#endif
static FILE *events;
static int initialization;
static uint64_t sequence;

FILE *scoop_test_open_output(const char *name) {
#ifdef _WIN32
    size_t directory_length = wcslen(run_directory);
    size_t name_length = strlen(name);
    if (name_length > SIZE_MAX / sizeof(wchar_t) - 2 ||
        directory_length > SIZE_MAX / sizeof(wchar_t) - name_length - 2) {
        errno = ENAMETOOLONG;
        return NULL;
    }
    wchar_t *path = malloc((directory_length + name_length + 2) * sizeof(wchar_t));
    if (!path)
        return NULL;
    memcpy(path, run_directory, directory_length * sizeof(wchar_t));
    path[directory_length] = L'/';
    // Output names are the internal ASCII event/capture filenames.
    for (size_t index = 0; index <= name_length; ++index)
        path[directory_length + 1 + index] = (unsigned char)name[index];
    FILE *file = _wfopen(path, L"wb");
#else
    size_t directory_length = strlen(run_directory);
    size_t name_length = strlen(name);
    if (directory_length > SIZE_MAX - name_length - 2) {
        errno = ENAMETOOLONG;
        return NULL;
    }
    char *path = malloc(directory_length + name_length + 2);
    if (!path)
        return NULL;
    memcpy(path, run_directory, directory_length);
    path[directory_length] = '/';
    memcpy(path + directory_length + 1, name, name_length + 1);
    FILE *file = fopen(path, "wb");
#endif
    free(path);
    return file;
}

static int initialize(void) {
    if (initialization)
        return initialization == 2 ? 0 : initialization;
#ifdef _WIN32
    const wchar_t *directory = _wgetenv(L"SCOOP_TEST_RUN_DIR");
#else
    const char *directory = getenv("SCOOP_TEST_RUN_DIR");
#endif
    if (!directory) {
        initialization = 2;
        return 0;
    }
    initialization = -1;
#ifdef _WIN32
    run_directory = _wcsdup(directory);
#else
    run_directory = strdup(directory);
#endif
    if (!run_directory)
        return -1;
    events = scoop_test_open_output("events.jsonl");
    initialization = events ? 1 : -1;
    return initialization;
}

static int json_string(const uint8_t *bytes, int64_t length) {
    if (length < 0 || fputc('"', events) == EOF)
        return -1;
    for (int64_t index = 0; index < length; ++index) {
        uint8_t byte = bytes[index];
        if (byte == '"' || byte == '\\') {
            if (fputc('\\', events) == EOF || fputc(byte, events) == EOF)
                return -1;
        } else if (byte < 32) {
            if (fprintf(events, "\\u%04x", (unsigned)byte) < 0)
                return -1;
        } else if (fputc(byte, events) == EOF) {
            return -1;
        }
    }
    return fputc('"', events) == EOF ? -1 : 0;
}

int32_t scoop_test_begin(const uint8_t *key, int64_t length) {
    int enabled = initialize();
    if (enabled <= 0)
        return enabled;
    if (fputs("{\"event\":\"begin\",\"case\":", events) == EOF || json_string(key, length) < 0 ||
        fputs("}\n", events) == EOF || fflush(events))
        return -1;
    if (scoop_test_capture_begin(sequence++) < 0)
        return -1;
    return 1;
}

int64_t scoop_test_end(void) { return scoop_test_capture_end(); }

int32_t scoop_test_event(const uint8_t *bytes, int64_t length) {
    if (initialize() != 1 || length < 0)
        return -1;
    if (fwrite(bytes, 1, (size_t)length, events) != (size_t)length || fputc('\n', events) == EOF ||
        fflush(events))
        return -1;
    return 0;
}

int32_t scoop_test_complete(void) {
    int enabled = initialize();
    if (enabled <= 0)
        return enabled;
    const char record[] = "{\"event\":\"complete\"}\n";
    return fputs(record, events) == EOF || fflush(events) ? -1 : 0;
}
