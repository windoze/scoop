#import "window.h"
#include <errno.h>
#include <fcntl.h>

@interface SWWatch : NSObject
@property(nonatomic, weak) SWApplication *application;
@property(nonatomic) int64_t identity;
@property(nonatomic) CFRunLoopSourceRef source;
@property(nonatomic, strong) NSLock *lock;
@property(nonatomic, strong) dispatch_source_t reader;
@property(nonatomic) BOOL paused;
@property(nonatomic) BOOL pending;
@property(nonatomic) BOOL removing;
@property(nonatomic) BOOL cancelled;
@end
@implementation SWWatch
@end

static void watch_perform(void *context) {
  @autoreleasepool {
    SWWatch *watch = (__bridge SWWatch *)context;
    SWApplication *application = watch.application;
    [watch.lock lock];
    BOOL removed = watch.cancelled;
    BOOL ready = watch.pending && !watch.removing;
    watch.pending = NO;
    [watch.lock unlock];
    if (removed) {
      CFRunLoopSourceInvalidate(watch.source);
      CFRelease(watch.source);
      watch.source = NULL;
      application.callback(application.context, watch.identity, 3);
      [application.watches removeObjectForKey:@(watch.identity)];
      watch.reader = nil;
      return;
    }
    if (ready)
      application.callback(application.context, watch.identity, 2);
    [watch.lock lock];
    if (!watch.removing && watch.paused) {
      watch.paused = NO;
      dispatch_resume(watch.reader);
    }
    [watch.lock unlock];
  }
}

void *scoop_window_watch_create(void *pointer, int64_t identity,
                                int32_t descriptor) {
  if (descriptor < 0 || fcntl(descriptor, F_GETFD) < 0) {
    errno = EBADF;
    return NULL;
  }
  SWApplication *application = (__bridge SWApplication *)pointer;
  SWWatch *watch = [SWWatch new];
  watch.application = application;
  watch.identity = identity;
  watch.lock = [NSLock new];
  CFRunLoopSourceContext context = {0};
  context.info = (__bridge void *)watch;
  context.perform = watch_perform;
  watch.source = CFRunLoopSourceCreate(NULL, 0, &context);
  if (!watch.source) {
    errno = ENOMEM;
    return NULL;
  }
  dispatch_queue_t queue =
      dispatch_queue_create("scoop.window.readable", DISPATCH_QUEUE_SERIAL);
  watch.reader = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ,
                                        (uintptr_t)descriptor, 0, queue);
  if (!watch.reader) {
    CFRelease(watch.source);
    errno = ENOMEM;
    return NULL;
  }
  dispatch_source_set_event_handler(watch.reader, ^{
    @autoreleasepool {
      [watch.lock lock];
      if (!watch.removing && !watch.paused) {
        watch.paused = YES;
        watch.pending = YES;
        dispatch_suspend(watch.reader);
        CFRunLoopSourceSignal(watch.source);
        CFRunLoopWakeUp(CFRunLoopGetMain());
      }
      [watch.lock unlock];
    }
  });
  dispatch_source_set_cancel_handler(watch.reader, ^{
    @autoreleasepool {
      [watch.lock lock];
      watch.cancelled = YES;
      CFRunLoopSourceSignal(watch.source);
      CFRunLoopWakeUp(CFRunLoopGetMain());
      [watch.lock unlock];
    }
  });
  application.watches[@(identity)] = watch;
  CFRunLoopAddSource(CFRunLoopGetMain(), watch.source, kCFRunLoopCommonModes);
  dispatch_resume(watch.reader);
  return (__bridge void *)watch;
}

void scoop_window_watch_remove(void *pointer) {
  SWWatch *watch = (__bridge SWWatch *)pointer;
  [watch.lock lock];
  if (!watch.removing) {
    watch.removing = YES;
    if (watch.paused) {
      watch.paused = NO;
      dispatch_resume(watch.reader);
    }
    dispatch_source_cancel(watch.reader);
  }
  [watch.lock unlock];
}
