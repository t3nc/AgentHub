import { tr } from "./i18n/runtime";
import type { GatewayProtocol, RetryPolicyView } from "./types";

/**
 * 这个 agent 对外说哪种协议。
 *
 * 网关路由的 `protocol` 要跟它一致 —— 路由是「客户端说 A，上游说 B」里的 A。
 */
export function downstreamProtocol(agent: string): GatewayProtocol {
  if (agent === "codex") return "responses";
  if (agent === "opencode") return "chat_completions";
  // Claude Code 及其它未识别的一律按 Anthropic
  return "anthropic";
}

/**
 * 直连时这个组合能不能成立。
 *
 * 返回 `null` 表示可以直连；返回字符串是给用户看的原因。
 * 与后端 `agent_accepts_protocol` 保持一致 —— 后端仍会再拦一次，这里只是提前置灰。
 */
export function directBlockReason(
  agent: string,
  protocol: GatewayProtocol | null
): string | null {
  // 未声明协议：按各自历史默认直连，不拦
  if (!protocol) return null;

  if (agent === "claude") {
    if (protocol === "anthropic") return null;
    if (protocol === "chat_completions") {
      return tr("supplier.claudeChat");
    }
    return tr("supplier.claudeResponses");
  }
  if (agent === "codex") {
    if (protocol === "anthropic") {
      return tr("supplier.codexAnthropic");
    }
    return null;
  }
  // OpenCode 靠 npm 适配器表达协议，三种都能直连
  return null;
}

/** 需要网关时，自动建的路由用哪种协议（就是 agent 自己说的那种）。 */
export function routeProtocolFor(agent: string): GatewayProtocol {
  return downstreamProtocol(agent);
}

/** 自动建路由时的默认重试策略，与路由编辑器的默认值一致。 */
export function defaultRetry(): RetryPolicyView {
  return {
    max_attempts: 3,
    first_byte_ms: 120_000,
    idle_ms: 180_000,
    retry_status: [429, 500, 502, 503, 504],
  };
}

/** Claude 的四个模型家族槽位，顺序即界面顺序。 */
export const SLOT_KEYS = ["sonnet", "opus", "haiku", "fable"] as const;

export const SLOT_LABEL: Record<(typeof SLOT_KEYS)[number], string> = {
  sonnet: "Sonnet",
  opus: "Opus",
  haiku: "Haiku",
  fable: "Fable",
};

/**
 * 经网关时别名 pin 里要写的官方模型 id。
 *
 * 与后端 `ext::provider::OFFICIAL_ALIAS` 一一对应 —— 网关靠 `model_map` 把它
 * 换成真正的上游模型，这样 Claude Code 的能力检测仍按官方模型走。
 */
export const OFFICIAL_ALIAS: Record<(typeof SLOT_KEYS)[number], string> = {
  sonnet: "claude-sonnet-5",
  opus: "claude-opus-5",
  haiku: "claude-haiku-5-5",
  fable: "claude-fable-5",
};

/** 槽位对应的 `/model` 命令写法，用在提示文案里。 */
export const SLOT_COMMAND: Record<(typeof SLOT_KEYS)[number], string> = {
  sonnet: "/model sonnet",
  opus: "/model opus",
  haiku: "/model haiku",
  fable: "/model fable",
};

/** 自动建路由时的 id：稳定、可预期，重复切换不会堆一堆路由。 */
export function autoRouteId(agent: string, providerId: string): string {
  return `auto-${agent}-${providerId}`.toLowerCase().replace(/[^a-z0-9-]/g, "-");
}
