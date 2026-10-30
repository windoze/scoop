#ifndef SCOOP_WINDOW_WINDOWS_H
#define SCOOP_WINDOW_WINDOWS_H

#define WIN32_LEAN_AND_MEAN
#define UNICODE
#include <windows.h>

#include <dwmapi.h>
#include <imm.h>
#include <limits.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <windowsx.h>

typedef int32_t (*SWCallback)(void *, int64_t, int32_t);
typedef struct {
  int64_t window;
  int32_t kind, flags;
  int64_t a, b, c, d;
  double x, y, z, w, scale, dx, dy;
  const uint8_t *text;
  int64_t text_length;
  int64_t time_seconds, time_nanoseconds;
} SWEventData;
typedef struct {
  int32_t kind;
  int64_t anchor, cursor;
  const uint8_t *text;
  int64_t text_length;
} SWTextEditData;
typedef struct {
  double width, height, pixel_width, pixel_height, scale;
  double x, y, outer_width, outer_height;
  int32_t visible, occluded, focused, minimized, maximized, dark;
} SWSnapshot;
typedef struct {
  void *pointer;
  int64_t error;
} SWResult;
typedef struct SWEvent {
  SWEventData data;
  SWTextEditData edits[2];
  int64_t edit_count;
  struct SWEvent *next;
} SWEvent;
typedef struct SWWatch SWWatch;
typedef struct SWApplication {
  SWCallback callback;
  void *context;
  HWND hwnd;
  HANDLE wake;
  volatile LONG pending, error;
  bool stopped;
  double deadline;
  SWEvent *head, *tail;
  SWWatch *watches;
} SWApplication;
typedef struct SWWindow {
  SWApplication *application;
  HWND hwnd;
  int64_t identity;
  double scale, min_width, min_height, max_width, max_height;
  double increment_width, increment_height;
  bool decorations, resizable, close_button, minimize_button, maximize_button;
  bool transparent, bottom, cursor_visible, pointer_inside, ime_allowed;
  bool marked, composing, destroying;
  int theme;
  HCURSOR cursor;
  HIMC ime;
  RECT caret;
  double caret_x, caret_y, caret_width, caret_height;
  WCHAR surrogate;
  uint64_t modifiers;
  SWEventData pressed[512];
  bool key_down[512];
  uint32_t pressed_buttons;
} SWWindow;

enum { SW_WAKE = WM_APP + 1, SW_READY };
extern const WCHAR sw_window_class[];
void sw_wake(SWApplication *application);
void sw_fail(SWApplication *application, DWORD error);
void sw_enqueue(SWApplication *application, SWEvent *event);
SWEvent *sw_event(SWWindow *window, int32_t kind);
void sw_stamp_input(SWEventData *data);
void sw_emit(SWWindow *window, int32_t kind);
void sw_free_event(SWEvent *event);
void sw_watch_message(SWApplication *application, int64_t identity);
void sw_watch_retire(SWApplication *application);
LRESULT CALLBACK sw_window_proc(HWND hwnd, UINT message, WPARAM wparam,
                                LPARAM lparam);
SWSnapshot sw_snapshot(SWWindow *window);
DWORD sw_style(SWWindow *window);
int64_t sw_size(SWWindow *window, double width, double height);
void sw_minmax(SWWindow *window, MINMAXINFO *info);
void sw_sizing(SWWindow *window, WPARAM edge, RECT *rect);
void sw_update_ime_caret(SWWindow *window);
bool sw_input(SWWindow *window, UINT message, WPARAM wparam, LPARAM lparam,
              LRESULT *result);
bool sw_text(SWWindow *window, UINT message, WPARAM wparam, LPARAM lparam,
             LRESULT *result);
void sw_reset_input(SWWindow *window);
void sw_cancel_composition(SWWindow *window);
void sw_composition(SWWindow *window, const WCHAR *commit, int commit_count,
                    const WCHAR *preedit, int preedit_count, int cursor);
uint64_t sw_modifiers(void);
WCHAR *sw_wide(const uint8_t *text, int64_t length);
uint8_t *sw_utf8(const WCHAR *text, int length, int64_t *bytes);
double sw_now(void);
int sw_theme(SWWindow *window);

#endif
