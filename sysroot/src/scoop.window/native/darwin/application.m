#import "window.h"

@implementation SWEvent
@end

void sw_wake(SWApplication *application) {
  CFRunLoopSourceSignal(application.source);
  CFRunLoopWakeUp(CFRunLoopGetMain());
}

static void perform(void *context) {
  @autoreleasepool {
    SWApplication *application = (__bridge SWApplication *)context;
    application.callback(application.context, 0,
                         application.deadlineFired ? 1 : 0);
    application.deadlineFired = NO;
  }
}

static void deadline(CFRunLoopTimerRef timer, void *context) {
  SWApplication *application = (__bridge SWApplication *)context;
  application.deadlineFired = YES;
  sw_wake(application);
}

@implementation SWApplication
- (void)enqueue:(SWEvent *)event {
  [self.events addObject:event];
  sw_wake(self);
}
- (NSApplicationTerminateReply)applicationShouldTerminate:
    (NSApplication *)sender {
  SWEvent *event = [SWEvent new];
  SWEventData data = {0};
  data.kind = 101;
  event.data = data;
  [self enqueue:event];
  return NSTerminateCancel;
}
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:
    (NSApplication *)sender {
  return NO;
}
- (void)applicationDidChangeScreenParameters:(NSNotification *)notification {
  SWEvent *event = [SWEvent new];
  SWEventData data = {0};
  data.kind = 100;
  event.data = data;
  [self enqueue:event];
}
@end

int32_t scoop_window_is_main_thread(void) {
  return [NSThread isMainThread] ? 1 : 0;
}

void *scoop_window_application_create(SWCallback callback, void *context,
                                      int32_t policy) {
  @autoreleasepool {
    NSApplication *app = [NSApplication sharedApplication];
    SWApplication *owner = [SWApplication new];
    owner.callback = callback;
    owner.context = context;
    owner.events = [NSMutableArray new];
    owner.windows = [NSMutableDictionary new];
    owner.watches = [NSMutableDictionary new];
    CFRunLoopSourceContext source = {0};
    source.info = (__bridge void *)owner;
    source.perform = perform;
    owner.source = CFRunLoopSourceCreate(NULL, 0, &source);
    if (!owner.source)
      return NULL;
    CFRunLoopAddCommonMode(CFRunLoopGetMain(),
                           (__bridge CFStringRef)NSEventTrackingRunLoopMode);
    CFRunLoopAddCommonMode(CFRunLoopGetMain(),
                           (__bridge CFStringRef)NSModalPanelRunLoopMode);
    CFRunLoopAddSource(CFRunLoopGetMain(), owner.source, kCFRunLoopCommonModes);
    app.delegate = owner;
    [app setActivationPolicy:(NSApplicationActivationPolicy)policy];
    [app finishLaunching];
    return (__bridge_retained void *)owner;
  }
}

int32_t scoop_window_application_run(void *pointer) {
  @autoreleasepool {
    @try {
      [NSApp run];
      return 0;
    } @catch (NSException *exception) {
      return -1;
    }
  }
}

void scoop_window_application_wake(void *pointer) {
  sw_wake((__bridge SWApplication *)pointer);
}

void scoop_window_application_deadline(void *pointer, double seconds) {
  SWApplication *owner = (__bridge SWApplication *)pointer;
  if (owner.timer) {
    CFRunLoopTimerInvalidate(owner.timer);
    CFRelease(owner.timer);
    owner.timer = NULL;
  }
  if (seconds < 0)
    return;
  CFRunLoopTimerContext context = {0};
  context.info = pointer;
  owner.timer = CFRunLoopTimerCreate(NULL, CFAbsoluteTimeGetCurrent() + seconds,
                                     0, 0, 0, deadline, &context);
  CFRunLoopAddTimer(CFRunLoopGetMain(), owner.timer, kCFRunLoopCommonModes);
}

void scoop_window_application_stop(void *pointer) {
  [NSApp stop:nil];
  NSEvent *wake = [NSEvent otherEventWithType:NSEventTypeApplicationDefined
                                     location:NSZeroPoint
                                modifierFlags:0
                                    timestamp:0
                                 windowNumber:0
                                      context:nil
                                      subtype:0
                                        data1:0
                                        data2:0];
  [NSApp postEvent:wake atStart:YES];
}

void scoop_window_application_destroy(void *pointer) {
  SWApplication *owner = (__bridge_transfer SWApplication *)pointer;
  NSApp.delegate = nil;
  if (owner.timer) {
    CFRunLoopTimerInvalidate(owner.timer);
    CFRelease(owner.timer);
  }
  CFRunLoopSourceInvalidate(owner.source);
  CFRelease(owner.source);
  owner.source = NULL;
}

void *scoop_window_next_event(void *pointer) {
  SWApplication *owner = (__bridge SWApplication *)pointer;
  if (!owner.events.count)
    return NULL;
  SWEvent *event = owner.events.firstObject;
  [owner.events removeObjectAtIndex:0];
  return (__bridge_retained void *)event;
}

SWEventData scoop_window_event_data(void *pointer) {
  SWEvent *event = (__bridge SWEvent *)pointer;
  SWEventData data = event.data;
  data.text = event.text.bytes;
  data.text_length = (int64_t)event.text.length;
  return data;
}

void scoop_window_event_release(void *pointer) {
  (void)CFBridgingRelease(pointer);
}
