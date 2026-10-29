#ifndef SCOOP_ASYNCIO_COMMON_TEMPORARY_H
#define SCOOP_ASYNCIO_COMMON_TEMPORARY_H

#include "file.h"

typedef struct {
    ScoopIoFile *file;
    ScoopIoChar *path;
    ScoopIoStatus status;
} ScoopIoTemporaryFile;

/* Releases storage only; any acquired file must have been delivered or closed. */
void scoop_io_temporary_free(ScoopIoTemporaryFile *result);
ScoopIoTemporaryFile scoop_io_temporary_take(ScoopIoTemporaryFile *value);
void scoop_io_temporary_path_free(ScoopIoChar *path);

#ifdef _WIN32
int scoop_io_file_temporary(ScoopIoLoop *loop, int64_t id, const wchar_t *parent,
                            const wchar_t *name);
#else
int scoop_io_file_temporary(ScoopIoLoop *loop, int64_t id, const char *pattern);
#endif

#endif
