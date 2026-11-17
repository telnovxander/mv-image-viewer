export const BEGIN: string;
export const END: string;
export function hotkeysMarkdown(
  table: { keys: { keys: string; action: string; done: boolean }[] },
  english: { hotkeys: Record<string, string> },
): string;
export function withHotkeys(readme: string, section: string): string;
