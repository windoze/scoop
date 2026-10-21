#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <string.h>
#include <sys/stat.h>

int32_t m33_platform_open(const int8_t *path, int32_t flags) {
  return open((const char *)path, flags, 0600);
}

int64_t m33_platform_size(const int8_t *path) {
  struct stat info;
  if (stat((const char *)path, &info) != 0) {
    return -1;
  }
  return info.st_size;
}

int64_t m33_platform_entries(const int8_t *path) {
  DIR *directory = opendir((const char *)path);
  if (directory == NULL) {
    return -1;
  }
  int64_t count = 0;
  struct dirent *entry;
  errno = 0;
  while ((entry = readdir(directory)) != NULL) {
    if (strcmp(entry->d_name, ".") != 0 && strcmp(entry->d_name, "..") != 0) {
      count++;
    }
  }
  int error = errno;
  int result = closedir(directory);
  if (error != 0) {
    errno = error;
    return -1;
  }
  return result == 0 ? count : -1;
}
