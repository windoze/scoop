#define WIN32_LEAN_AND_MEAN 1
#include <windows.h>

#include <dbghelp.h>
#include <shellapi.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>

static int fail(const char *operation) {
    fprintf(stderr, "%s failed: %lu\n", operation, GetLastError());
    return 1;
}

static int inspect(const wchar_t *image, const wchar_t *source, DWORD wanted) {
    DWORD length = GetFullPathNameW(image, 0, NULL, NULL);
    if (!length)
        return fail("image path");
    wchar_t *directory = malloc((size_t)length * sizeof(*directory));
    if (!directory)
        return 1;
    wchar_t *file = NULL;
    if (!GetFullPathNameW(image, length, directory, &file) || !file) {
        free(directory);
        return fail("image directory");
    }
    *file = L'\0';
    HANDLE process = GetCurrentProcess();
    SymSetOptions(SYMOPT_LOAD_LINES | SYMOPT_FAIL_CRITICAL_ERRORS |
                  SYMOPT_NO_PROMPTS);
    BOOL initialized = SymInitializeW(process, directory, FALSE);
    free(directory);
    if (!initialized)
        return fail("SymInitializeW");
    int result = 1;
    DWORD64 base =
        SymLoadModuleExW(process, NULL, image, L"scoop_fixture", 0, 0, NULL, 0);
    if (!base) {
        fail("SymLoadModuleExW");
        goto done;
    }
    IMAGEHLP_LINEW64 line = {0};
    line.SizeOfStruct = sizeof(line);
    LONG displacement = -1;
    if (!SymGetLineFromNameW64(process, L"scoop_fixture", source, wanted,
                               &displacement, &line)) {
        fail("SymGetLineFromNameW64");
        goto done;
    }
    if (line.LineNumber != wanted || displacement != 0 || line.Address < base) {
        fprintf(stderr, "wrong source line: %lu, displacement %ld\n",
                line.LineNumber, displacement);
        goto done;
    }
    printf("line=%lu\n", line.LineNumber);
    result = 0;
done:
    SymCleanup(process);
    return result;
}

int main(void) {
    int argc = 0;
    wchar_t **argv = CommandLineToArgvW(GetCommandLineW(), &argc);
    if (!argv || argc != 4) {
        LocalFree(argv);
        return 2;
    }
    // DbgHelp matches the recorded filename, including native separators.
    for (wchar_t *part = argv[2]; *part; ++part) {
        if (*part == L'/')
            *part = L'\\';
    }
    wchar_t *end = NULL;
    DWORD wanted = wcstoul(argv[3], &end, 10);
    int result =
        wanted && *end == L'\0' ? inspect(argv[1], argv[2], wanted) : 2;
    LocalFree(argv);
    return result;
}
