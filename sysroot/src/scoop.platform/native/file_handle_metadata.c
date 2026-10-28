#define _GNU_SOURCE 1
#include "file_handles.h"
#include <stddef.h>

#ifdef _WIN32
static void timestamp(int64_t ticks, int64_t *seconds, int64_t *nanos) {
    int64_t fraction = ticks % INT64_C(10000000);
    *seconds = ticks / INT64_C(10000000) - INT64_C(11644473600);
    if (fraction < 0) {
        --*seconds;
        fraction += INT64_C(10000000);
    }
    *nanos = fraction * 100;
}

ScoopPlatformFileMetadataResult scoop_platform_handle_metadata(ScoopPlatformFileHandle handle) {
    ScoopPlatformFileMetadataResult result = {.data = {.kind = -1}};
    FILE_BASIC_INFO basic;
    FILE_STANDARD_INFO standard;
    FILE_ATTRIBUTE_TAG_INFO tag;
    HANDLE native = scoop_file_handle(handle);
    if (!GetFileInformationByHandleEx(native, FileBasicInfo, &basic, sizeof(basic)) ||
        !GetFileInformationByHandleEx(native, FileStandardInfo, &standard, sizeof(standard)) ||
        !GetFileInformationByHandleEx(native, FileAttributeTagInfo, &tag, sizeof(tag))) {
        result.error = (int32_t)GetLastError();
        return result;
    }
    ScoopPlatformFileMetadata *data = &result.data;
    data->kind = standard.Directory ? 1 : 0;
    if ((tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) &&
        (tag.ReparseTag == IO_REPARSE_TAG_SYMLINK || tag.ReparseTag == IO_REPARSE_TAG_MOUNT_POINT))
        data->kind = 2;
    data->permissions = (basic.FileAttributes & FILE_ATTRIBUTE_READONLY) ? 0444 : 0666;
    if (standard.Directory)
        data->permissions |= 0111;
    data->size = standard.EndOfFile.QuadPart;
    timestamp(basic.LastAccessTime.QuadPart, &data->accessed_seconds, &data->accessed_nanos);
    timestamp(basic.LastWriteTime.QuadPart, &data->modified_seconds, &data->modified_nanos);
    timestamp(basic.ChangeTime.QuadPart, &data->changed_seconds, &data->changed_nanos);
    timestamp(basic.CreationTime.QuadPart, &data->created_seconds, &data->created_nanos);
    return result;
}

ScoopPlatformFileMetadataResult scoop_platform_path_metadata_result(const void *path, bool follow) {
    DWORD flags = FILE_FLAG_BACKUP_SEMANTICS | (follow ? 0 : FILE_FLAG_OPEN_REPARSE_POINT);
    HANDLE native = CreateFileW(path, FILE_READ_ATTRIBUTES,
                                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL,
                                OPEN_EXISTING, flags, NULL);
    if (native == INVALID_HANDLE_VALUE)
        return (ScoopPlatformFileMetadataResult){.data = {.kind = -1},
                                                 .error = (int32_t)GetLastError()};
    ScoopPlatformFileMetadataResult result =
        scoop_platform_handle_metadata(scoop_file_wrap(native));
    (void)CloseHandle(native);
    return result;
}
#else
#include <errno.h>

ScoopPlatformFileMetadataResult scoop_platform_handle_metadata(ScoopPlatformFileHandle handle) {
    ScoopPlatformFileMetadata data =
        scoop_platform_file_metadata(scoop_file_handle(handle), NULL, true);
    return (ScoopPlatformFileMetadataResult){data, data.kind < 0 ? errno : 0};
}

ScoopPlatformFileMetadataResult scoop_platform_path_metadata_result(const void *path, bool follow) {
    ScoopPlatformFileMetadata data = scoop_platform_file_metadata(-1, path, follow);
    return (ScoopPlatformFileMetadataResult){data, data.kind < 0 ? errno : 0};
}
#endif
