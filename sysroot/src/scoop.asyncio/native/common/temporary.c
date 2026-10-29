#include "temporary.h"

void scoop_io_temporary_free(ScoopIoTemporaryFile *result) {
    if (!result)
        return;
    free(result->file);
    free(result->path);
    free(result);
}

ScoopIoTemporaryFile scoop_io_temporary_take(ScoopIoTemporaryFile *value) {
    ScoopIoTemporaryFile result = *value;
    free(value);
    return result;
}

void scoop_io_temporary_path_free(ScoopIoChar *path) { free(path); }
