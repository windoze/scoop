#define _GNU_SOURCE
#include "internal.h"

#include <link.h>
#include <string.h>
#include <unistd.h>

static size_t align_note(size_t size) { return (size + 3) & ~(size_t)3; }

static void build_id(const struct dl_phdr_info *image, ScoopTraceModule *module) {
    for (ElfW(Half) index = 0; index < image->dlpi_phnum; index++) {
        const ElfW(Phdr) *segment = &image->dlpi_phdr[index];
        if (segment->p_type != PT_NOTE)
            continue;
        const unsigned char *cursor = (const void *)(image->dlpi_addr + segment->p_vaddr);
        size_t remaining = segment->p_memsz;
        while (remaining >= sizeof(ElfW(Nhdr))) {
            ElfW(Nhdr) note;
            memcpy(&note, cursor, sizeof note);
            cursor += sizeof note;
            remaining -= sizeof note;
            size_t name = align_note(note.n_namesz), value = align_note(note.n_descsz);
            if (name > remaining || value > remaining - name)
                break;
            if (note.n_type == NT_GNU_BUILD_ID && note.n_namesz == 4 &&
                memcmp(cursor, "GNU", 4) == 0 && note.n_descsz <= SCOOP_TRACE_MAX_ID) {
                module->id_kind = 2;
                module->id_length = note.n_descsz;
                memcpy(module->id, cursor + name, note.n_descsz);
                return;
            }
            cursor += name + value;
            remaining -= name + value;
        }
    }
}

typedef struct ModuleQuery {
    ScoopTraceFrames *frames;
    ScoopTraceLoadedModule *modules;
    uint32_t count;
} ModuleQuery;

static int visit_image(struct dl_phdr_info *image, size_t size, void *data) {
    (void)size;
    ModuleQuery *query = data;
    uint32_t module = SCOOP_TRACE_NO_MODULE;
    for (ElfW(Half) segment_index = 0; segment_index < image->dlpi_phnum; segment_index++) {
        const ElfW(Phdr) *segment = &image->dlpi_phdr[segment_index];
        if (segment->p_type != PT_LOAD)
            continue;
        uintptr_t low = image->dlpi_addr + segment->p_vaddr;
        for (uint32_t index = 0; index < query->frames->count; index++) {
            ScoopTraceFrame *frame = &query->frames->frames[index];
            if (frame->query_pc < low || frame->query_pc - low >= segment->p_memsz ||
                frame->module != SCOOP_TRACE_NO_MODULE)
                continue;
            if (module == SCOOP_TRACE_NO_MODULE) {
                module = query->count++;
                ScoopTraceLoadedModule *found = &query->modules[module];
                found->identity.load_bias = image->dlpi_addr;
                found->identity.image_base = low;
                build_id(image, &found->identity);
                const char *path = image->dlpi_name;
                char executable[4096];
                if (path == NULL || path[0] == 0) {
                    ssize_t length = readlink("/proc/self/exe", executable, sizeof executable - 1);
                    if (length >= 0 && (size_t)length < sizeof executable - 1) {
                        executable[length] = 0;
                        path = executable;
                    } else {
                        path = "/proc/self/exe";
                    }
                }
                found->path = scoop_trace_copy_path(path);
            }
            frame->module = module;
            frame->link_pc = frame->query_pc - image->dlpi_addr;
        }
    }
    return 0;
}

uint32_t scoop_trace_modules(ScoopTraceFrames *frames, ScoopTraceLoadedModule *modules) {
    ModuleQuery query = {.frames = frames, .modules = modules};
    /* The platform loader keeps its image names and notes valid in callbacks. */
    dl_iterate_phdr(visit_image, &query);
    return query.count;
}
