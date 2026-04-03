// Windows backend placeholder for Scoop C runtime platform unwind layer (v0).
//
// 说明：
// - TODO T1411a 的 v0 仅要求提供“占位接口与 build gate”，不要求实现 Windows 行为；
// - 该 backend 统一返回 0，以便上层通过 capability gating 或稳定诊断处理。

static SCOOP_UNWIND_UNUSED uint32_t scoop_platform_unwind_capture_ips(uintptr_t *out_ips,
                                                                      uint32_t out_cap,
                                                                      uint32_t skip_frames) {
  (void)out_ips;
  (void)out_cap;
  (void)skip_frames;
  return 0;
}

