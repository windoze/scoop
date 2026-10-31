#include "../../native/windows/window.h"

// Exercise the production UTF-16 normalization on a real window. These samples
// do not simulate an installed IME's candidate selection or language behavior.
void scoop_window_test_text_batch(HWND hwnd, int32_t sample) {
  SWWindow *window = (SWWindow *)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
  const WCHAR chinese[] = {0x4e2d, 0xd83d, 0xde00};
  const WCHAR commit[] = {0x4e2d, 0x6587};
  const WCHAR japanese[] = {0x306b, 0xd83d, 0xde00};
  const WCHAR word[] = {0x65e5, 0x672c, 0x8a9e};
  const WCHAR invalid[] = {0xd83d};
  if (sample == 0 || sample == 1)
    sw_composition(window, NULL, 0, chinese, 3, sample ? 1 : 3);
  else if (sample == 2)
    sw_composition(window, commit, 2, japanese, 3, 3);
  else if (sample == 3)
    sw_composition(window, L"lost", 4, chinese, 3, 2);
  else if (sample == 4)
    sw_composition(window, NULL, 0, word, 3, 3);
  else if (sample == 5)
    sw_composition(window, NULL, 0, invalid, 1, 1);
  else if (sample == 6)
    sw_composition(window, NULL, 0, chinese, 3, 3);
  else if (sample == 7)
    sw_composition(window, commit, 2, NULL, 0, -1);
  else if (sample == 8)
    sw_composition(window, L"A", 1, NULL, 0, -1);
}

int32_t scoop_window_test_caret(HWND hwnd) {
  HIMC context = ImmGetContext(hwnd);
  if (!context)
    return 0;
  CANDIDATEFORM value = {0};
  BOOL ok = ImmGetCandidateWindow(context, 0, &value);
  ImmReleaseContext(hwnd, context);
  double scale = GetDpiForWindow(hwnd) / 96.0;
  return ok && value.dwStyle == CFS_EXCLUDE &&
         value.ptCurrentPos.x == (LONG)round(12 * scale) &&
         value.ptCurrentPos.y == (LONG)round(20 * scale) &&
         value.rcArea.right == (LONG)ceil(14 * scale) &&
         value.rcArea.bottom == (LONG)ceil(38 * scale);
}

void scoop_window_test_input(HWND hwnd) {
  BYTE original[256], state[256] = {0};
  GetKeyboardState(original);
  SetKeyboardState(state);
  LPARAM key = (0x1e << 16) | 1;
  SendMessageW(hwnd, WM_KEYDOWN, 'A', key);
  SendMessageW(hwnd, WM_KEYDOWN, 'A', key | (1LL << 30));
  SendMessageW(hwnd, WM_KILLFOCUS, 0, 0);
  SendMessageW(hwnd, WM_KEYUP, 'A', key | (3LL << 30));
  state[VK_SHIFT] = state[VK_RSHIFT] = 0x80;
  SetKeyboardState(state);
  SendMessageW(hwnd, WM_KEYDOWN, VK_SHIFT, (0x36 << 16) | 1);
  state[VK_SHIFT] = state[VK_RSHIFT] = 0;
  SetKeyboardState(state);
  SendMessageW(hwnd, WM_KEYUP, VK_SHIFT, (0x36 << 16) | (3LL << 30) | 1);
  SendMessageW(hwnd, WM_KEYDOWN, VK_NUMLOCK, (0x145 << 16) | 1);
  SendMessageW(hwnd, WM_KEYUP, VK_NUMLOCK, (0x145 << 16) | (3LL << 30) | 1);
  SendMessageW(hwnd, WM_KEYDOWN, VK_PAUSE, (0x45 << 16) | 1);
  SendMessageW(hwnd, WM_KEYUP, VK_PAUSE, (0x45 << 16) | (3LL << 30) | 1);
  SendMessageW(hwnd, WM_KEYDOWN, 255, (0x1ff << 16) | 1);
  SendMessageW(hwnd, WM_KEYUP, 255, (0x1ff << 16) | (3LL << 30) | 1);
  SetKeyboardState(original);
  LPARAM position = MAKELPARAM(24, 36);
  SendMessageW(hwnd, WM_MOUSEMOVE, 0, position);
  SendMessageW(hwnd, WM_LBUTTONDBLCLK, MK_LBUTTON, position);
  SendMessageW(hwnd, WM_LBUTTONUP, 0, position);
  POINT screen = {24, 36};
  ClientToScreen(hwnd, &screen);
  SendMessageW(hwnd, WM_MOUSEWHEEL, MAKEWPARAM(0, 60),
               MAKELPARAM(screen.x, screen.y));
  SendMessageW(hwnd, WM_MOUSEHWHEEL, MAKEWPARAM(0, -120),
               MAKELPARAM(screen.x, screen.y));
  SendMessageW(hwnd, WM_MOUSELEAVE, 0, 0);
}

void scoop_window_test_pointer_cancellation(HWND hwnd) {
  const LPARAM inside = MAKELPARAM(24, 36);
  const LPARAM outside = MAKELPARAM(-17, -19);
  SendMessageW(hwnd, WM_LBUTTONDOWN, MK_LBUTTON, inside);
  SendMessageW(hwnd, WM_MOUSEMOVE, MK_LBUTTON, outside);
  SendMessageW(hwnd, WM_MOUSELEAVE, 0, 0);
  SendMessageW(hwnd, WM_KILLFOCUS, 0, 0);
  SendMessageW(hwnd, WM_CANCELMODE, 0, 0);
  SendMessageW(hwnd, WM_LBUTTONUP, 0, outside);

  SendMessageW(hwnd, WM_LBUTTONDOWN, MK_LBUTTON, inside);
  SendMessageW(hwnd, WM_RBUTTONDOWN, MK_LBUTTON | MK_RBUTTON, inside);
  SendMessageW(hwnd, WM_CANCELMODE, 0, 0);
  SendMessageW(hwnd, WM_LBUTTONUP, MK_RBUTTON, inside);
  SendMessageW(hwnd, WM_RBUTTONUP, 0, inside);

  SendMessageW(hwnd, WM_LBUTTONDOWN, MK_LBUTTON, inside);
  ReleaseCapture();
  SendMessageW(hwnd, WM_LBUTTONUP, 0, inside);

  SendMessageW(hwnd, WM_LBUTTONDOWN, MK_LBUTTON, inside);
  SendMessageW(hwnd, WM_LBUTTONUP, 0, inside);
}
