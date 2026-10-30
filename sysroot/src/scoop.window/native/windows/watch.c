#include "window.h"

struct SWWatch {
  SWApplication *application;
  int64_t identity;
  HANDLE event;
  PTP_WAIT wait;
  bool removing, active;
  SWWatch *next;
};

static void CALLBACK wait_ready(PTP_CALLBACK_INSTANCE instance, void *context,
                                PTP_WAIT wait, TP_WAIT_RESULT result) {
  SWWatch *watch = context;
  // Threadpool waits are one-shot: no managed work or HANDLE consumption here.
  if (!PostMessageW(watch->application->hwnd, SW_READY, (WPARAM)watch->identity,
                    0))
    sw_fail(watch->application, GetLastError());
}

SWResult scoop_window_watch_create(SWApplication *app, int64_t identity,
                                   HANDLE event) {
  DWORD flags;
  if (!GetHandleInformation(event, &flags))
    return (SWResult){NULL, GetLastError()};
  SWWatch *watch = calloc(1, sizeof(*watch));
  if (!watch)
    return (SWResult){NULL, ERROR_NOT_ENOUGH_MEMORY};
  watch->application = app;
  watch->identity = identity;
  watch->event = event;
  watch->wait = CreateThreadpoolWait(wait_ready, watch, NULL);
  if (!watch->wait) {
    DWORD error = GetLastError();
    free(watch);
    return (SWResult){NULL, error};
  }
  watch->next = app->watches;
  app->watches = watch;
  SetThreadpoolWait(watch->wait, event, NULL);
  return (SWResult){watch, 0};
}

void scoop_window_watch_remove(SWWatch *watch) {
  if (watch->removing)
    return;
  watch->removing = true;
  SetThreadpoolWait(watch->wait, NULL, NULL);
  WaitForThreadpoolWaitCallbacks(watch->wait, TRUE);
  CloseThreadpoolWait(watch->wait);
  watch->wait = NULL;
  sw_wake(watch->application);
}

void sw_watch_retire(SWApplication *app) {
  SWWatch **link = &app->watches;
  while (*link) {
    SWWatch *watch = *link;
    if (!watch->removing || watch->active) {
      link = &watch->next;
      continue;
    }
    int64_t identity = watch->identity;
    *link = watch->next;
    free(watch);
    app->callback(app->context, identity, 3);
    // A completion may remove another watch or install a new one.
    link = &app->watches;
  }
}

void sw_watch_message(SWApplication *app, int64_t identity) {
  SWWatch **link = &app->watches;
  while (*link && (*link)->identity != identity)
    link = &(*link)->next;
  SWWatch *watch = *link;
  if (!watch)
    return;
  if (watch->removing || watch->active)
    return;
  watch->active = true;
  app->callback(app->context, identity, 2);
  watch->active = false;
  if (!watch->removing)
    SetThreadpoolWait(watch->wait, watch->event, NULL);
  else
    sw_wake(app);
}
