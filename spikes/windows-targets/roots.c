#include <assert.h>
#include <stdio.h>
#include <string.h>
#include <windows.h>

#include "gc_binding/stackmap.h"

static ScoopStackMapIndex maps;
static int before, after;
extern void *roots_probe(void *object);

static void read_maps(void) {
    const unsigned char *base = (const unsigned char *)GetModuleHandleW(NULL);
    const IMAGE_DOS_HEADER *dos = (const IMAGE_DOS_HEADER *)base;
    const IMAGE_NT_HEADERS64 *pe =
        (const IMAGE_NT_HEADERS64 *)(base + dos->e_lfanew);
    const IMAGE_SECTION_HEADER *sections = IMAGE_FIRST_SECTION(pe);
    ScoopStackMapImage image = {0};
    for (unsigned i = 0; i < pe->FileHeader.NumberOfSections; ++i) {
        const IMAGE_SECTION_HEADER *section = &sections[i];
        if (memcmp(section->Name, ".llvm_st", 8) == 0) {
            image.section = base + section->VirtualAddress;
            image.section_size = section->Misc.VirtualSize;
        } else if (memcmp(section->Name, ".text\0\0\0", 8) == 0) {
            image.text_start = (uintptr_t)base + section->VirtualAddress;
            image.text_end = image.text_start + section->Misc.VirtualSize;
        }
    }
    ScoopStackMapError error;
    if (!scoop_stackmap_build_index(&image, 1, &maps, &error)) {
        fprintf(stderr, "stackmap: %s at %zu\n",
                scoop_stackmap_error_message(error.code), error.section_offset);
        abort();
    }
    assert(maps.record_count == 1);
}

static void update_root(const ScoopStackMapLocation *location,
                        const CONTEXT *frame) {
    assert(location->kind == SCOOP_STACKMAP_INDIRECT);
    assert(location->size == sizeof(void *));
    // LLVM uses DWARF numbers even for COFF stackmaps: RBP=6 and RSP=7.
    assert(location->dwarf_register == 6 || location->dwarf_register == 7);
    uintptr_t base = location->dwarf_register == 6 ? frame->Rbp : frame->Rsp;
    void **slot = (void **)(base + location->offset);
    assert(*slot == &before || *slot == &after);
    *slot = &after;
}

__declspec(noinline) void relocate_probe(unsigned char *large) {
    assert(large[0] == 42);
    large[16383] = 17;
    CONTEXT frame;
    RtlCaptureContext(&frame);
    DWORD64 image_base = 0;
    PRUNTIME_FUNCTION function =
        RtlLookupFunctionEntry(frame.Rip, &image_base, NULL);
    assert(function != NULL);
    void *handler_data = NULL;
    DWORD64 establisher = 0;
    RtlVirtualUnwind(UNW_FLAG_NHANDLER, image_base, frame.Rip, function, &frame,
                     &handler_data, &establisher, NULL);
    const ScoopStackMapRecord *record = scoop_stackmap_lookup(&maps, frame.Rip);
    assert(record != NULL);
    assert(record->safepoint_id == 100);
    assert(record->root_count == 1);
    assert(record->stack_size > 16384);
    update_root(&record->roots[0].base, &frame);
    update_root(&record->roots[0].derived, &frame);
}

int main(void) {
    read_maps();
    assert(roots_probe(&before) == &after);
    scoop_stackmap_dispose_index(&maps);
    puts("Win64 statepoint relocation through SEH frame context passed");
    return 0;
}
