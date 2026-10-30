#define UNICODE
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <commctrl.h>
#include <stdint.h>

static int active_mode, completed_modes;
static const UINT mode_message = WM_APP + 101;
static const UINT_PTR timer_id = 0x53434f50;

static BOOL CALLBACK close_dialog(HWND hwnd, LPARAM owner) {
  WCHAR name[32];
  GetClassNameW(hwnd, name, 32);
  if (GetWindow(hwnd, GW_OWNER) == (HWND)owner &&
      lstrcmpW(name, L"#32770") == 0)
    PostMessageW(hwnd, WM_COMMAND, IDCANCEL, 0);
  return TRUE;
}

static void CALLBACK end_mode(HWND hwnd, UINT message, UINT_PTR identity,
                              DWORD time) {
  KillTimer(hwnd, identity);
  if (active_mode == 1)
    EnumThreadWindows(GetCurrentThreadId(), close_dialog, (LPARAM)hwnd);
  else if (active_mode == 2)
    EndMenu();
  else if (active_mode == 3)
    PostMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE, 0);
}

static LRESULT CALLBACK mode_proc(HWND hwnd, UINT message, WPARAM wp, LPARAM lp,
                                  UINT_PTR identity, DWORD_PTR reference) {
  if (message != mode_message)
    return DefSubclassProc(hwnd, message, wp, lp);
  active_mode = (int)wp;
  if (!SetTimer(hwnd, timer_id, (UINT)lp, end_mode)) {
    active_mode = 0;
    return 0;
  }
  if (active_mode == 1) {
    MessageBoxW(hwnd, L"Scoop native modal-loop test", L"Scoop", MB_OKCANCEL);
  } else if (active_mode == 2) {
    HMENU menu = CreatePopupMenu();
    AppendMenuW(menu, MF_STRING, 1, L"Scoop native menu-loop test");
    RECT rect;
    GetWindowRect(hwnd, &rect);
    TrackPopupMenu(menu, TPM_RETURNCMD, rect.left + 40, rect.top + 40, 0, hwnd,
                   NULL);
    DestroyMenu(menu);
  } else if (active_mode == 3) {
    // DefWindowProc enters the real move/size loop after the Scoop pump
    // returned.
    SendMessageW(hwnd, WM_SYSCOMMAND, SC_SIZE | WMSZ_BOTTOMRIGHT, 0);
  }
  KillTimer(hwnd, timer_id);
  active_mode = 0;
  ++completed_modes;
  RemoveWindowSubclass(hwnd, mode_proc, 1);
  return 0;
}

int32_t scoop_window_test_schedule_mode(HWND hwnd, int32_t mode,
                                        int32_t milliseconds) {
  if (!SetWindowSubclass(hwnd, mode_proc, 1, 0))
    return 0;
  return PostMessageW(hwnd, mode_message, (WPARAM)mode, milliseconds) != 0;
}
int32_t scoop_window_test_active_mode(void) { return active_mode; }
int32_t scoop_window_test_completed_modes(void) { return completed_modes; }

int32_t scoop_window_test_system_dpi(void) {
  return SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE) != 0;
}
