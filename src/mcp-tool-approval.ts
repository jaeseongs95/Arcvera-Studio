// A display/response contract shared by the two existing GUI approval surfaces.
// Native independently verifies the original pending request before any write.
export const MCP_TOOL_APPROVAL_METHOD = 'mcpServer/elicitation/request';
export const MCP_TOOL_APPROVAL_REVIEW_BYTES = 16 * 1024;
export const MCP_TOOL_APPROVAL_ACTIONS = ['accept', 'decline', 'cancel'] as const;
export type McpToolApprovalAction = typeof MCP_TOOL_APPROVAL_ACTIONS[number];
export interface McpToolApproval {
  origin: 'core'; threadId: string; turnId: string; serverName: string; toolName: string; arguments: Record<string, unknown>;
}
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const bytes = (value: string) => new TextEncoder().encode(value).byteLength;
const id = (value: unknown): value is string => typeof value === 'string' && bytes(value) > 0 && bytes(value) <= 256 && !/[\u0000-\u001f\u007f-\u009f]/u.test(value);
function reviewable(value: unknown, depth = 0): boolean {
  if (depth > 64) return false;
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  // safety ceiling: preserve finite fractions, but reject unsafe magnitudes and -0 before stringify.
  if (typeof value === 'number') return Number.isFinite(value) && Math.abs(value) <= Number.MAX_SAFE_INTEGER && !Object.is(value, -0);
  if (Array.isArray(value)) return value.every(item => reviewable(item, depth + 1));
  return object(value) && value.__detail !== true && value.truncated !== true && value.hasMore !== true
    && Object.values(value).every(item => reviewable(item, depth + 1));
}
export function readMcpToolApproval(value: unknown): McpToolApproval | undefined {
  if (!object(value) || value.origin !== 'core' || !id(value.threadId) || !id(value.turnId) || !id(value.serverName) || !id(value.toolName)
    || !object(value.arguments) || !reviewable(value.arguments)) return;
  const serialized = JSON.stringify(value.arguments);
  if (typeof serialized !== 'string' || bytes(serialized) > MCP_TOOL_APPROVAL_REVIEW_BYTES) return;
  return { origin: 'core', threadId: value.threadId, turnId: value.turnId, serverName: value.serverName, toolName: value.toolName, arguments: value.arguments };
}
export function mcpToolApprovalFromRequest(method: string, params: unknown): McpToolApproval | undefined {
  if (method !== MCP_TOOL_APPROVAL_METHOD || !object(params) || params.origin !== 'core' || params.mode !== 'form'
    || !object(params._meta) || params._meta.codex_approval_kind !== 'mcp_tool_call') return;
  return readMcpToolApproval({ origin: params.origin, threadId: params.threadId, turnId: params.turnId, serverName: params.serverName,
    toolName: params._meta.tool_name, arguments: params._meta.tool_params });
}
export function mcpToolApprovalResponse(action: McpToolApprovalAction) {
  if (!MCP_TOOL_APPROVAL_ACTIONS.includes(action)) throw new Error('Unsupported MCP tool approval action');
  return { action, content: null, _meta: null };
}
