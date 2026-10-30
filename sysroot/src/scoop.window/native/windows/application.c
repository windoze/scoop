#include "window.h"

const WCHAR sw_window_class[] = L"Scoop.Window";
static const WCHAR host_class[] = L"Scoop.Window.Host";
static DWORD main_thread;

// The normal CRT initializer runs on the startup thread before Scoop main.
__attribute__((constructor)) static void remember_main_thread(void) {
  main_thread = GetCurrentThreadId();
}

int32_t scoop_window_is_main_thread(void) {
  return GetCurrentThreadId() == main_thread;
}

double sw_now(void) {
  LARGE_INTEGER value, frequency;
  QueryPerformanceCounter(&value);
  QueryPerformanceFrequency(&frequency);
  return (double)value.QuadPart / (double)frequency.QuadPart;
}

void sw_fail(SWApplication *app, DWORD error) {
  InterlockedCompareExchange(&app->error,
                             (LONG)(error ? error : ERROR_GEN_FAILURE), 0);
  sw_wake(app);
}

void sw_wake(SWApplication *app) {
  SetEvent(app->wake);
  if (!InterlockedExchange(&app->pending, 1)) {
    if (!PostMessageW(app->hwnd, SW_WAKE, 0, 0))
      InterlockedCompareExchange(&app->error, (LONG)GetLastError(), 0);
  }
}

static void perform(SWApplication *app, int stage) {
  ResetEvent(app->wake);
  InterlockedExchange(&app->pending, 0);
  sw_watch_retire(app);
  if (app->deadline >= 0 && sw_now() >= app->deadline) {
    KillTimer(app->hwnd, 1);
    app->deadline = -1;
    stage = 1;
  }
  if (!app->stopped)
    app->callback(app->context, 0, stage);
}

static void arm_deadline(SWApplication *app) {
  double ms = ceil((app->deadline - sw_now()) * 1000.0);
  UINT delay = ms <= 1.0                 ? 1
               : ms > USER_TIMER_MAXIMUM ? USER_TIMER_MAXIMUM
                                         : (UINT)ms;
  if (!SetTimer(app->hwnd, 1, delay, NULL))
    sw_fail(app, GetLastError());
}

static LRESULT CALLBACK host_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
  SWApplication *app = (SWApplication *)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
  if (msg == WM_NCCREATE) {
    app = ((CREATESTRUCTW *)lp)->lpCreateParams;
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (LONG_PTR)app);
  }
  if (!app)
    return DefWindowProcW(hwnd, msg, wp, lp);
  switch (msg) {
  case SW_WAKE:
    perform(app, 0);
    return 0;
  case SW_READY:
    sw_watch_message(app, (int64_t)wp);
    return 0;
  case WM_TIMER:
    if (wp == 1) {
      KillTimer(hwnd, 1);
      if (app->deadline >= 0) {
        if (sw_now() < app->deadline)
          arm_deadline(app);
        else {
          app->deadline = -1;
          perform(app, 1);
        }
      }
    }
    return 0;
  case WM_DISPLAYCHANGE:
  case WM_QUERYENDSESSION: {
    SWEvent *event = calloc(1, sizeof(*event));
    if (!event)
      sw_fail(app, ERROR_NOT_ENOUGH_MEMORY);
    else {
      event->data.kind = msg == WM_DISPLAYCHANGE ? 100 : 101;
      sw_enqueue(app, event);
    }
    // A shutdown request is delivered as an intent; forced termination is an OS
    // boundary.
    return msg == WM_QUERYENDSESSION ? FALSE : 0;
  }
  default:
    return DefWindowProcW(hwnd, msg, wp, lp);
  }
}

SWResult scoop_window_application_create(SWCallback callback, void *context,
                                         int32_t policy) {
  DPI_AWARENESS_CONTEXT required = DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2;
  if (!AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(), required)) {
    if (!SetProcessDpiAwarenessContext(required))
      return (SWResult){NULL, GetLastError()};
    if (!AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(), required))
      return (SWResult){NULL, ERROR_INVALID_STATE};
  }
  HINSTANCE instance = GetModuleHandleW(NULL);
  WNDCLASSEXW cls = {.cbSize = sizeof(cls),
                     .lpfnWndProc = host_proc,
                     .hInstance = instance,
                     .lpszClassName = host_class};
  if (!RegisterClassExW(&cls))
    return (SWResult){NULL, GetLastError()};
  cls.lpfnWndProc = sw_window_proc;
  cls.lpszClassName = sw_window_class;
  cls.style = CS_DBLCLKS;
  if (!RegisterClassExW(&cls)) {
    DWORD error = GetLastError();
    UnregisterClassW(host_class, instance);
    return (SWResult){NULL, error};
  }
  SWApplication *app = calloc(1, sizeof(*app));
  DWORD error = ERROR_NOT_ENOUGH_MEMORY;
  if (app) {
    app->callback = callback;
    app->context = context;
    app->deadline = -1;
    app->wake = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (app->wake)
      app->hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, host_class, L"", WS_POPUP,
                                  0, 0, 0, 0, NULL, NULL, instance, app);
    if (app->hwnd)
      return (SWResult){app, 0};
    error = GetLastError();
    if (app->wake)
      CloseHandle(app->wake);
    free(app);
  }
  UnregisterClassW(sw_window_class, instance);
  UnregisterClassW(host_class, instance);
  return (SWResult){NULL, error};
}

int32_t scoop_window_application_run(SWApplication *app) {
  while (!app->stopped) {
    MSG msg;
    // A finite snapshot gives native input a turn even in explicit Poll mode.
    for (int n = 0; n < 64 && PeekMessageW(&msg, NULL, 0, 0, PM_REMOVE); ++n) {
      if (msg.message == WM_QUIT)
        return ERROR_CANCELLED;
      TranslateMessage(&msg);
      DispatchMessageW(&msg);
      if (app->stopped)
        break;
    }
    LONG error = InterlockedExchange(&app->error, 0);
    if (error)
      return error;
    if (app->stopped)
      break;
    if (app->deadline >= 0 && sw_now() >= app->deadline) {
      perform(app, 1);
      continue;
    }
    DWORD result = MsgWaitForMultipleObjectsEx(
        1, &app->wake, INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    if (result == WAIT_FAILED)
      return (int32_t)GetLastError();
    if (result == WAIT_OBJECT_0 &&
        !PeekMessageW(&msg, app->hwnd, SW_WAKE, SW_WAKE, PM_NOREMOVE))
      perform(app, 0);
  }
  return 0;
}

void scoop_window_application_wake(SWApplication *app) { sw_wake(app); }
void scoop_window_application_deadline(SWApplication *app, double seconds) {
  KillTimer(app->hwnd, 1);
  app->deadline = seconds < 0 ? -1 : sw_now() + seconds;
  if (seconds >= 0)
    arm_deadline(app);
}
void scoop_window_application_stop(SWApplication *app) {
  app->stopped = true;
  sw_wake(app);
}
void scoop_window_application_destroy(SWApplication *app) {
  KillTimer(app->hwnd, 1);
  DestroyWindow(app->hwnd);
  CloseHandle(app->wake);
  while (app->head) {
    SWEvent *event = app->head;
    app->head = event->next;
    sw_free_event(event);
  }
  UnregisterClassW(sw_window_class, GetModuleHandleW(NULL));
  UnregisterClassW(host_class, GetModuleHandleW(NULL));
  free(app);
}

void sw_enqueue(SWApplication *app, SWEvent *event) {
  if (app->tail)
    app->tail->next = event;
  else
    app->head = event;
  app->tail = event;
  sw_wake(app);
}
SWEvent *sw_event(SWWindow *window, int32_t kind) {
  SWEvent *event = calloc(1, sizeof(*event));
  if (!event) {
    sw_fail(window->application, ERROR_NOT_ENOUGH_MEMORY);
    return NULL;
  }
  event->data.window = window->identity;
  event->data.kind = kind;
  return event;
}
void *scoop_window_next_event(SWApplication *app) {
  SWEvent *event = app->head;
  if (event) {
    app->head = event->next;
    if (!app->head)
      app->tail = NULL;
  }
  return event;
}
SWEventData scoop_window_event_data(SWEvent *event) { return event->data; }
void sw_free_event(SWEvent *event) {
  free((void *)event->data.text);
  for (int64_t n = 0; n < event->edit_count; ++n)
    free((void *)event->edits[n].text);
  free(event);
}
void scoop_window_event_release(SWEvent *event) { sw_free_event(event); }

WCHAR *sw_wide(const uint8_t *text, int64_t length) {
  if (length < 0 || length > INT_MAX) {
    SetLastError(ERROR_INVALID_PARAMETER);
    return NULL;
  }
  int count =
      length ? MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS,
                                   (const char *)text, (int)length, NULL, 0)
             : 0;
  if (length && !count)
    return NULL;
  WCHAR *result = calloc((size_t)count + 1, sizeof(WCHAR));
  if (!result) {
    SetLastError(ERROR_NOT_ENOUGH_MEMORY);
    return NULL;
  }
  if (count)
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, (const char *)text,
                        (int)length, result, count);
  return result;
}
uint8_t *sw_utf8(const WCHAR *text, int length, int64_t *bytes) {
  int count = length ? WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text,
                                           length, NULL, 0, NULL, NULL)
                     : 0;
  if (length && !count)
    return NULL;
  uint8_t *result = calloc((size_t)count + 1, 1);
  if (!result) {
    SetLastError(ERROR_NOT_ENOUGH_MEMORY);
    return NULL;
  }
  if (count)
    WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text, length,
                        (char *)result, count, NULL, NULL);
  *bytes = count;
  return result;
}
