#include "work.h"

typedef struct {
  ScoopIoWork work;
  void *payload;
  void (*execute)(void *);
} ScoopIoNativeWork;

static void execute_native_work(uv_work_t *native) {
  ScoopIoNativeWork *request = native->data;
  request->execute(request->payload);
}

static void finish_native_work(uv_work_t *native, int status) {
  ScoopIoNativeWork *request = native->data;
  scoop_io_work_finished(&request->work, status);
}

int scoop_io_native_work(ScoopIoLoop *loop, int64_t id, void *payload,
                         void (*execute)(void *)) {
  ScoopIoNativeWork *request = calloc(1, sizeof(*request));
  if (!request)
    return UV_ENOMEM;
  request->payload = payload;
  request->execute = execute;
  int status = scoop_io_work_submit(&request->work, loop, id,
                                    execute_native_work, finish_native_work);
  if (status < 0)
    free(request);
  return status;
}

void scoop_io_native_work_release(void *payload, void (*release)(void *)) {
  release(payload);
}
