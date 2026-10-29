#include "internal.h"

#include <inttypes.h>

void scoop_trace_render(const void *bytes, FILE *stream) {
    const ScoopTraceHeader *header = bytes;
    const ScoopTraceFrame *frames = (const void *)(header + 1);
    const ScoopTraceModule *modules = (const void *)(frames + header->frame_count);
    for (uint32_t index = 0; index < header->frame_count; index++) {
        const ScoopTraceFrame *frame = &frames[index];
        fprintf(stream, "  at ");
        if (frame->module != SCOOP_TRACE_NO_MODULE) {
            const ScoopTraceModule *module = &modules[frame->module];
            const char *path = (const char *)bytes + module->path_offset;
            fprintf(stream, "%s [", path);
            if (module->id_kind != 0) {
                fputs(module->id_kind == 1 ? "uuid=" : "build-id=", stream);
                for (uint32_t byte = 0; byte < module->id_length; byte++) {
                    fprintf(stream, "%02x", module->id[byte]);
                }
            } else {
                fputs("module-id unavailable", stream);
            }
            fprintf(stream, ", link=0x%" PRIxPTR "]", frame->link_pc);
        } else {
            fputs("<module unavailable>", stream);
        }
        fprintf(stream, " (pc=0x%" PRIxPTR ", query=0x%" PRIxPTR ")\n", frame->raw_pc,
                frame->query_pc);
    }
    if (header->frame_count == 0) {
        fputs("  <no stack frames available>\n", stream);
    }
    if (header->flags & SCOOP_TRACE_TRUNCATED) {
        fputs("  <stack trace truncated>\n", stream);
    }
    if (header->flags & SCOOP_TRACE_PARTIAL_UNWIND) {
        fputs("  <unwind stopped with a partial stack>\n", stream);
    }
    if (header->flags & SCOOP_TRACE_BOUNDARY_MISSING) {
        fputs("  <caller boundary unavailable; runtime frames retained>\n", stream);
    }
}
