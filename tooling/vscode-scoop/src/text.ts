export interface Position {
  row: number;
  column: number;
}

// Tree-sitter counts LF rows; VS Code also recognizes a standalone CR. Both
// APIs index JavaScript strings in UTF-16 code units, including surrogate pairs.
export class SourceText {
  readonly text: string;
  readonly lineStarts = [0];
  readonly treeLineStarts = [0];

  constructor(text: string) {
    this.text = text;
    for (let index = 0; index < text.length; index++) {
      if (text[index] === "\n") {
        this.lineStarts.push(index + 1);
        this.treeLineStarts.push(index + 1);
      } else if (text[index] === "\r" && text[index + 1] !== "\n") {
        this.lineStarts.push(index + 1);
      }
    }
  }

  positionAt(offset: number): Position {
    return positionIn(this.lineStarts, offset);
  }

  treePositionAt(offset: number): Position {
    return positionIn(this.treeLineStarts, offset);
  }

  line(row: number): string {
    return this.text
      .slice(this.lineStarts[row], this.lineStarts[row + 1] ?? this.text.length)
      .replace(/[\r\n]+$/, "");
  }
}

function positionIn(starts: readonly number[], offset: number): Position {
  let low = 0;
  let high = starts.length;
  while (low + 1 < high) {
    const middle = (low + high) >>> 1;
    if (starts[middle] <= offset) low = middle;
    else high = middle;
  }
  return { row: low, column: offset - starts[low] };
}

export function advancePosition(start: Position, text: string): Position {
  const lines = text.split("\n");
  return lines.length === 1
    ? { row: start.row, column: start.column + text.length }
    : {
        row: start.row + lines.length - 1,
        column: lines[lines.length - 1].length,
      };
}
