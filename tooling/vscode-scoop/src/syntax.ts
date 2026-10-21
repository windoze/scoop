import { readFile } from "node:fs/promises";
import path from "node:path";
import {
  Language,
  Parser,
  Query,
  type Tree,
  Edit as TreeEdit,
} from "web-tree-sitter";
import { advancePosition, SourceText } from "./text.ts";

export interface Edit {
  rangeOffset: number;
  rangeLength: number;
  text: string;
}

export class SyntaxDocument {
  readonly parser: Parser;
  source: SourceText;
  version: number;
  private previousTree: Tree | null = null;
  private dirty = true;

  constructor(language: Language, text: string, version: number) {
    this.parser = new Parser().setLanguage(language);
    this.source = new SourceText(text);
    this.version = version;
  }

  get tree(): Tree {
    if (this.dirty) {
      const next = this.parser.parse(this.source.text, this.previousTree);
      if (!next)
        throw new Error("Tree-sitter could not parse the Scoop document.");
      this.previousTree?.delete();
      this.previousTree = next;
      this.dirty = false;
    }
    if (!this.previousTree)
      throw new Error("The Scoop syntax tree has been disposed.");
    return this.previousTree;
  }

  update(changes: readonly Edit[], text: string, version: number): void {
    // Changes in a VS Code event refer to the same old document. Applying them
    // from the end preserves the coordinates of every remaining edit.
    for (const change of [...changes].sort(
      (a, b) => b.rangeOffset - a.rangeOffset,
    )) {
      const start = this.source.treePositionAt(change.rangeOffset);
      this.previousTree?.edit(
        new TreeEdit({
          startIndex: change.rangeOffset,
          oldEndIndex: change.rangeOffset + change.rangeLength,
          newEndIndex: change.rangeOffset + change.text.length,
          startPosition: start,
          oldEndPosition: this.source.treePositionAt(
            change.rangeOffset + change.rangeLength,
          ),
          newEndPosition: advancePosition(start, change.text),
        }),
      );
    }
    this.source = new SourceText(text);
    this.version = version;
    this.dirty = true;
  }

  replace(text: string, version: number): void {
    this.previousTree?.delete();
    this.previousTree = null;
    this.source = new SourceText(text);
    this.version = version;
    this.dirty = true;
  }

  dispose(): void {
    this.previousTree?.delete();
    this.previousTree = null;
    this.parser.delete();
  }
}

export class ScoopSyntax {
  readonly language: Language;
  readonly highlights: Query;
  readonly folds: Query;
  readonly indents: Query;
  private readonly documents = new Map<string, SyntaxDocument>();

  private constructor(language: Language, queries: Query[]) {
    this.language = language;
    [this.highlights, this.folds, this.indents] = queries;
  }

  static async load(assets: string): Promise<ScoopSyntax> {
    await Parser.init({
      locateFile: (file: string) => path.join(assets, file),
    });
    const language = await Language.load(
      path.join(assets, "tree-sitter-scoop.wasm"),
    );
    const queries: Query[] = [];
    try {
      for (const name of ["highlights", "folds", "indents"]) {
        const source = await readFile(path.join(assets, `${name}.scm`), "utf8");
        queries.push(new Query(language, source));
      }
      return new ScoopSyntax(language, queries);
    } catch (error) {
      for (const query of queries) query.delete();
      throw error;
    }
  }

  document(uri: string, text: string, version: number): SyntaxDocument {
    let document = this.documents.get(uri);
    if (!document) {
      document = new SyntaxDocument(this.language, text, version);
      this.documents.set(uri, document);
    } else if (document.version !== version) {
      // A document can change before the provider is first requested or while
      // its language is switched. A fresh parse handles that missed event.
      document.replace(text, version);
    }
    return document;
  }

  change(
    uri: string,
    changes: readonly Edit[],
    text: string,
    version: number,
  ): void {
    this.documents.get(uri)?.update(changes, text, version);
  }

  close(uri: string): void {
    this.documents.get(uri)?.dispose();
    this.documents.delete(uri);
  }

  dispose(): void {
    for (const document of this.documents.values()) document.dispose();
    this.documents.clear();
    this.highlights.delete();
    this.folds.delete();
    this.indents.delete();
  }
}
