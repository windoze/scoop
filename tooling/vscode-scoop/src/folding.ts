import type { QueryCapture } from "web-tree-sitter";
import type { SourceText } from "./text.ts";

export interface Fold {
  start: number;
  end: number;
  kind?: "comment" | "imports";
}

export function folds(
  source: SourceText,
  captures: readonly QueryCapture[],
): Fold[] {
  const result: Fold[] = [];
  const groups = new Map<string, Fold[]>();
  for (const capture of captures) {
    const node = capture.node;
    if (node.isMissing) continue;
    const start = source.positionAt(node.startIndex).row;
    let end = source.positionAt(node.endIndex).row;
    if (
      capture.name === "fold.import" ||
      capture.name === "fold.line_comment"
    ) {
      if (
        capture.name === "fold.line_comment" &&
        source.line(start).trimStart().indexOf("//") !== 0
      )
        continue;
      const group = groups.get(capture.name) ?? [];
      group.push({ start, end });
      groups.set(capture.name, group);
      continue;
    }
    const closing = node.lastChild;
    if (
      closing &&
      !closing.isMissing &&
      /^(\}|\)|\]|>|""")$/.test(closing.text)
    ) {
      const before = source.text.slice(
        source.lineStarts[end],
        closing.startIndex,
      );
      if (before.trim() === "") end--;
    } else if (source.positionAt(node.endIndex).column === 0) end--;
    if (end > start)
      result.push({
        start,
        end,
        kind: capture.name === "fold.comment" ? "comment" : undefined,
      });
  }
  for (const [name, group] of groups) {
    group.sort((a, b) => a.start - b.start);
    let run: Fold | undefined;
    for (const fold of group) {
      if (run && fold.start <= run.end + 1)
        run.end = Math.max(run.end, fold.end);
      else {
        if (run && run.end > run.start) result.push(run);
        run = { ...fold, kind: name === "fold.import" ? "imports" : "comment" };
      }
    }
    if (run && run.end > run.start) result.push(run);
  }
  const seen = new Set<string>();
  return result
    .sort((a, b) => a.start - b.start || b.end - a.end)
    .filter((fold) => {
      const key = `${fold.start}:${fold.end}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
}
