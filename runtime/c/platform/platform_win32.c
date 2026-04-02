// Windows backend placeholder for Scoop C runtime platform layer (v0).
//
// 说明：
// - TODO T1402 的 v0 仅要求提供“占位接口与 build gate”，不要求实现 Windows 行为。
// - 这些实现统一返回失败，以便上层通过 capability gating 或稳定诊断处理。
// - 所有 API 必须是 `static`，避免成为对外导出符号（见 T1401）。

static const char *scoop_platform_env_getenv(const char *key_cstr) {
  (void)key_cstr;
  return 0;
}

static int scoop_platform_time_now_unix_millis(int64_t *out_unix_millis) {
  (void)out_unix_millis;
  return 0;
}

static int scoop_platform_io_write_stdout_all(const uint8_t *buf, size_t len) {
  (void)buf;
  (void)len;
  return 0;
}

static int scoop_platform_io_write_stderr_all(const uint8_t *buf, size_t len) {
  (void)buf;
  (void)len;
  return 0;
}

static int scoop_platform_io_write_stdout_byte(uint8_t byte) {
  (void)byte;
  return 0;
}

static int scoop_platform_io_write_stderr_byte(uint8_t byte) {
  (void)byte;
  return 0;
}

static int scoop_platform_io_read_stdin(uint8_t *buf, size_t len, size_t *out_nread) {
  if (out_nread != 0) {
    *out_nread = 0;
  }
  (void)buf;
  (void)len;
  return 0;
}

