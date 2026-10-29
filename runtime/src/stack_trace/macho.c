#define _DARWIN_C_SOURCE
#include "internal.h"

#include <dlfcn.h>
#include <mach-o/dyld.h>
#include <mach-o/loader.h>
#include <pthread.h>
#include <stdlib.h>
#include <string.h>

/* dyld invokes these callbacks under its loader lock. Copy facts there so a
 * later capture neither borrows unloadable memory nor races image removal. */
typedef struct Image {
    const struct mach_header *header;
    uintptr_t low;
    uintptr_t high;
    ScoopTraceLoadedModule module;
    struct Image *next;
} Image;

static pthread_once_t images_once = PTHREAD_ONCE_INIT;
static pthread_mutex_t images_lock = PTHREAD_MUTEX_INITIALIZER;
static Image *images;

static void add_image(const struct mach_header *raw, intptr_t slide) {
    if (raw->magic != MH_MAGIC_64) {
        return;
    }
    const struct mach_header_64 *header = (const void *)raw;
    Image *image = scoop_trace_allocate(sizeof *image);
    *image = (Image){.header = raw, .low = UINTPTR_MAX};
    image->module.identity.image_base = (uintptr_t)raw;
    image->module.identity.load_bias = slide;
    Dl_info info = {0};
    dladdr(raw, &info);
    image->module.path = scoop_trace_copy_path(info.dli_fname);
    const struct load_command *command = (const void *)(header + 1);
    for (uint32_t index = 0; index < header->ncmds; index++) {
        if (command->cmd == LC_SEGMENT_64) {
            const struct segment_command_64 *segment = (const void *)command;
            if (segment->initprot != 0 && segment->vmsize != 0) {
                uintptr_t low = segment->vmaddr + slide;
                uintptr_t high = low + segment->vmsize;
                if (low < image->low)
                    image->low = low;
                if (high > image->high)
                    image->high = high;
            }
        } else if (command->cmd == LC_UUID) {
            const struct uuid_command *uuid = (const void *)command;
            image->module.identity.id_kind = 1;
            image->module.identity.id_length = sizeof uuid->uuid;
            memcpy(image->module.identity.id, uuid->uuid, sizeof uuid->uuid);
        }
        command = (const void *)((const char *)command + command->cmdsize);
    }
    pthread_mutex_lock(&images_lock);
    image->next = images;
    images = image;
    pthread_mutex_unlock(&images_lock);
}

static void remove_image(const struct mach_header *header, intptr_t slide) {
    (void)slide;
    pthread_mutex_lock(&images_lock);
    for (Image **cursor = &images; *cursor != NULL; cursor = &(*cursor)->next) {
        Image *image = *cursor;
        if (image->header == header) {
            *cursor = image->next;
            free(image->module.path);
            free(image);
            break;
        }
    }
    pthread_mutex_unlock(&images_lock);
}

static void initialize_images(void) {
    _dyld_register_func_for_remove_image(remove_image);
    _dyld_register_func_for_add_image(add_image);
}

uint32_t scoop_trace_modules(ScoopTraceFrames *frames, ScoopTraceLoadedModule *modules) {
    pthread_once(&images_once, initialize_images);
    uint32_t count = 0;
    pthread_mutex_lock(&images_lock);
    for (Image *image = images; image != NULL; image = image->next) {
        uint32_t module = SCOOP_TRACE_NO_MODULE;
        for (uint32_t index = 0; index < frames->count; index++) {
            ScoopTraceFrame *frame = &frames->frames[index];
            if (frame->query_pc < image->low || frame->query_pc >= image->high ||
                frame->module != SCOOP_TRACE_NO_MODULE) {
                continue;
            }
            if (module == SCOOP_TRACE_NO_MODULE) {
                module = count++;
                modules[module] = image->module;
                modules[module].path = scoop_trace_copy_path(image->module.path);
            }
            frame->module = module;
            frame->link_pc = frame->query_pc - image->module.identity.load_bias;
        }
    }
    pthread_mutex_unlock(&images_lock);
    return count;
}
