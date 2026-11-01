#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <cmath>
#include <cstdint>
#include <dwmapi.h>
#include <imm.h>
#include <memory>
#include <new>

extern "C" void scoop_window_test_text_batch(HWND, int32_t);

struct ViewProbe {
  HWND window;
  HWND parent;
  HWND firstChild;
  HWND child = nullptr;
  LONG_PTR procedure;

  ~ViewProbe() {
    if (child && IsWindow(child))
      DestroyWindow(child);
  }
  bool matches() const {
    return IsWindow(window) && GetParent(window) == parent &&
           GetWindow(window, GW_CHILD) == firstChild &&
           GetWindowLongPtrW(window, GWLP_WNDPROC) == procedure;
  }
};

extern "C" {
int64_t scoop_surface_test_desktop_pixel(HWND window, double x, double y) {
  if (!IsWindow(window) || FAILED(DwmFlush()))
    return -1;
  const double scale = GetDpiForWindow(window) / 96.0;
  POINT point{static_cast<LONG>(std::round(x * scale)),
              static_cast<LONG>(std::round(y * scale))};
  if (!ClientToScreen(window, &point))
    return -1;
  HDC desktop = GetDC(nullptr);
  if (!desktop)
    return -1;
  const COLORREF pixel = GetPixel(desktop, point.x, point.y);
  ReleaseDC(nullptr, desktop);
  return pixel == CLR_INVALID ? -1LL : static_cast<int64_t>(pixel);
}

ViewProbe *scoop_surface_test_view_create(HWND window, bool customContent) {
  if (!IsWindow(window))
    return nullptr;
  auto probe = std::unique_ptr<ViewProbe>(new (std::nothrow) ViewProbe);
  if (!probe)
    return nullptr;
  probe->window = window;
  probe->parent = GetParent(window);
  probe->procedure = GetWindowLongPtrW(window, GWLP_WNDPROC);
  if (customContent) {
    probe->child =
        CreateWindowExW(0, L"STATIC", L"", WS_CHILD | WS_VISIBLE, 4, 4, 12, 12,
                        window, nullptr, GetModuleHandleW(nullptr), nullptr);
    if (!probe->child)
      return nullptr;
  }
  probe->firstChild = GetWindow(window, GW_CHILD);
  return probe.release();
}

bool scoop_surface_test_view_check(ViewProbe *probe, int32_t width,
                                   int32_t height, double scale) {
  RECT rect;
  return probe->matches() && GetClientRect(probe->window, &rect) &&
         rect.right == width && rect.bottom == height &&
         std::abs(GetDpiForWindow(probe->window) / 96.0 - scale) < 0.000001;
}

bool scoop_surface_test_view_restored(ViewProbe *probe) {
  // Reattachment in the host test separately proves the composition target was
  // retired.
  return !IsWindow(probe->window) || probe->matches();
}

void scoop_surface_test_view_drop(ViewProbe *probe) { delete probe; }

bool scoop_surface_test_view_button(ViewProbe *probe, double x, double y,
                                    bool pressed) {
  if (!IsWindow(probe->window))
    return false;
  const double scale = GetDpiForWindow(probe->window) / 96.0;
  const LPARAM position = MAKELPARAM(static_cast<short>(std::round(x * scale)),
                                     static_cast<short>(std::round(y * scale)));
  SendMessageW(probe->window, WM_MOUSEMOVE, 0, position);
  SendMessageW(probe->window, pressed ? WM_LBUTTONDOWN : WM_LBUTTONUP,
               pressed ? MK_LBUTTON : 0, position);
  return true;
}

bool scoop_surface_test_view_text(ViewProbe *probe, bool commit) {
  if (!IsWindow(probe->window))
    return false;
  // The window test Cone owns protocol injection at the production UTF-16
  // boundary.
  scoop_window_test_text_batch(probe->window, commit ? 8 : 0);
  return true;
}

bool scoop_surface_test_view_caret(ViewProbe *probe, double x, double y,
                                   double width, double height) {
  HIMC context = ImmGetContext(probe->window);
  if (!context)
    return false;
  CANDIDATEFORM candidate{};
  const BOOL found = ImmGetCandidateWindow(context, 0, &candidate);
  ImmReleaseContext(probe->window, context);
  const double scale = GetDpiForWindow(probe->window) / 96.0;
  return found && candidate.dwStyle == CFS_EXCLUDE &&
         candidate.ptCurrentPos.x == static_cast<LONG>(std::round(x * scale)) &&
         candidate.ptCurrentPos.y == static_cast<LONG>(std::round(y * scale)) &&
         candidate.rcArea.left == candidate.ptCurrentPos.x &&
         candidate.rcArea.top == candidate.ptCurrentPos.y &&
         candidate.rcArea.right ==
             static_cast<LONG>(std::ceil((x + width) * scale)) &&
         candidate.rcArea.bottom ==
             static_cast<LONG>(std::ceil((y + height) * scale));
}
}
