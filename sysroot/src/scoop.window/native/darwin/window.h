#ifndef SCOOP_WINDOW_DARWIN_H
#define SCOOP_WINDOW_DARWIN_H

#import <AppKit/AppKit.h>
#import <CoreFoundation/CoreFoundation.h>
#include <stdint.h>

#include "../abi.h"

@class SWApplication, SWWindow;

@interface SWTextEdit : NSObject
@property(nonatomic) SWTextEditData data;
@property(nonatomic, strong) NSData *text;
@end

@interface SWEvent : NSObject
@property(nonatomic) SWEventData data;
@property(nonatomic, strong) NSData *text;
@property(nonatomic, strong) NSArray<SWTextEdit *> *edits;
@end

@interface SWApplication : NSObject <NSApplicationDelegate>
@property(nonatomic) SWCallback callback;
@property(nonatomic) void *context;
@property(nonatomic) CFRunLoopSourceRef source;
@property(nonatomic) CFRunLoopTimerRef timer;
@property(nonatomic) BOOL deadlineFired;
@property(nonatomic, strong) NSMutableArray<SWEvent *> *events;
@property(nonatomic, strong)
    NSMutableDictionary<NSNumber *, SWWindow *> *windows;
@property(nonatomic, strong) NSMutableDictionary<NSNumber *, id> *watches;
- (void)enqueue:(SWEvent *)event;
@end

@interface SWView : NSView
@property(nonatomic, weak) SWWindow *owner;
@property(nonatomic, strong)
    NSMutableDictionary<NSNumber *, SWEvent *> *pressedKeys;
@property(nonatomic) NSEventModifierFlags modifiers;
@property(nonatomic, strong) NSTrackingArea *tracking;
@property(nonatomic, strong) NSCursor *cursor;
@property(nonatomic) BOOL pointerInside, cursorVisible, cursorHidden;
@property(nonatomic) BOOL imeAllowed, discardingText;
@property(nonatomic) NSRect imeCaret;
@property(nonatomic, strong) NSString *markedText;
@property(nonatomic) NSRange markedSelection;
@property(nonatomic, strong) NSMutableArray<SWTextEdit *> *textEdits;
@property(nonatomic) NSUInteger textBatchDepth;
@property(nonatomic) BOOL textBatchHadMarked, textBatchInvalid;
@property(nonatomic) BOOL compositionConsumed, compositionCommitted, forwardKey;
@end

@interface SWView (Input)
- (void)resetInput;
@end
@interface SWView (Pointer)
- (void)updateCursor;
@end
@interface SWView (Text) <NSTextInputClient>
- (void)beginTextBatch;
- (void)endTextBatch;
- (void)cancelComposition;
@end

void sw_commit(SWView *view, NSString *text);
void sw_modifiers(SWView *view, NSEventModifierFlags flags);

@interface SWWindow : NSObject <NSWindowDelegate>
@property(nonatomic, weak) SWApplication *application;
@property(nonatomic) int64_t identity;
@property(nonatomic, strong) NSWindow *window;
@property(nonatomic, strong) SWView *view;
@property(nonatomic, strong) NSVisualEffectView *effect;
@property(nonatomic) BOOL transparent;
@property(nonatomic) NSInteger attentionRequest;
- (void)emit:(int32_t)kind;
@end

void sw_wake(SWApplication *application);
SWEvent *sw_event(SWWindow *window, int32_t kind);
SWSnapshot sw_snapshot(SWWindow *window);
double sw_desktop_top(void);

#endif
