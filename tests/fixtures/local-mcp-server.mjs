// Test-only local MCP fixture. It never reads credentials or launches commands.
import { createInterface } from 'node:readline';
const tool = process.env.DESKTOP_MCP_PROBE_TOOL ?? 'desktop_local_probe_v1';
for await (const line of createInterface({ input: process.stdin })) {
  let message; try { message = JSON.parse(line); } catch { continue; }
  if (message.id === undefined) continue;
  let result;
  if (message.method === 'initialize') result = { protocolVersion: message.params.protocolVersion, capabilities: { tools: { listChanged: false } }, serverInfo: { name: 'desktop-local-test-fixture', version: '1' } };
  else if (message.method === 'ping') result = {};
  else if (message.method === 'tools/list') result = { tools: [{ name: tool, description: 'Synthetic local integration fixture', inputSchema: { type: 'object', properties: {}, additionalProperties: false } }] };
  else if (message.method === 'tools/call' && message.params?.name === tool) result = { content: [{ type: 'text', text: 'desktop-local-fixture' }] };
  else { process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: message.id, error: { code: -32601, message: 'Unsupported fixture method' } }) + '\n'); continue; }
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: message.id, result }) + '\n');
}
