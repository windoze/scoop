// POSIX backend for Scoop C runtime platform layer (v0).
//
// 注意：
// - 本文件通过 `platform.h` 以 `#include` 方式被包含到 runtime 的编译单元中；
// - 所有 API 必须是 `static`，避免成为对外导出符号（见 T1401）。

#include <errno.h>
#include <stdlib.h>
#include <sys/time.h>
#include <unistd.h>

static const char *scoop_platform_env_getenv(const char *key_cstr) {
  if (key_cstr == 0) {
    return 0;
  }
  return getenv(key_cstr);
}

static int scoop_platform_time_now_unix_millis(int64_t *out_unix_millis) {
  if (out_unix_millis == 0) {
    return 0;
  }

  struct timeval tv;
  int ok = gettimeofday(&tv, 0);
  if (ok != 0) {
    return 0;
  }

  int64_t sec = (int64_t)tv.tv_sec;
  int64_t usec = (int64_t)tv.tv_usec;
  *out_unix_millis = (sec * 1000) + (usec / 1000);
  return 1;
}

static int scoop_platform_io_write_all_fd(int fd, const uint8_t *buf, size_t len) {
  if (len == 0) {
    return 1;
  }
  if (buf == 0) {
    return 0;
  }

  const uint8_t *p = buf;
  size_t remaining = len;
  while (remaining > 0) {
    ssize_t n = write(fd, p, remaining);
    if (n < 0) {
      if (errno == EINTR) {
        continue;
      }
      return 0;
    }
    if (n == 0) {
      // `write` 返回 0 通常意味着不可恢复的错误（例如管道已关闭）。
      return 0;
    }

    p += (size_t)n;
    remaining -= (size_t)n;
  }

  return 1;
}

static int scoop_platform_io_write_stdout_all(const uint8_t *buf, size_t len) {
  return scoop_platform_io_write_all_fd(1, buf, len);
}

static int scoop_platform_io_write_stderr_all(const uint8_t *buf, size_t len) {
  return scoop_platform_io_write_all_fd(2, buf, len);
}

static int scoop_platform_io_write_stdout_byte(uint8_t byte) {
  return scoop_platform_io_write_all_fd(1, &byte, 1);
}

static int scoop_platform_io_write_stderr_byte(uint8_t byte) {
  return scoop_platform_io_write_all_fd(2, &byte, 1);
}

static int scoop_platform_io_read_stdin(uint8_t *buf, size_t len, size_t *out_nread) {
  if (out_nread == 0) {
    return 0;
  }
  *out_nread = 0;

  if (len == 0) {
    return 1;
  }
  if (buf == 0) {
    return 0;
  }

  for (;;) {
    ssize_t n = read(0, buf, len);
    if (n < 0) {
      if (errno == EINTR) {
        continue;
      }
      return 0;
    }

    *out_nread = (size_t)n;
    return 1;
  }
}

