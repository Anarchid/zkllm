import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const { JsStore } = require('chronicle');

const store = JsStore.open({ path: './data/store' });
const stats = store.stats();
console.log('Store stats:', JSON.stringify(stats));

// Inference log
const log = store.getStateJson('framework/inference-log');
if (log) {
  const entries = Array.isArray(log) ? log : JSON.parse(log);
  console.log('\nInference log entries:', entries.length);
  for (const e of entries) {
    console.log('---');
    console.log('Agent:', e.agentName, '| Success:', e.success, '| Stop:', e.stopReason);
    console.log('Duration:', e.durationMs, 'ms | Tokens:', JSON.stringify(e.tokenUsage));
    if (e.request?.note) console.log('Request:', e.request.note);
    if (e.request?.blobId) console.log('Request: [blob', e.request.blobId, ']');
    if (e.response?.blobId) console.log('Response: [blob', e.response.blobId, ']');
  }
} else {
  console.log('No inference log found');
}

// Process log
const plog = store.getStateJson('framework/process-log');
if (plog) {
  const entries = Array.isArray(plog) ? plog : JSON.parse(plog);
  console.log('\nProcess log entries:', entries.length);
  for (const e of entries) {
    const evt = e.processEvent || {};
    console.log('---');
    console.log('Type:', evt.type, '| Source:', evt.source);
    if (evt.content) console.log('Content:', String(evt.content).substring(0, 150));
    if (evt.triggerInference !== undefined) console.log('triggerInference:', evt.triggerInference);
    for (const r of (e.responses || [])) {
      if (r.requestInference !== undefined) console.log('  module requestInference:', r.requestInference);
      if (r.addMessages) console.log('  addMessages:', r.addMessages.length);
    }
  }
} else {
  console.log('No process log found');
}

// List all state IDs
console.log('\nAll state IDs:');
const stateIds = store.listStates();
if (stateIds) {
  for (const id of stateIds) {
    console.log(' ', id);
  }
}

// Dump messages state
console.log('\n=== MESSAGES STATE ===');
const msgs = store.getStateJson('messages');
if (msgs) {
  const items = Array.isArray(msgs) ? msgs : [msgs];
  console.log('Message count:', items.length);
  for (let i = 0; i < items.length; i++) {
    const m = items[i];
    const participant = m.participant || m.role || '?';
    const contentPreview = JSON.stringify(m.content || m).substring(0, 200);
    console.log(`  [${i}] ${participant}: ${contentPreview}`);
  }
}

// Dump context state
console.log('\n=== CONTEXT STATE ===');
const ctx = store.getStateJson('agents/commander/context');
if (ctx) {
  console.log('Type:', typeof ctx, Array.isArray(ctx) ? `(array len ${ctx.length})` : '');
  console.log(JSON.stringify(ctx, null, 2).substring(0, 2000));
}

// Count tokens in system prompt + tools to estimate overhead
const sysPromptLen = msgs ? 0 : 0; // Can't measure here, but ~5000 tokens estimated
console.log('\nToken analysis:');
console.log('  Inference 1 input tokens: 5957');
console.log('  Inference 2 input tokens: 5978');
console.log('  Difference: 21 tokens');
console.log('  Messages in store: 12');
const msgItems = Array.isArray(msgs) ? msgs : [];
let totalChars = 0;
for (const m of msgItems) {
  const txt = JSON.stringify(m.content || '');
  totalChars += txt.length;
}
console.log('  Total message content chars:', totalChars);
console.log('  Est tokens for all messages:', Math.round(totalChars / 4));

// Check response blobs
console.log('\n=== RESPONSE BLOBS ===');
for (const blobId of ['2c806487beb4767da1cc5f8eeb4f03af8ff332076e16ca1b0660048eb0b6e3ab', '9efcea796a9e72a0b9eb42f42bd724d88d10a4ac3f1f1b75135b7016742fdc9f']) {
  const blob = store.getBlob(blobId);
  if (blob) {
    const text = Buffer.from(blob).toString('utf8');
    const parsed = JSON.parse(text);
    // Show content blocks
    if (parsed.content) {
      console.log(`\nBlob ${blobId.substring(0, 8)}... content blocks:`);
      for (const block of parsed.content) {
        if (block.type === 'text') {
          console.log(`  text: "${block.text.substring(0, 150)}"`);
        } else if (block.type === 'tool_use') {
          console.log(`  tool_use: ${block.name} (${block.id})`);
        } else {
          console.log(`  ${block.type}`);
        }
      }
    }
    if (parsed.stopReason) console.log('  stopReason:', parsed.stopReason);
  }
}

store.close();
