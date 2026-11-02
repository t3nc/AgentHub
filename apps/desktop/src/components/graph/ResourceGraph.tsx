import { useCallback, useMemo, useRef } from "react";
import {
  Controls,
  Position,
  ReactFlow,
  type Edge,
  type Node,
  type NodeMouseHandler,
  type ReactFlowInstance,
} from "@xyflow/react";
import { STORE_NODE_ID, type GraphModel, type Health } from "../../lib/graph";
import { tr } from "../../lib/i18n/runtime";
import { useSelection } from "../../lib/selection";
import {
  AGENT_H,
  AGENT_W,
  STORE_H,
  STORE_W,
  nodeTypes,
  type AgentNodeData,
  type StoreNodeData,
} from "./nodes";

/** 图谱里只有这两种节点；ReactFlowInstance 是泛型的，得给它确切类型 */
type GraphNode = Node<StoreNodeData> | Node<AgentNodeData>;
type GraphFlow = ReactFlowInstance<GraphNode, Edge>;

type Anchor = "left" | "right" | "top";

interface Slot {
  x: number;
  y: number;
  anchor: Anchor;
}

/**
 * 左 2 / 右 2 / 下 1；超过 5 个自定义平台时环形排布。
 *
 * 槽位是压着「固定高 420px 的面板」调的：整图约 672×350，
 * fitView 后缩放接近 1，节点里的 12px 文字不会被缩成 7px。
 */
const SLOTS: Slot[] = [
  { x: -336, y: -112, anchor: "right" },
  { x: -336, y: 30, anchor: "right" },
  { x: 140, y: -112, anchor: "left" },
  { x: 140, y: 30, anchor: "left" },
  { x: -98, y: 150, anchor: "top" },
];

function slotFor(index: number, total: number): Slot {
  if (total <= SLOTS.length) return SLOTS[index];
  const radius = 330;
  const angle = (-90 + (360 / total) * index) * (Math.PI / 180);
  const anchor: Anchor =
    Math.cos(angle) < -0.2 ? "right" : Math.cos(angle) > 0.2 ? "left" : "top";
  return {
    x: Math.cos(angle) * radius - AGENT_W / 2,
    y: Math.sin(angle) * radius - AGENT_H / 2,
    anchor,
  };
}

const STROKE: Record<Health, string> = {
  ok: "rgb(var(--ok))",
  warn: "rgb(var(--warn))",
  danger: "rgb(var(--danger))",
};

const STORE_HANDLE: Record<Anchor, string> = {
  right: "left",
  left: "right",
  top: "bottom",
};

const TARGET_POSITION: Record<Anchor, Position> = {
  right: Position.Right,
  left: Position.Left,
  top: Position.Top,
};

export function ResourceGraph({
  model,
  focus = "all",
  onNodeContextMenu,
}: {
  model: GraphModel;
  /** `issues` 时把健康的 agent 压暗，只突出需要处理的那几条边 */
  focus?: "all" | "issues";
  /** 右键节点：交给页面弹上下文菜单（id 为 store 或 agent id） */
  onNodeContextMenu?: (event: React.MouseEvent, nodeId: string) => void;
}) {
  const { selection, select, clear } = useSelection();
  /** 视口只在这里操作：点节点居中、点空白回全图。不引入 Provider，实例从 onInit 拿。 */
  const flowRef = useRef<GraphFlow | null>(null);

  const slots = useMemo(
    () => model.agents.map((_, index) => slotFor(index, model.agents.length)),
    [model.agents]
  );

  const nodes = useMemo(() => {
    const storeNode: Node<StoreNodeData> = {
      id: STORE_NODE_ID,
      type: "store",
      position: { x: -STORE_W / 2, y: -STORE_H / 2 },
      // 显式尺寸：自定义节点的内容尺寸要等 CSS 生效才准，早测量会让边锚点与 fitView 算错
      width: STORE_W,
      height: STORE_H,
      draggable: false,
      selectable: true,
      data: {
        path: model.store.path,
        skillCount: model.store.skillCount,
        sharedSkills: model.store.sharedSkills,
        sizeBytes: model.store.sizeBytes,
        health: model.store.health,
      },
    };

    const agentNodes: Node<AgentNodeData>[] = model.agents.map((agent, index) => {
      const slot = slots[index] ?? SLOTS[0];
      const dimmed = focus === "issues" && agent.health !== "danger";
      return {
        id: agent.id,
        type: "agent",
        position: { x: slot.x, y: slot.y },
        width: AGENT_W,
        height: AGENT_H,
        draggable: false,
        selectable: true,
        targetPosition: TARGET_POSITION[slot.anchor],
        // 压暗而不是隐藏：保留「本机还有哪些 agent」的空间感
        style: dimmed ? { opacity: 0.32 } : undefined,
        data: { agent },
      };
    });

    return [storeNode, ...agentNodes];
  }, [focus, model, slots]);

  const edges = useMemo<Edge[]>(
    () =>
      model.agents.map((agent, index) => {
        const slot = slots[index] ?? SLOTS[0];
        const label =
          agent.broken > 0
            ? tr("graph.broken", { count: agent.broken })
            : agent.copies > 0
              ? tr("graph.copies", { count: agent.copies })
              : tr("graph.linked", { count: agent.linked });
        const dimmed = focus === "issues" && agent.health !== "danger";
        return {
          id: `edge-${agent.id}`,
          source: STORE_NODE_ID,
          sourceHandle: STORE_HANDLE[slot.anchor],
          target: agent.id,
          type: "bezier",
          label,
          // 高 DPI 屏上 1px 线会被抗锯齿吃掉，给到 1.6 更稳
          style: {
            stroke: STROKE[agent.health],
            strokeWidth: 1.6,
            strokeOpacity: dimmed ? 0.16 : 0.9,
          },
          labelStyle: { fill: "rgb(var(--fg-dim))", fontSize: 11, opacity: dimmed ? 0.3 : 1 },
          labelBgStyle: { fill: "rgb(var(--surface-2))", fillOpacity: dimmed ? 0.3 : 1 },
          labelBgPadding: [4, 2] as [number, number],
          labelBgBorderRadius: 3,
        };
      }),
    [focus, model.agents, slots]
  );

  /** 把某个节点移到视口中心。已经放大过就不缩回去——用户可能正盯着细节读。 */
  const centerOn = useCallback((node: Node) => {
    const flow = flowRef.current;
    if (!flow) return;
    const width = node.width ?? node.measured?.width ?? 0;
    const height = node.height ?? node.measured?.height ?? 0;
    void flow.setCenter(node.position.x + width / 2, node.position.y + height / 2, {
      zoom: Math.max(flow.getZoom(), 1.15),
      duration: 450,
    });
  }, []);

  const onNodeClick = useCallback<NodeMouseHandler>(
    (_event, node) => {
      if (node.id === STORE_NODE_ID) {
        select({ kind: "store", id: STORE_NODE_ID });
      } else {
        select({ kind: "agent", id: node.id });
      }
      // 选中与视口一起动，别只动一半
      centerOn(node);
    },
    [centerOn, select]
  );

  const onPaneClick = useCallback(() => {
    clear();
    // 点空白 = 回到全图，和左下角 Controls 的 fitView 同一个落点
    void flowRef.current?.fitView({ padding: 0.08, duration: 450 });
  }, [clear]);

  const onNodeContextMenuHandler = useCallback<NodeMouseHandler>(
    (event, node) => {
      if (!onNodeContextMenu) return;
      event.preventDefault();
      onNodeContextMenu(event, node.id);
    },
    [onNodeContextMenu]
  );

  return (
    <div className="absolute inset-0" data-graph>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodeClick={onNodeClick}
        onNodeContextMenu={onNodeContextMenuHandler}
        onPaneClick={onPaneClick}
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable
        // 地图式交互：拖拽平移 + 滚轮缩放。代价是面板内的滚轮不再滚页面
        // （preventScrolling），所以右下角常驻一行提示，别让人以为滚轮坏了。
        panOnDrag
        zoomOnScroll
        zoomOnPinch
        preventScrolling
        zoomOnDoubleClick={false}
        // 1.6 放大看不清节点里的 12px 正文，放宽到 2；下限留给「图比面板大」的情况
        minZoom={0.4}
        maxZoom={2}
        fitView
        fitViewOptions={{ padding: 0.08 }}
        proOptions={{ hideAttribution: true }}
        // 背景点阵用 CSS 画：Background 组件的 color 走 SVG 属性，解析不了 var()
        style={{
          backgroundImage: "radial-gradient(rgb(var(--grid-line)) 1px, transparent 1px)",
          backgroundSize: "20px 20px",
        }}
        onInit={(instance) => {
          flowRef.current = instance;
          // 首帧量到的节点尺寸可能还没算上 CSS；初始化后补两次适配（别删，删了首屏会缩歪）
          window.setTimeout(() => void instance.fitView({ padding: 0.08 }), 120);
          window.setTimeout(() => void instance.fitView({ padding: 0.08 }), 420);
        }}
      >
        <Controls
          showInteractive={false}
          position="bottom-left"
          className="!bottom-2.5 !left-2.5 !shadow-panel"
        />
      </ReactFlow>

      <div className="pointer-events-none absolute left-2.5 top-2.5 z-10 flex items-center gap-2.5 rounded-md border border-line bg-ink-900/90 px-2 py-1 text-[11px] text-text-dim">
        <span className="flex items-center gap-1">
          <span className="h-1.5 w-1.5 rounded-full bg-state-ok" />
          {tr("graph.legend.linked")}
        </span>
        <span className="flex items-center gap-1">
          <span className="h-1.5 w-1.5 rounded-full bg-state-warn" />
          {tr("graph.legend.pending")}
        </span>
        <span className="flex items-center gap-1">
          <span className="h-1.5 w-1.5 rounded-full bg-state-danger" />
          {tr("graph.legend.broken")}
        </span>
      </div>

      {/* 面板里的滚轮被缩放占用，这行提示就是「为什么页面没滚」的答案 */}
      <div className="pointer-events-none absolute bottom-2.5 right-2.5 z-10 rounded-md border border-line bg-ink-900/85 px-2 py-1 text-[11px] text-text-dim">
        {tr("graph.hint")}
      </div>

      {model.agents.length === 0 && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <p className="text-[12px] text-text-dim">{tr("graph.empty")}</p>
        </div>
      )}
    </div>
  );
}
