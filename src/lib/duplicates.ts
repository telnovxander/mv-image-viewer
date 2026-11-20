// IV-17: the choice of what to delete in duplicate groups, pure and tested.
import type { DuplicateGroup } from "./backend";

/** Whether `path` is inside one of the protected folders (point 19), on Windows or elsewhere. */
export function isProtected(path: string, folders: readonly string[]): boolean {
  const norm = (p: string) => p.replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();
  const file = norm(path);
  return folders.some((folder) => file.startsWith(`${norm(folder)}\\`));
}

/** The default choice: every file but the one kept, never a protected one. */
export function defaultChoice(groups: readonly DuplicateGroup[], protectedFolders: readonly string[]): string[][] {
  return groups.map((g) => g.files.filter((_, i) => i !== g.keep && !isProtected(g.files[i].path, protectedFolders)).map((f) => f.path));
}

/** Toggle `path` in group `index`; a change that would empty the group (point 16) is refused. */
export function toggleChoice(choice: string[][], groups: readonly DuplicateGroup[], index: number, path: string): string[][] {
  const current = choice[index] ?? [];
  const next = current.includes(path) ? current.filter((p) => p !== path) : [...current, path];
  if (next.length >= groups[index].files.length) return choice;
  return choice.map((c, i) => (i === index ? next : c));
}
