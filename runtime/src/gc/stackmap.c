#include "stackmap/internal.h"
#include <stdlib.h>

static int compare_record_pc(const void *left, const void *right) {
    const ScoopStackMapRecord *left_record = left;
    const ScoopStackMapRecord *right_record = right;
    if (left_record->return_pc < right_record->return_pc) {
        return -1;
    }
    if (left_record->return_pc > right_record->return_pc) {
        return 1;
    }
    return 0;
}

static int compare_record_id(const void *left, const void *right) {
    const ScoopStackMapRecord *left_record = left;
    const ScoopStackMapRecord *right_record = right;
    if (left_record->safepoint_id < right_record->safepoint_id) {
        return -1;
    }
    if (left_record->safepoint_id > right_record->safepoint_id) {
        return 1;
    }
    return 0;
}

bool scoop_stackmap_read_records(const ScoopStackMapImage *images, size_t image_count,
                                 ScoopStackMapIndex *index, ScoopStackMapError *error) {
    if (index == NULL || error == NULL) {
        return false;
    }
    *index = (ScoopStackMapIndex){0};
    *error = (ScoopStackMapError){.code = SCOOP_STACKMAP_OK};
    if (images == NULL && image_count != 0) {
        error->code = SCOOP_STACKMAP_INVALID_ARGUMENT;
        return false;
    }
    for (size_t image = 0; image < image_count; image++) {
        if (!scoop_stackmap_read_image(&images[image], image, index, error)) {
            scoop_stackmap_dispose_index(index);
            return false;
        }
    }
    return true;
}

bool scoop_stackmap_build_index(const ScoopStackMapImage *images, size_t image_count,
                                ScoopStackMapIndex *index, ScoopStackMapError *error) {
    if (!scoop_stackmap_read_records(images, image_count, index, error)) {
        return false;
    }
    qsort(index->records, index->record_count, sizeof *index->records,
          compare_record_id);
    for (size_t record = 1; record < index->record_count; record++) {
        if (index->records[record - 1].safepoint_id ==
            index->records[record].safepoint_id) {
            error->code = SCOOP_STACKMAP_DUPLICATE_SAFEPOINT_ID;
            scoop_stackmap_dispose_index(index);
            return false;
        }
    }
    if (!scoop_stackmap_finish_index(index, error)) {
        scoop_stackmap_dispose_index(index);
        return false;
    }
    return true;
}

bool scoop_stackmap_finish_index(ScoopStackMapIndex *index, ScoopStackMapError *error) {
    qsort(index->records, index->record_count, sizeof *index->records,
          compare_record_pc);
    for (size_t record = 1; record < index->record_count; record++) {
        if (index->records[record - 1].return_pc == index->records[record].return_pc) {
            error->code = SCOOP_STACKMAP_DUPLICATE_RETURN_PC;
            error->image_index = index->records[record].image_index;
            error->section_offset = index->records[record].section_offset;
            return false;
        }
    }
    return true;
}

void scoop_stackmap_dispose_record(ScoopStackMapRecord *record) {
    free(record->roots);
    free(record->live_outs);
    *record = (ScoopStackMapRecord){0};
}

void scoop_stackmap_dispose_index(ScoopStackMapIndex *index) {
    if (index == NULL) {
        return;
    }
    for (size_t record = 0; record < index->record_count; record++) {
        scoop_stackmap_dispose_record(&index->records[record]);
    }
    free(index->records);
    *index = (ScoopStackMapIndex){0};
}

const ScoopStackMapRecord *scoop_stackmap_lookup(const ScoopStackMapIndex *index,
                                                 uintptr_t return_pc) {
    if (index == NULL) {
        return NULL;
    }
    size_t low = 0;
    size_t high = index->record_count;
    while (low < high) {
        size_t middle = low + (high - low) / 2;
        uintptr_t candidate = index->records[middle].return_pc;
        if (candidate < return_pc) {
            low = middle + 1;
        } else if (candidate > return_pc) {
            high = middle;
        } else {
            return &index->records[middle];
        }
    }
    return NULL;
}

const char *scoop_stackmap_error_message(ScoopStackMapErrorCode code) {
    switch (code) {
    case SCOOP_STACKMAP_OK:
        return "no error";
    case SCOOP_STACKMAP_INVALID_ARGUMENT:
        return "invalid parser argument or image span";
    case SCOOP_STACKMAP_TRUNCATED:
        return "truncated stack map section";
    case SCOOP_STACKMAP_UNSUPPORTED_VERSION:
        return "unsupported stack map version";
    case SCOOP_STACKMAP_NONZERO_RESERVED:
        return "non-zero reserved or padding field";
    case SCOOP_STACKMAP_INTEGER_OVERFLOW:
        return "stack map size or address overflow";
    case SCOOP_STACKMAP_OUT_OF_MEMORY:
        return "out of memory building stack map index";
    case SCOOP_STACKMAP_INVALID_FUNCTION_COUNT:
        return "function and record counts disagree";
    case SCOOP_STACKMAP_INVALID_FUNCTION_ADDRESS:
        return "function or return PC is outside executable text";
    case SCOOP_STACKMAP_INVALID_STACK_SIZE:
        return "dynamic, zero, or unaligned stack size";
    case SCOOP_STACKMAP_INVALID_RECORD:
        return "invalid statepoint stack map record";
    case SCOOP_STACKMAP_INVALID_LOCATION:
        return "unknown stack map location kind";
    case SCOOP_STACKMAP_INVALID_CONSTANT_INDEX:
        return "constant-pool location index is invalid";
    case SCOOP_STACKMAP_DEOPT_UNSUPPORTED:
        return "statepoint deopt locations are unsupported";
    case SCOOP_STACKMAP_DUPLICATE_SAFEPOINT_ID:
        return "duplicate SafepointId";
    case SCOOP_STACKMAP_DUPLICATE_RETURN_PC:
        return "duplicate statepoint return PC";
    case SCOOP_STACKMAP_TRAILING_DATA:
        return "unexpected trailing stack map data";
    }
    return "unknown stack map parser error";
}
