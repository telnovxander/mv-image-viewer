// The help articles (IV-20, the same as in Halbmond FM-21): each is a list of lines in locales `guide.<article>.lines`. A line
// starting with "## " is a heading, "- " a list item, "`…`" inside a line is an example in the
// monospace font; anything else is a paragraph.

export type GuideBlock =
  | { kind: "heading"; text: string }
  | { kind: "list"; items: string[] }
  | { kind: "paragraph"; text: string };

export function parseGuide(lines: readonly string[]): GuideBlock[] {
  const blocks: GuideBlock[] = [];
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) continue;
    if (line.startsWith("## ")) blocks.push({ kind: "heading", text: line.slice(3) });
    else if (line.startsWith("- ")) {
      const last = blocks[blocks.length - 1];
      if (last?.kind === "list") last.items.push(line.slice(2));
      else blocks.push({ kind: "list", items: [line.slice(2)] });
    } else blocks.push({ kind: "paragraph", text: line });
  }
  return blocks;
}

/** Splits a line into plain text and `code` parts (odd indexes are code). */
export function codeParts(text: string): string[] {
  return text.split("`");
}
