import type { Node, QueryCapture } from "web-tree-sitter";
import type { SourceText } from "./text.ts";

export interface IndentEdit {
  line: number;
  length: number;
  text: string;
}

export interface IndentOptions {
  tabSize: number;
  insertSpaces: boolean;
}

interface Scope {
  start: number;
  end: number;
  closing?: number;
  startsAfter: boolean;
}

const openingTokens = new Set(["{", "(", "[", "<", "${", ":", "where"]);
const closingTokens = new Set(["}", ")", "]", ">"]);

function recoveredClosing(opening: Node): Node | null {
  const expected = { "{": "}", "(": ")", "[": "]" }[opening.type];
  let depth = 1;
  for (let node = opening.nextSibling; node; node = node.nextSibling) {
    if (node.type === opening.type) depth++;
    if (node.type === expected && --depth === 0) return node;
  }
  return null;
}

function scopeFor(
  node: Node,
  capture: string,
  source: SourceText,
): Scope | undefined {
  if (capture === "indent.begin") {
    const recovered = openingTokens.has(node.type);
    const container = recovered ? node.parent : node;
    const opening = recovered
      ? node
      : node.children.find((child) => openingTokens.has(child.type));
    if (!container || !opening) return;
    const last = recovered ? recoveredClosing(node) : node.lastChild;
    const closing =
      last && !last.isMissing && closingTokens.has(last.type) ? last : null;
    const end =
      !closing && source.text.slice(container.endIndex).trim() === ""
        ? source.text.length
        : container.endIndex;
    return {
      start: source.positionAt(opening.startIndex).row,
      end: source.positionAt(closing?.endIndex ?? end).row,
      closing: closing?.startIndex,
      startsAfter: true,
    };
  }
  if (capture === "indent.branch" || capture === "indent.continuation") {
    const parent = node.parent;
    if (
      !parent ||
      node.type === "block" ||
      (capture === "indent.branch" && node.type === "if_expression")
    )
      return;
    const start = source.positionAt(node.startIndex).row;
    if (start <= source.positionAt(parent.startIndex).row) return;
    return {
      start,
      end: source.positionAt(node.endIndex).row,
      startsAfter: false,
    };
  }
}

export function indentLines(
  source: SourceText,
  captures: readonly QueryCapture[],
  options: IndentOptions,
  first = 0,
  last = source.lineStarts.length - 1,
  includeBlank = false,
): IndentEdit[] {
  const scopes: Scope[] = [];
  const ignored: { start: number; end: number }[] = [];
  for (const capture of captures) {
    if (capture.name === "indent.ignore") {
      ignored.push({
        start: source.positionAt(capture.node.startIndex).row,
        end: source.positionAt(capture.node.endIndex).row,
      });
    } else {
      const scope = scopeFor(capture.node, capture.name, source);
      if (scope) scopes.push(scope);
    }
  }
  const unit = options.insertSpaces ? " ".repeat(options.tabSize) : "\t";
  const edits: IndentEdit[] = [];
  for (
    let line = first;
    line <= Math.min(last, source.lineStarts.length - 1);
    line++
  ) {
    const text = source.line(line);
    const leading = text.match(/^[\t ]*/)?.[0] ?? "";
    if (
      (!includeBlank && text.trim() === "") ||
      ignored.some((range) => range.start < line && line <= range.end)
    )
      continue;
    const start = source.lineStarts[line] + leading.length;
    const closingPrefix =
      text.slice(leading.length).match(/^[\]})>\t ]*/)?.[0] ?? "";
    let depth = 0;
    for (const scope of scopes) {
      if (
        line < scope.start ||
        line > scope.end ||
        (scope.startsAfter && line === scope.start)
      )
        continue;
      if (
        scope.closing !== undefined &&
        scope.closing >= start &&
        scope.closing < start + closingPrefix.length
      )
        continue;
      depth++;
    }
    const indent = unit.repeat(depth);
    if (leading !== indent)
      edits.push({ line, length: leading.length, text: indent });
  }
  return edits;
}
