# Spiegelmond — rules for agents

Spiegelmond is the image viewer of the Moonshine Vibecoder family (alias **IV**).
Tasks, decisions and the Definition of Done live in the MV repository
(`telnovxander/moonshine-vibecoder`): `projects/IV/PROJECT.md`, `projects/IV/tasks/IV-NN.md`,
and its `CLAUDE.md` (statuses, checkpoints, autonomy). The family standard is
`projects/MV/STANDARD.md`, localization `projects/MV/I18N.md`, style `projects/MV/BRAND.md`.

## Safety (mandatory)

**Перед любой работой с удалением файлов или изменением метаданных перечитай раздел
«Безопасность (ОБЯЗАТЕЛЬНО)» в MV/projects/IV/PROJECT.md.**

(Before any work that deletes files or changes metadata, re-read the section
"Безопасность (ОБЯЗАТЕЛЬНО)" in `projects/IV/PROJECT.md` of the MV repository. A task that breaks
any point of that section cannot go to In Review. In short: delete only to the Recycle Bin, never
permanently; metadata changes only after a preview, atomically, with an undo journal and with
the pixels unchanged; a duplicate group always keeps at least one file; tests only on files
from `testdata/`, never on real user data.)

## Layout and commands

To be added by task IV-01 (Tauri + React skeleton). Keep the safety section above unchanged.

Commit messages: short English with the task key, e.g. `IV-03: add folder listing`.
