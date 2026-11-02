import { Ban, Settings2, Undo2 } from "lucide-react";
import { Button } from "../ui";
import { useT } from "../../lib/i18n";
import { navItem, type NavKey } from "../../lib/nav";

/**
 * 功能被关闭时的落地页。
 *
 * 导航里已经把入口藏掉了，这里只服务一种情况：用户手改 URL（`#provider`）或者
 * 从旧书签进来。给出「为什么」与「去哪儿开」，并明确数据没被删。
 */
export function FeatureDisabled({
  page,
  onOpenSettings,
  onBack,
}: {
  page: NavKey;
  onOpenSettings: () => void;
  onBack: () => void;
}) {
  const t = useT();
  const item = navItem(page);
  return (
    <div className="flex min-h-[60vh] flex-1 items-center justify-center px-6 py-14">
      <div className="panel w-full max-w-[520px] px-7 py-8">
        <div className="flex items-start gap-3.5">
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-line bg-surface-2 text-text-dim">
            <Ban className="h-5 w-5" />
          </span>
          <div className="min-w-0">
            <h2 className="text-[17px] font-semibold text-text-strong">
              {t("disabled.title", { name: item.label })}
            </h2>
            <p className="mt-1 text-[12.5px] leading-relaxed text-text-dim">
              {t("disabled.body")}
            </p>
          </div>
        </div>

        <div className="mt-5 space-y-2 rounded-lg border border-line bg-surface-2/60 px-3.5 py-3">
          <p className="text-[12px] leading-relaxed text-text-muted">{t("disabled.keep")}</p>
          <p className="mono text-[11.5px] leading-relaxed text-text-dim">
            {t("disabled.files")}
          </p>
        </div>

        <div className="mt-6 flex items-center gap-2">
          <Button variant="primary" onClick={onOpenSettings}>
            <Settings2 className="h-3.5 w-3.5" />
            {t("disabled.goSettings")}
          </Button>
          <Button variant="outline" onClick={onBack}>
            <Undo2 className="h-3.5 w-3.5" />
            {t("disabled.backHome")}
          </Button>
        </div>
      </div>
    </div>
  );
}
