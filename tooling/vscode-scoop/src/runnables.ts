import type { Node } from "web-tree-sitter";
import type { SyntaxDocument } from "./syntax.ts";

export type RunnableTarget =
  | { kind: "main"; name: string }
  | { kind: "test"; name: string; container: string };

export interface Runnable {
  target: RunnableTarget;
  start: number;
  end: number;
}

function identifier(node: Node): string {
  return node.text.startsWith("`") ? node.text.slice(1, -1) : node.text;
}

function qualifiedName(node: Node): string {
  return node.namedChildren
    .filter((item) => item.type === "identifier")
    .map(identifier)
    .join(".");
}

function child(node: Node, type: string): Node | undefined {
  return node.namedChildren.find((item) => item.type === type);
}

function modifiers(node: Node): string[] {
  return (
    child(node, "modifiers")
      ?.namedChildren.filter((item) => item.type === "modifier")
      .map((item) => item.text) ?? []
  );
}

function ordinaryFunction(node: Node): boolean {
  return (
    node.type === "function_declaration" &&
    !node.childForFieldName("receiver") &&
    !child(node, "type_parameters") &&
    !child(node, "context_parameters") &&
    !modifiers(node).includes("suspend") &&
    (!!node.childForFieldName("body") || !!child(node, "expression_body"))
  );
}

export function runnables(document: SyntaxDocument): Runnable[] {
  const root = document.tree.rootNode;
  const testNames = new Set(["Test", "scoop.test.Test"]);
  for (const node of root.namedChildren) {
    if (node.type === "import_header") {
      const imported = child(node, "qualified_identifier");
      const importedName = imported?.namedChildren
        .filter((item) => item.type === "identifier")
        .at(-1);
      if (importedName && identifier(importedName) === "Test") {
        const alias = node.childForFieldName("alias");
        if (alias) testNames.add(identifier(alias));
      }
    }
    // A locally declared annotation with the same spelling is not a test marker.
    if (node.type === "annotation_class_declaration") {
      const name = node.childForFieldName("name");
      if (name) testNames.delete(identifier(name));
    }
  }
  const marked = (node: Node): boolean =>
    child(node, "modifiers")?.namedChildren.some((annotation) => {
      if (annotation.type !== "annotation") return false;
      const name = annotation.childForFieldName("name");
      return !!name && testNames.has(qualifiedName(name));
    }) ?? false;

  const packageHeader = child(root, "package_header");
  const packageName =
    packageHeader && child(packageHeader, "qualified_identifier");
  const prefix = packageName ? `${qualifiedName(packageName)}.` : "";
  const result: Runnable[] = [];
  const add = (node: Node, target: RunnableTarget) => {
    const name = node.childForFieldName("name");
    if (name && !name.isMissing)
      result.push({ target, start: name.startIndex, end: name.endIndex });
  };
  for (const node of root.namedChildren) {
    const name = node.childForFieldName("name");
    if (!name || name.isMissing) continue;
    if (ordinaryFunction(node) && identifier(name) === "main") {
      add(node, { kind: "main", name: "main" });
    } else if (
      node.type === "class_declaration" &&
      marked(node) &&
      !child(node, "type_parameters") &&
      !modifiers(node).some((value) =>
        ["abstract", "sealed", "inner"].includes(value),
      )
    ) {
      const container = `${prefix}${identifier(name)}`;
      for (const member of child(node, "class_body")?.namedChildren ?? []) {
        const method = member.childForFieldName("name");
        if (
          method &&
          ordinaryFunction(member) &&
          marked(member) &&
          !child(member, "parameters")?.namedChildren.some(
            (item) => item.type === "parameter",
          )
        ) {
          add(member, { kind: "test", name: identifier(method), container });
        }
      }
    }
  }
  return result;
}
