import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type SelectionKind = "agent" | "skill" | "store";

/**
 * 只存「标识」不存对象：数据刷新后不会拿着过期引用渲染。
 * - agent: AgentInstall.id
 * - skill: SkillRecord.path（同名技能在不同 agent 下是不同条目）
 * - store: 固定为 "store"
 */
export interface Selection {
  kind: SelectionKind;
  id: string;
}

interface SelectionContextValue {
  selection: Selection | null;
  select: (next: Selection | null) => void;
  clear: () => void;
  isSelected: (kind: SelectionKind, id: string) => boolean;
}

const SelectionContext = createContext<SelectionContextValue | null>(null);

export function SelectionProvider({ children }: { children: ReactNode }) {
  const [selection, setSelection] = useState<Selection | null>(null);

  const select = useCallback((next: Selection | null) => {
    setSelection((prev) => {
      // 重复点击同一个对象视为取消选择（画布上点同一节点的手感）
      if (prev && next && prev.kind === next.kind && prev.id === next.id) {
        return null;
      }
      return next;
    });
  }, []);

  const clear = useCallback(() => setSelection(null), []);

  const isSelected = useCallback(
    (kind: SelectionKind, id: string) =>
      selection?.kind === kind && selection.id === id,
    [selection]
  );

  const value = useMemo<SelectionContextValue>(
    () => ({ selection, select, clear, isSelected }),
    [selection, select, clear, isSelected]
  );

  return <SelectionContext.Provider value={value}>{children}</SelectionContext.Provider>;
}

export function useSelection(): SelectionContextValue {
  const ctx = useContext(SelectionContext);
  if (!ctx) throw new Error("useSelection 必须在 SelectionProvider 内使用");
  return ctx;
}
