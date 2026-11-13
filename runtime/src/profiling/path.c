#define _POSIX_C_SOURCE 200809L
#include "../profiling.h"
#if defined(SCOOP_ENABLE_PROFILING) && SCOOP_ENABLE_PROFILING
#include "path.h"
#include <errno.h>
#include <inttypes.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <wchar.h>
#include <windows.h>
typedef wchar_t PathChar;
#define PATH_TEXT(value) L##value
#define path_length wcslen
#define path_open _wfopen
#define PATH_SEPARATOR L'\\'
static bool separator(PathChar c) { return c == L'/' || c == L'\\'; }
static bool make_directory(const PathChar *path) {
  if (CreateDirectoryW(path, NULL))
    return true;
  DWORD code = GetLastError();
  errno = code == ERROR_ALREADY_EXISTS ? EEXIST : EIO;
  return false;
}
static uint64_t process_id(void) { return GetCurrentProcessId(); }
static PathChar *output_root(void) {
  const wchar_t *value = _wgetenv(L"SCOOP_PROFILE_DIR");
  if (value == NULL || value[0] == 0)
    value = L"scoop-profile";
  DWORD count = GetFullPathNameW(value, 0, NULL, NULL);
  if (count == 0) {
    errno = EIO;
    return NULL;
  }
  wchar_t *absolute = calloc((size_t)count + 8, sizeof(wchar_t));
  if (absolute == NULL)
    return NULL;
  if (GetFullPathNameW(value, count, absolute, NULL) == 0) {
    free(absolute);
    errno = EIO;
    return NULL;
  }
  if (wcsncmp(absolute, L"\\\\?\\", 4) == 0)
    return absolute;
  wchar_t *extended = calloc((size_t)count + 8, sizeof(wchar_t));
  if (extended == NULL) {
    free(absolute);
    return NULL;
  }
  if (wcsncmp(absolute, L"\\\\", 2) == 0) {
    wcscpy(extended, L"\\\\?\\UNC\\");
    wcscat(extended, absolute + 2);
  } else {
    wcscpy(extended, L"\\\\?\\");
    wcscat(extended, absolute);
  }
  free(absolute);
  return extended;
}
static size_t root_end(const PathChar *path) {
  if (wcsncmp(path, L"\\\\?\\UNC\\", 8) != 0)
    return 7;
  size_t pos = 8;
  for (unsigned part = 0; part < 2; part++) {
    while (path[pos] != 0 && !separator(path[pos]))
      pos++;
    if (path[pos] != 0)
      pos++;
  }
  return pos;
}
#else
#include <sys/stat.h>
#include <unistd.h>
typedef char PathChar;
#define PATH_TEXT(value) value
#define path_length strlen
#define path_open fopen
#define PATH_SEPARATOR '/'
static bool separator(PathChar c) { return c == '/'; }
static bool make_directory(const PathChar *path) {
  return mkdir(path, 0777) == 0;
}
static uint64_t process_id(void) { return (uint64_t)getpid(); }
static PathChar *output_root(void) {
  const char *value = getenv("SCOOP_PROFILE_DIR");
  return strdup(value == NULL || value[0] == 0 ? "scoop-profile" : value);
}
static size_t root_end(const PathChar *path) { return path[0] == '/' ? 1 : 0; }
#endif

static bool make_parents(PathChar *path) {
  for (size_t pos = root_end(path); path[pos] != 0; pos++) {
    if (!separator(path[pos]))
      continue;
    PathChar saved = path[pos];
    path[pos] = 0;
    bool success = make_directory(path) || errno == EEXIST;
    path[pos] = saved;
    if (!success)
      return false;
  }
  return make_directory(path) || errno == EEXIST;
}

bool scoop_profile_open_files(FILE **counters, FILE **metrics,
                              uint64_t now_ns) {
  PathChar *root = output_root();
  if (root == NULL)
    return false;
  if (!make_parents(root)) {
    free(root);
    return false;
  }
  size_t length = path_length(root);
  if (length > SIZE_MAX / sizeof(PathChar) - 128) {
    free(root);
    errno = ENAMETOOLONG;
    return false;
  }
  PathChar *path = calloc(length + 128, sizeof(PathChar));
  if (path == NULL) {
    free(root);
    return false;
  }
  memcpy(path, root, length * sizeof(PathChar));
  free(root);
  path[length++] = PATH_SEPARATOR;
  size_t directory_end = 0;
  for (uint64_t attempt = 0;; attempt++) {
    char suffix[96];
    int written =
        snprintf(suffix, sizeof suffix, "run-%" PRIu64 "-%" PRIu64 "-%" PRIu64,
                 process_id(), now_ns, attempt);
    if (written < 0 || (size_t)written >= sizeof suffix) {
      free(path);
      errno = EIO;
      return false;
    }
    for (int i = 0; i <= written; i++)
      path[length + (size_t)i] = (PathChar)suffix[i];
    if (make_directory(path)) {
      directory_end = length + (size_t)written;
      break;
    }
    if (errno != EEXIST || attempt == UINT64_MAX) {
      free(path);
      return false;
    }
  }
  path[directory_end++] = PATH_SEPARATOR;
  static const PathChar raw[] = PATH_TEXT("counts.profraw");
  static const PathChar json[] = PATH_TEXT("runtime.jsonl");
  memcpy(path + directory_end, raw, sizeof raw);
  FILE *counter = path_open(path, PATH_TEXT("wb"));
  if (counter == NULL) {
    free(path);
    return false;
  }
  memcpy(path + directory_end, json, sizeof json);
  FILE *metric = path_open(path, PATH_TEXT("wb"));
  int saved_errno = errno;
  free(path);
  if (metric == NULL) {
    fclose(counter);
    errno = saved_errno;
    return false;
  }
  *counters = counter;
  *metrics = metric;
  return true;
}
#endif
