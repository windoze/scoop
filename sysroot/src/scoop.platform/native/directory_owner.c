#define _GNU_SOURCE 1
#include "directory_owner.h"

#ifdef _WIN32
struct DirectoryOwner {
    HANDLE handle;
    DWORD offset;
    bool buffered;
    bool finished;
    /* A full NT UTF-16 name and its record fit without truncating an entry. */
    union {
        uint64_t alignment;
        unsigned char bytes[65536 + sizeof(FILE_FULL_DIR_INFO)];
    } buffer;
};

static DWORD fill(struct DirectoryOwner *owner) {
    if (GetFileInformationByHandleEx(owner->handle, FileFullDirectoryInfo, owner->buffer.bytes,
                                     (DWORD)sizeof(owner->buffer.bytes))) {
        owner->offset = 0;
        owner->buffered = true;
        return 0;
    }
    DWORD error = GetLastError();
    if (error == ERROR_NO_MORE_FILES) {
        owner->finished = true;
        return 0;
    }
    return error;
}

ScoopPlatformFileResult scoop_platform_directory_owner_close(void *pointer) {
    struct DirectoryOwner *owner = pointer;
    DWORD error = CloseHandle(owner->handle) ? 0 : GetLastError();
    (void)HeapFree(GetProcessHeap(), 0, owner);
    return (ScoopPlatformFileResult){0, (int32_t)error};
}

ScoopPlatformDirectoryOpened scoop_platform_directory_owner_open(const void *path) {
    HANDLE handle = CreateFileW(path, FILE_LIST_DIRECTORY,
                                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL,
                                OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, NULL);
    if (handle == INVALID_HANDLE_VALUE)
        return (ScoopPlatformDirectoryOpened){NULL, (int32_t)GetLastError()};
    struct DirectoryOwner *owner = HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, sizeof(*owner));
    if (owner == NULL) {
        (void)CloseHandle(handle);
        return (ScoopPlatformDirectoryOpened){NULL, ERROR_NOT_ENOUGH_MEMORY};
    }
    owner->handle = handle;
    DWORD error = fill(owner);
    if (error != 0) {
        (void)scoop_platform_directory_owner_close(owner);
        return (ScoopPlatformDirectoryOpened){NULL, (int32_t)error};
    }
    return (ScoopPlatformDirectoryOpened){owner, 0};
}

ScoopPlatformDirectoryEntry scoop_platform_directory_owner_next(void *pointer) {
    struct DirectoryOwner *owner = pointer;
    for (;;) {
        if (owner->finished)
            return (ScoopPlatformDirectoryEntry){NULL, 0, 0};
        if (!owner->buffered) {
            DWORD error = fill(owner);
            if (error != 0)
                return (ScoopPlatformDirectoryEntry){NULL, 0, (int32_t)error};
            if (owner->finished)
                return (ScoopPlatformDirectoryEntry){NULL, 0, 0};
        }
        FILE_FULL_DIR_INFO *entry = (FILE_FULL_DIR_INFO *)(owner->buffer.bytes + owner->offset);
        owner->buffered = entry->NextEntryOffset != 0;
        owner->offset += entry->NextEntryOffset;
        DWORD length = entry->FileNameLength / sizeof(WCHAR);
        if ((length == 1 && entry->FileName[0] == L'.') ||
            (length == 2 && entry->FileName[0] == L'.' && entry->FileName[1] == L'.'))
            continue;
        return (ScoopPlatformDirectoryEntry){entry->FileName, length, 0};
    }
}
#else
#include "directory.h"
#include <errno.h>
#include <string.h>

ScoopPlatformDirectoryOpened scoop_platform_directory_owner_open(const void *path) {
    void *owner = scoop_platform_directory_open(path);
    return (ScoopPlatformDirectoryOpened){owner, owner == NULL ? errno : 0};
}

ScoopPlatformDirectoryEntry scoop_platform_directory_owner_next(void *owner) {
    const char *name = scoop_platform_directory_next(owner);
    int error = name == NULL ? errno : 0;
    return (ScoopPlatformDirectoryEntry){name, name == NULL ? 0 : (int64_t)strlen(name), error};
}

ScoopPlatformFileResult scoop_platform_directory_owner_close(void *owner) {
    int status = scoop_platform_directory_close(owner);
    return (ScoopPlatformFileResult){status, status < 0 ? errno : 0};
}
#endif
