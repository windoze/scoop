import type { QueryCapture } from "web-tree-sitter";
import type { SourceText } from "./text.ts";

export const tokenTypes = [
  "namespace",
  "type",
  "class",
  "interface",
  "struct",
  "enum",
  "enumMember",
  "typeParameter",
  "function",
  "method",
  "parameter",
  "property",
  "variable",
  "decorator",
  "keyword",
  "modifier",
  "operator",
  "string",
  "number",
  "comment",
] as const;
export const tokenModifiers = [
  "declaration",
  "readonly",
  "defaultLibrary",
  "documentation",
] as const;

export interface Highlight {
  line: number;
  start: number;
  length: number;
  type: number;
  modifiers: number;
}

interface Span {
  start: number;
  end: number;
  type: number;
  modifiers: number;
  priority: number;
  pattern: number;
}

const captureTypes: Record<string, (typeof tokenTypes)[number]> = {
  module: "namespace",
  type: "type",
  "type.class": "class",
  "type.interface": "interface",
  "type.struct": "struct",
  "type.enum": "enum",
  "type.parameter": "typeParameter",
  "type.builtin": "type",
  constant: "enumMember",
  function: "function",
  "function.call": "function",
  "function.method": "method",
  "function.method.call": "method",
  "variable.parameter": "parameter",
  "variable.member": "property",
  variable: "variable",
  "variable.readonly": "variable",
  "variable.builtin": "variable",
  attribute: "decorator",
  label: "variable",
  keyword: "keyword",
  "keyword.modifier": "modifier",
  operator: "operator",
  punctuation: "operator",
  string: "string",
  character: "string",
  boolean: "keyword",
  number: "number",
  comment: "comment",
};

function spanFor(capture: QueryCapture): Span | undefined {
  const name = capture.name;
  const type = captureTypes[name] ?? captureTypes[name.split(".")[0]];
  if (
    !type ||
    capture.node.isMissing ||
    capture.node.endIndex <= capture.node.startIndex
  )
    return;
  const modifiers =
    (name.endsWith(".readonly") ? 1 << 1 : 0) |
    (name.endsWith(".builtin") ? 1 << 2 : 0) |
    (name.endsWith(".documentation") ? 1 << 3 : 0);
  return {
    start: capture.node.startIndex,
    end: capture.node.endIndex,
    type: tokenTypes.indexOf(type),
    modifiers,
    priority: Number(capture.setProperties?.priority ?? 0),
    pattern: capture.patternIndex,
  };
}

function outranks(left: Span, right: Span): boolean {
  return (
    left.priority > right.priority ||
    (left.priority === right.priority &&
      (left.end - left.start < right.end - right.start ||
        (left.end - left.start === right.end - right.start &&
          left.pattern > right.pattern)))
  );
}

// Queries deliberately overlap (a name and its role, or a string and an escape).
// VS Code requires disjoint, single-line tokens. Sweep capture boundaries to
// keep the most specific classification without dropping surrounding text.
export function highlights(
  source: SourceText,
  captures: readonly QueryCapture[],
): Highlight[] {
  const spans = captures
    .map(spanFor)
    .filter((span): span is Span => span !== undefined);
  const events = spans
    .flatMap((span) => [
      { offset: span.start, start: true, span },
      { offset: span.end, start: false, span },
    ])
    .sort((a, b) => a.offset - b.offset);
  const active = new Set<Span>();
  const result: Highlight[] = [];
  let previous = 0;
  for (let index = 0; index < events.length; ) {
    const offset = events[index].offset;
    let winner: Span | undefined;
    for (const span of active) {
      if (!winner || outranks(span, winner)) winner = span;
    }
    if (winner && previous < offset)
      appendLines(source, result, previous, offset, winner);
    while (index < events.length && events[index].offset === offset) {
      const event = events[index++];
      if (event.start) active.add(event.span);
      else active.delete(event.span);
    }
    previous = offset;
  }
  return result;
}

function appendLines(
  source: SourceText,
  result: Highlight[],
  start: number,
  end: number,
  span: Span,
): void {
  let { row } = source.positionAt(start);
  while (start < end && row < source.lineStarts.length) {
    const lineStart = source.lineStarts[row];
    const lineEnd = lineStart + source.line(row).length;
    const length = Math.min(end, lineEnd) - start;
    if (length > 0) {
      const last = result[result.length - 1];
      if (
        last &&
        last.line === row &&
        last.start + last.length === start - lineStart &&
        last.type === span.type &&
        last.modifiers === span.modifiers
      )
        last.length += length;
      else
        result.push({
          line: row,
          start: start - lineStart,
          length,
          type: span.type,
          modifiers: span.modifiers,
        });
    }
    row++;
    start = source.lineStarts[row] ?? end;
  }
}
