import { useTranslation } from "react-i18next";
import { codeParts, parseGuide } from "../lib/guide";
import { Dialog } from "./Dialog";

function Inline({ text }: { text: string }) {
  return (
    <>
      {codeParts(text).map((part, index) =>
        index % 2 === 1 ? (
          <code key={index} className="mono guide__code">
            {part}
          </code>
        ) : (
          part
        ),
      )}
    </>
  );
}

/** A help article of the "?" menu (IV-20, as in Halbmond FM-21), from locales `guide.<article>`. */
export function GuideDialog({ article, onClose }: { article: string; onClose: () => void }) {
  const { t } = useTranslation();
  const lines = t(`guide.${article}.lines`, { returnObjects: true });
  const blocks = parseGuide(Array.isArray(lines) ? lines.map(String) : []);
  return (
    <Dialog title={t(`guide.${article}.title`)} onClose={onClose} wide>
      <div className="guide">
        {blocks.map((block, index) =>
          block.kind === "heading" ? (
            <h3 key={index} className="guide__heading">
              <Inline text={block.text} />
            </h3>
          ) : block.kind === "list" ? (
            <ul key={index} className="guide__list">
              {block.items.map((item, i) => (
                <li key={i}>
                  <Inline text={item} />
                </li>
              ))}
            </ul>
          ) : (
            <p key={index} className="guide__text">
              <Inline text={block.text} />
            </p>
          ),
        )}
      </div>
      <div className="dialog__actions">
        <button type="button" className="button button--primary" autoFocus onClick={onClose}>
          {t("common.close")}
        </button>
      </div>
    </Dialog>
  );
}
