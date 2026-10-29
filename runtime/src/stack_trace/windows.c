#include "windows.h"

#include <stdlib.h>
#include <string.h>

static INIT_ONCE loader_once = INIT_ONCE_STATIC_INIT;
static uint64_t loader_generation = 1;
static void *loader_cookie;

static void NTAPI loader_changed(ULONG reason, const void *data, void *context) {
    (void)data;
    (void)context;
    /* Loader notifications run under the loader lock. No external calls here. */
    /* Adding a module cannot reuse another module's still-loaded addresses. */
    if (reason == 2) /* LDR_DLL_NOTIFICATION_REASON_UNLOADED */
        __atomic_fetch_add(&loader_generation, 1, __ATOMIC_RELAXED);
}

static BOOL CALLBACK initialize_loader(PINIT_ONCE once, PVOID parameter, PVOID *context) {
    (void)once;
    (void)parameter;
    (void)context;
    typedef LONG(NTAPI * RegisterNotification)(ULONG, void(NTAPI *)(ULONG, const void *, void *),
                                               void *, void **);
    HMODULE ntdll = GetModuleHandleW(L"ntdll.dll");
    RegisterNotification register_notification =
        (RegisterNotification)GetProcAddress(ntdll, "LdrRegisterDllNotification");
    if (register_notification != NULL &&
        register_notification(0, loader_changed, NULL, &loader_cookie) < 0)
        loader_cookie = NULL;
    return TRUE;
}

uint64_t scoop_trace_generation(void) {
    InitOnceExecuteOnce(&loader_once, initialize_loader, NULL, NULL);
    return loader_cookie == NULL ? 0 : __atomic_load_n(&loader_generation, __ATOMIC_RELAXED);
}

char *scoop_trace_utf8(const WCHAR *text) {
    int bytes = WideCharToMultiByte(CP_UTF8, 0, text, -1, NULL, 0, NULL, NULL);
    if (bytes == 0)
        return scoop_trace_copy_path(NULL);
    char *copy = scoop_trace_allocate((size_t)bytes);
    if (WideCharToMultiByte(CP_UTF8, 0, text, -1, copy, bytes, NULL, NULL) == 0) {
        free(copy);
        return scoop_trace_copy_path(NULL);
    }
    return copy;
}

WCHAR *scoop_trace_module_path(HMODULE module) {
    DWORD capacity = MAX_PATH;
    for (;;) {
        WCHAR *path = scoop_trace_allocate((size_t)capacity * sizeof *path);
        DWORD length = GetModuleFileNameW(module, path, capacity);
        if (length > 0 && length < capacity) {
            return path;
        }
        free(path);
        if (length == 0 || capacity > UINT32_MAX / 2)
            return NULL;
        capacity *= 2;
    }
}

static void module_identity(HMODULE handle, ScoopTraceModule *module) {
    const unsigned char *base = (const void *)handle;
    const IMAGE_DOS_HEADER *dos = (const void *)base;
    const IMAGE_NT_HEADERS64 *pe = (const void *)(base + dos->e_lfanew);
    module->image_base = (uintptr_t)base;
    module->load_bias = (intptr_t)((uintptr_t)base - pe->OptionalHeader.ImageBase);
    const IMAGE_DATA_DIRECTORY *directory =
        &pe->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_DEBUG];
    DWORD image_size = pe->OptionalHeader.SizeOfImage;
    if (directory->VirtualAddress == 0 || directory->VirtualAddress >= image_size ||
        directory->Size > image_size - directory->VirtualAddress)
        return;
    const IMAGE_DEBUG_DIRECTORY *debug = (const void *)(base + directory->VirtualAddress);
    for (size_t index = 0; index < directory->Size / sizeof *debug; index++) {
        DWORD rva = debug[index].AddressOfRawData;
        if (debug[index].Type != IMAGE_DEBUG_TYPE_CODEVIEW || debug[index].SizeOfData < 24 ||
            rva == 0 || rva >= image_size || 24 > image_size - rva ||
            memcmp(base + rva, "RSDS", 4) != 0)
            continue;
        module->id_kind = 3;
        module->id_length = 20;
        memcpy(module->id, base + rva + 4, 20); /* GUID followed by age. */
        return;
    }
}

uint32_t scoop_trace_modules(ScoopTraceFrames *frames, ScoopTraceLoadedModule *modules,
                             uint64_t *generation) {
    uint64_t before = scoop_trace_generation();
    uint32_t count = 0;
    for (uint32_t index = 0; index < frames->count; index++) {
        ScoopTraceFrame *frame = &frames->frames[index];
        HMODULE handle;
        if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, (LPCWSTR)frame->query_pc,
                                &handle))
            continue;
        uint32_t found = 0;
        while (found < count && modules[found].identity.image_base != (uintptr_t)handle)
            found++;
        if (found == count) {
            module_identity(handle, &modules[found].identity);
            WCHAR *path = scoop_trace_module_path(handle);
            modules[found].path =
                path == NULL ? scoop_trace_copy_path(NULL) : scoop_trace_utf8(path);
            free(path);
            count++;
        }
        frame->module = found;
        frame->link_pc = frame->query_pc - modules[found].identity.load_bias;
        FreeLibrary(handle);
    }
    *generation = before == scoop_trace_generation() ? before : 0;
    return count;
}
