#import "../../native/darwin/window.h"

static void command_keys(SWView *view) {
  for (NSNumber *code in @[ @123, @8 ]) {
    BOOL shortcut = code.unsignedShortValue == 8;
    for (NSNumber *type in @[ @(NSEventTypeKeyDown), @(NSEventTypeKeyUp) ]) {
      NSEvent *event = [NSEvent
                     keyEventWithType:(NSEventType)type.unsignedIntegerValue
                             location:NSZeroPoint
                        modifierFlags:shortcut ? NSEventModifierFlagCommand : 0
                            timestamp:1.0
                         windowNumber:view.window.windowNumber
                              context:nil
                           characters:shortcut ? @"c" : @"\uf702"
          charactersIgnoringModifiers:shortcut ? @"c" : @"\uf702"
                            isARepeat:NO
                              keyCode:code.unsignedShortValue];
      if (event.type == NSEventTypeKeyDown)
        [view keyDown:event];
      else
        [view keyUp:event];
    }
  }
}

int32_t scoop_window_test_text(void *pointer) {
  @autoreleasepool {
    @try {
      SWView *view = (__bridge SWView *)pointer;
      NSRange absent = NSMakeRange(NSNotFound, 0);
      if ((view.window.styleMask & NSWindowStyleMaskTitled) ||
          !view.window.canBecomeKeyWindow || !view.window.canBecomeMainWindow ||
          !view.window.keyWindow || view.window.firstResponder != view)
        return 10;
      if (!view.imeAllowed || !view.inputContext || [view hasMarkedText])
        return 1;
      command_keys(view);
      NSRange actual = NSMakeRange(9, 2);
      NSRect caret = [view firstRectForCharacterRange:absent
                                          actualRange:&actual];
      NSRect expected = [view.window
          convertRectToScreen:[view convertRect:NSMakeRect(12, 34, 2, 18)
                                         toView:nil]];
      if (!NSEqualRects(caret, expected) || actual.location != NSNotFound)
        return 2;
      if ([view attributedSubstringForProposedRange:NSMakeRange(0, 1)
                                        actualRange:&actual] ||
          actual.location != NSNotFound ||
          [view characterIndexForPoint:NSZeroPoint] != NSNotFound)
        return 3;

      [view beginTextBatch];
      [view setMarkedText:@"中😀"
             selectedRange:NSMakeRange(1, 2)
          replacementRange:absent];
      if (!NSEqualRanges([view markedRange], NSMakeRange(0, 3)) ||
          !NSEqualRanges([view selectedRange], NSMakeRange(1, 2)))
        return 4;
      [view setMarkedText:[[NSAttributedString alloc] initWithString:@"文"]
             selectedRange:NSMakeRange(0, 1)
          replacementRange:NSMakeRange(1, 2)];
      [view insertText:@"字" replacementRange:absent];
      [view setMarkedText:@"글"
             selectedRange:NSMakeRange(1, 0)
          replacementRange:absent];
      [view doCommandBySelector:@selector(insertNewline:)];
      if (!view.compositionConsumed || !view.compositionCommitted ||
          view.forwardKey)
        return 5;
      [view endTextBatch];

      [view setMarkedText:@"é😀"
             selectedRange:NSMakeRange(1, 2)
          replacementRange:absent];
      [view unmarkText];
      [view unmarkText];
      if ([view hasMarkedText] || [view markedRange].location != NSNotFound)
        return 6;

      // An invalid new transaction must not clear the application's selection.
      [view beginTextBatch];
      [view setMarkedText:@"valid"
             selectedRange:NSMakeRange(0, 0)
          replacementRange:absent];
      [view setMarkedText:@"😀"
             selectedRange:NSMakeRange(1, 0)
          replacementRange:absent];
      [view endTextBatch];
      if ([view hasMarkedText])
        return 7;
      [view setMarkedText:@"invalid"
             selectedRange:NSMakeRange(NSUIntegerMax - 1, 8)
          replacementRange:absent];
      [view setMarkedText:@"outside"
             selectedRange:absent
          replacementRange:NSMakeRange(10, 0)];
      if ([view hasMarkedText])
        return 8;

      // A previously published composition is removed once after rejection.
      [view setMarkedText:@"old"
             selectedRange:NSMakeRange(3, 0)
          replacementRange:absent];
      [view beginTextBatch];
      [view setMarkedText:@"valid"
             selectedRange:NSMakeRange(0, 0)
          replacementRange:absent];
      [view setMarkedText:@"😀"
             selectedRange:NSMakeRange(1, 0)
          replacementRange:absent];
      [view endTextBatch];

      [view setMarkedText:@"abcd"
             selectedRange:NSMakeRange(4, 0)
          replacementRange:absent];
      [view insertText:[[NSAttributedString alloc] initWithString:@"Q"]
          replacementRange:NSMakeRange(1, 2)];

      [view setMarkedText:@"cancel"
             selectedRange:absent
          replacementRange:absent];
      [view resetInput];
      if ([view hasMarkedText] || !view.imeAllowed)
        return 9;

      [view setMarkedText:@"disable"
             selectedRange:absent
          replacementRange:absent];
      return 0;
    } @catch (NSException *exception) {
      return -1;
    }
  }
}
