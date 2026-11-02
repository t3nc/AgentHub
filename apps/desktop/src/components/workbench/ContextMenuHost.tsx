import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { clsx } from "clsx";
import { ChevronRight, Loader2 } from "lucide-react";
import { useContextMenu, type MenuOpenState } from "../../lib/context-menu";
import { isMenuItem, type MenuEntry, type MenuItem } from "../../lib/menu";
import { tr } from "../../lib/i18n/runtime";

const MARGIN = 8;
const MAX_HEIGHT = 420;
const SUB_WIDTH = 220;
const SUB_HEIGHT = 320;

interface Position {
  left: number;
  top: number;
  maxHeight: number;
  width: number;
}

interface SubState {
  index: number;
  left: number;
  top: number;
  active: number;
}

/** 关闭时把事件转交给下层元素，实现「右键另一处直接换菜单」而不是先关再点。 */
function relayContextMenu(x: number, y: number) {
  window.setTimeout(() => {
    const target = document.elementFromPoint(x, y);
    target?.dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: x, clientY: y })
    );
  }, 0);
}

/**
 * 右键菜单渲染器。
 *
 * 必须 portal 到 body：页面外层有带动画的容器，`position: fixed` 一旦被祖先的
 * transform 收编，菜单会被排到文档末尾（技能库的悬浮操作条就这样丢过一次）。
 */
export function ContextMenuHost() {
  const { state, close } = useContextMenu();
  return createPortal(
    <AnimatePresence>
      {state && <MenuPanel key={state.key} state={state} onClose={close} />}
    </AnimatePresence>,
    document.body
  );
}

function MenuPanel({ state, onClose }: { state: MenuOpenState; onClose: () => void }) {
  const panelRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef(new Map<string, HTMLButtonElement>());
  const [pos, setPos] = useState<Position | null>(null);
  const [active, setActive] = useState(-1);
  const [sub, setSub] = useState<SubState | null>(null);
  const [running, setRunning] = useState<string | null>(null);
  const hoverTimer = useRef<number | null>(null);

  const entries = state.entries;
  const items = useMemo(
    () =>
      entries
        .map((entry, index) => ({ entry, index }))
        .filter((row): row is { entry: MenuItem; index: number } => isMenuItem(row.entry)),
    [entries]
  );
  const selectable = useMemo(() => items.filter((row) => !row.entry.disabled), [items]);

  // 测量后钳制到视口内；右侧/底部放不下就翻到指针另一侧
  useLayoutEffect(() => {
    const el = panelRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const maxHeight = Math.min(MAX_HEIGHT, window.innerHeight - MARGIN * 2);
    const height = Math.min(rect.height, maxHeight);
    // 先把锚点收进视口：键盘触发 / 内容超长的页面里，指针坐标可能落在窗口之外
    const anchorX = Math.min(Math.max(state.x, MARGIN), window.innerWidth - MARGIN);
    const anchorY = Math.min(Math.max(state.y, MARGIN), window.innerHeight - MARGIN);
    let left = anchorX;
    if (left + rect.width > window.innerWidth - MARGIN) {
      left = Math.max(MARGIN, anchorX - rect.width);
    }
    let top = anchorY;
    if (top + height > window.innerHeight - MARGIN) {
      top = Math.max(MARGIN, anchorY - height);
    }
    setPos({ left, top, maxHeight, width: rect.width });
  }, [state.key, state.x, state.y]);

  /** 量好、可见之后再收焦点：visibility:hidden 的元素 focus() 会被浏览器忽略 */
  useEffect(() => {
    if (pos) panelRef.current?.focus();
  }, [pos]);

  // 打开时高亮第一个「非危险」项：一按回车不会落在删除上
  useEffect(() => {
    const first = selectable.find((row) => !row.entry.danger) ?? selectable[0];
    setActive(first ? first.index : -1);
  }, [state.key, selectable]);

  // 打开期间：页面滚动 / 缩放 / 失焦都直接收起，避免菜单悬在错误位置。
  // 但菜单**自己**滚动（长菜单要能往下翻）不能算——capture 阶段会把面板内部的
  // scroll 也收上来，不排除的话一滚就关。
  useEffect(() => {
    const onScroll = (event: Event) => {
      const target = event.target;
      if (target instanceof Element && target.closest("[data-context-menu],[data-context-submenu]")) {
        return;
      }
      onClose();
    };
    const close = () => onClose();
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
    };
  }, [onClose]);

  /** 键盘移动时把选中项滚进可视区（长菜单被 maxHeight 截断时必需） */
  useEffect(() => {
    if (active < 0) return;
    const entry = entries[active];
    if (entry && isMenuItem(entry)) {
      itemRefs.current.get(entry.id)?.scrollIntoView({ block: "nearest" });
    }
  }, [active, entries]);

  useEffect(() => {
    return () => {
      if (hoverTimer.current) window.clearTimeout(hoverTimer.current);
    };
  }, []);

  /** 子菜单独立用 fixed 定位：父面板是可滚动容器，绝对定位的子菜单会被裁掉 */
  const openSub = (index: number, item: MenuItem) => {
    const el = itemRefs.current.get(item.id);
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const flip = rect.right + SUB_WIDTH + MARGIN > window.innerWidth;
    const height = Math.min(SUB_HEIGHT, window.innerHeight - MARGIN * 2);
    const top = Math.min(rect.top - 4, window.innerHeight - height - MARGIN);
    setSub({
      index,
      left: flip ? Math.max(MARGIN, rect.left - SUB_WIDTH + 4) : rect.right - 4,
      top: Math.max(MARGIN, top),
      active: 0,
    });
  };

  const move = (delta: 1 | -1) => {
    if (selectable.length === 0) return;
    const current = selectable.findIndex((row) => row.index === active);
    const next = current < 0 ? 0 : (current + delta + selectable.length) % selectable.length;
    setActive(selectable[next].index);
    setSub(null);
  };

  const runItem = (item: MenuItem) => {
    if (item.disabled || running) return;
    if (item.children?.length) {
      const index = entries.findIndex((entry) => entry === item);
      if (index >= 0) openSub(index, item);
      return;
    }
    if (!item.run) return;
    if (item.pending) {
      // 慢动作（重新扫描 / 检测版本）：留着菜单显示转圈，做完再收
      setRunning(item.id);
      void Promise.resolve()
        .then(() => item.run?.())
        .finally(() => {
          setRunning(null);
          onClose();
        });
      return;
    }
    // 快动作：先收菜单，避免压住随后弹出的对话框 / 抽屉
    onClose();
    window.setTimeout(() => void item.run?.(), 0);
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      if (sub) setSub(null);
      else onClose();
      return;
    }
    if (event.key === "Tab") {
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (sub) {
        const children = subEntries(entries, sub.index);
        if (children.length > 0) {
          const delta = event.key === "ArrowDown" ? 1 : -1;
          setSub({ ...sub, active: (sub.active + delta + children.length) % children.length });
        }
      } else {
        move(event.key === "ArrowDown" ? 1 : -1);
      }
      return;
    }
    if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      setActive(event.key === "Home" ? (selectable[0]?.index ?? -1) : (selectable[selectable.length - 1]?.index ?? -1));
      return;
    }
    if (event.key === "ArrowLeft") {
      if (sub) {
        event.preventDefault();
        setSub(null);
      }
      return;
    }
    if (event.key === "ArrowRight") {
      const item = items.find((row) => row.index === active)?.entry;
      if (item?.children?.length) {
        event.preventDefault();
        openSub(items.find((row) => row.index === active)?.index ?? 0, item);
      }
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (sub) {
        const children = subEntries(entries, sub.index);
        const pick = children[sub.active];
        if (pick) runItem(pick);
        return;
      }
      const item = items.find((row) => row.index === active)?.entry;
      if (item) runItem(item);
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.08 }}
      className="fixed inset-0 z-[80]"
      onMouseDown={onClose}
      onContextMenu={(event) => {
        event.preventDefault();
        const { clientX: x, clientY: y } = event;
        onClose();
        relayContextMenu(x, y);
      }}
    >
      <motion.div
        ref={panelRef}
        role="menu"
        tabIndex={-1}
        aria-label={tr("menu.aria")}
        data-context-menu
        initial={{ opacity: 0, y: -4 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -2 }}
        transition={{ duration: 0.12, ease: "easeOut" }}
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={onKeyDown}
        style={{
          left: pos?.left ?? state.x,
          top: pos?.top ?? state.y,
          maxHeight: pos?.maxHeight ?? MAX_HEIGHT,
          visibility: pos ? "visible" : "hidden",
        }}
        className="fixed w-[240px] overflow-y-auto overscroll-contain rounded-lg border border-line bg-ink-900 py-1 shadow-[var(--shadow-drawer)] outline-none"
      >
        {entries.map((entry, index) => {
          if (entry.kind === "separator") {
            return <div key={`sep-${index}`} className="my-1 h-px bg-line" />;
          }
          if (entry.kind === "label") {
            return (
              <p
                key={`label-${index}`}
                className="px-2 pb-0.5 pt-1.5 text-[11px] font-medium tracking-[0.06em] text-text-dim"
              >
                {entry.text}
              </p>
            );
          }
          const item = entry;
          const Icon = item.icon;
          const isActive = !item.disabled && active === index;
          const hasChildren = Boolean(item.children?.length);
          const right = item.reason ?? item.shortcut ?? item.hint;
          return (
            <button
              key={item.id}
              ref={(node) => {
                if (node) itemRefs.current.set(item.id, node);
                else itemRefs.current.delete(item.id);
              }}
              type="button"
              role="menuitem"
              data-menu-item={item.id}
              aria-disabled={item.disabled || undefined}
              aria-haspopup={hasChildren ? "menu" : undefined}
              aria-expanded={hasChildren ? sub?.index === index : undefined}
              aria-keyshortcuts={item.shortcut}
              disabled={item.disabled || Boolean(running)}
              onMouseEnter={() => {
                if (item.disabled) return;
                setActive(index);
                if (hoverTimer.current) window.clearTimeout(hoverTimer.current);
                if (hasChildren) {
                  hoverTimer.current = window.setTimeout(() => {
                    if (entries[index] === item) openSub(index, item);
                  }, 280);
                } else {
                  setSub(null);
                }
              }}
              onClick={() => runItem(item)}
              title={item.disabled ? item.reason : undefined}
              className={clsx(
                "mx-1 flex h-8 w-[calc(100%-8px)] items-center gap-2 rounded-md px-2 text-left transition-colors",
                item.disabled
                  ? "cursor-not-allowed opacity-40"
                  : isActive
                    ? item.danger
                      ? "cursor-pointer bg-state-danger/10 text-state-danger"
                      : "cursor-pointer bg-surface-hover text-text-strong"
                    : item.danger
                      ? "cursor-pointer text-state-danger hover:bg-state-danger/10"
                      : "cursor-pointer text-text-muted hover:bg-surface-hover hover:text-text-strong"
              )}
            >
              <span className="flex h-3.5 w-3.5 shrink-0 items-center justify-center">
                {running === item.id ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : Icon ? (
                  <Icon className="h-3.5 w-3.5" />
                ) : null}
              </span>
              <span className="min-w-0 flex-1 truncate text-[12px]">{item.label}</span>
              {right && (
                <span className="mono max-w-[104px] shrink-0 truncate text-[11px] text-text-dim">
                  {right}
                </span>
              )}
              {hasChildren && <ChevronRight className="h-3 w-3 shrink-0 text-text-dim" />}
            </button>
          );
        })}
      </motion.div>

      {sub && (
        <SubMenu
          entries={entries[sub.index] as MenuItem}
          left={sub.left}
          top={sub.top}
          active={sub.active}
          onActive={(index) => setSub({ ...sub, active: index })}
          onRun={runItem}
          onHoverOut={() => setSub(null)}
        />
      )}
    </motion.div>
  );
}

/** 取某个父项下「可点」的子项，供键盘上下移动与回车使用。 */
function subEntries(entries: MenuEntry[], index: number): MenuItem[] {
  const parent = entries[index];
  if (!parent || !isMenuItem(parent) || !parent.children) return [];
  return parent.children.filter(
    (child): child is MenuItem => isMenuItem(child) && !child.disabled
  );
}

/** 子菜单：fixed 定位（不随父面板滚动裁剪），靠近右缘时翻到左侧。 */
function SubMenu({
  entries,
  left,
  top,
  active,
  onActive,
  onRun,
  onHoverOut,
}: {
  entries: MenuItem;
  left: number;
  top: number;
  active: number;
  onActive: (index: number) => void;
  onRun: (item: MenuItem) => void;
  onHoverOut: () => void;
}) {
  const flat = (entries.children ?? []).filter(
    (child): child is MenuItem => isMenuItem(child) && !child.disabled
  );

  return (
    <div
      role="menu"
      data-context-submenu
      onMouseDown={(event) => event.stopPropagation()}
      onMouseLeave={onHoverOut}
      style={{ left, top }}
      className="fixed max-h-[320px] w-[220px] overflow-y-auto overscroll-contain rounded-lg border border-line bg-ink-900 py-1 shadow-[var(--shadow-drawer)]"
    >
      {(entries.children ?? []).map((entry, index) => {
        if (entry.kind === "separator") {
          return <div key={`sub-sep-${index}`} className="my-1 h-px bg-line" />;
        }
        if (entry.kind === "label") {
          return (
            <p
              key={`sub-label-${index}`}
              className="px-2 pb-0.5 pt-1.5 text-[11px] font-medium tracking-[0.06em] text-text-dim"
            >
              {entry.text}
            </p>
          );
        }
        const item = entry;
        const Icon = item.icon;
        const hot = flat[active]?.id === item.id;
        return (
          <button
            key={item.id}
            type="button"
            role="menuitem"
            data-menu-item={item.id}
            aria-disabled={item.disabled || undefined}
            aria-keyshortcuts={item.shortcut}
            disabled={item.disabled}
            onMouseEnter={() => {
              const at = flat.findIndex((row) => row.id === item.id);
              if (at >= 0) onActive(at);
            }}
            onClick={() => onRun(item)}
            className={clsx(
              "mx-1 flex h-8 w-[calc(100%-8px)] items-center gap-2 rounded-md px-2 text-left transition-colors",
              item.disabled
                ? "cursor-not-allowed opacity-40"
                : hot
                  ? "cursor-pointer bg-surface-hover text-text-strong"
                  : "cursor-pointer text-text-muted hover:bg-surface-hover hover:text-text-strong"
            )}
          >
            <span className="flex h-3.5 w-3.5 shrink-0 items-center justify-center">
              {Icon ? <Icon className="h-3.5 w-3.5" /> : null}
            </span>
            <span className="min-w-0 flex-1 truncate text-[12px]">{item.label}</span>
            {item.shortcut && (
              <span className="mono shrink-0 text-[11px] text-text-dim">{item.shortcut}</span>
            )}
          </button>
        );
      })}
    </div>
  );
}
