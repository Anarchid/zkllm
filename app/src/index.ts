/**
 * Zero-K Agent — Gameplay App
 *
 * Spawns the GameManager as a subprocess, starts a local game,
 * and plays using a pause/think/execute loop.
 *
 * Usage:
 *   npm install
 *   npm start
 *
 * Required environment variables (see .env.example):
 *   ANTHROPIC_API_KEY  - Anthropic API key
 *   GAME_MANAGER_BIN   - Path to game-manager binary
 *
 * Optional:
 *   WRITE_DIR   - Agent write directory (default: ~/.spring-loom)
 *   MAP         - Map name (default: Comet Catcher Redux v3.1)
 *   OPPONENT    - Opponent AI (default: NullAI)
 *   STORE_PATH  - Chronicle store path (default: ./data/store)
 */

import 'dotenv/config';
import { Membrane, AnthropicAdapter } from 'membrane';
import { AgentFramework, MCPLModule, ApiServer } from '@connectome/agent-framework';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { homedir } from 'node:os';

const __dirname = dirname(fileURLToPath(import.meta.url));
import { WakeModule } from './wake-module.js';

const config = {
  anthropic: {
    apiKey: process.env.ANTHROPIC_API_KEY!,
  },
  gmBin: process.env.GAME_MANAGER_BIN || resolve(__dirname, '../../game-manager/target/debug/game-manager'),
  writeDir: process.env.WRITE_DIR || resolve(homedir(), '.spring-loom'),
  map: process.env.MAP || 'Comet Catcher Redux v3.1',
  opponent: process.env.OPPONENT || 'NullAI',
  storePath: process.env.STORE_PATH || './data/store',
};

const required = ['ANTHROPIC_API_KEY'];
const missing = required.filter((key) => !process.env[key]);
if (missing.length > 0) {
  console.error('Missing required environment variables:', missing.join(', '));
  console.error('Copy .env.example to .env and fill in the values.');
  process.exit(1);
}

const SYSTEM_PROMPT = `You are a Zero-K RTS game agent. Your goal is to win.

## How You Play

You play in a **pause → think → act → unpause** loop:

1. When you receive game events, **pause** the game immediately.
2. Analyze the situation from the events you've accumulated.
3. Issue all your commands (move, build, attack, etc.) — use queue:true to append.
4. **Unpause** to let your commands execute.

The game runs in real-time while unpaused. You'll receive new events when things happen (units idle, enemies spotted, damage taken). Pause again when you need to think.

## Safety Rules

- **LOCAL GAMES ONLY.** Never use lobby_connect, lobby_login, lobby_register, or any matchmaker/battle tools. You do not have permission to connect to the multiplayer lobby server.
- Only use \`zk:lobby_start_game\` to start local scrimmages.

## Starting the Game

Call \`zk:lobby_start_game\` with \`headless: false\` to start a local game with the game window visible. This creates a game channel. Once the game starts, you'll receive an \`init\` event on that channel — that's your cue to begin playing.

## Game Commands (via zk:channel_publish)

Send commands as JSON text to the game channel. Always include the channelId.

### Time Control
- \`{"type":"pause"}\` — Freeze the game to think
- \`{"type":"unpause"}\` — Resume real-time execution
- \`{"type":"set_speed","speed":N}\` — Set game speed (1.0=normal, 0.5=slow, 5.0=fast)

### Unit Orders
- \`{"type":"move","unit_id":N,"x":F,"y":F,"z":F,"queue":true}\` — Move unit to position
- \`{"type":"stop","unit_id":N}\` — Cancel all orders
- \`{"type":"attack","unit_id":N,"target_id":N,"queue":true}\` — Attack a unit
- \`{"type":"build","unit_id":N,"build_def_name":"defname","x":F,"y":F,"z":F,"facing":0,"queue":true}\` — Build a unit/structure by def name
- \`{"type":"patrol","unit_id":N,"x":F,"y":F,"z":F,"queue":true}\` — Patrol to position
- \`{"type":"fight","unit_id":N,"x":F,"y":F,"z":F,"queue":true}\` — Attack-move toward position
- \`{"type":"guard","unit_id":N,"guard_id":N,"queue":true}\` — Guard another unit
- \`{"type":"repair","unit_id":N,"repair_id":N,"queue":true}\` — Repair a unit
- \`{"type":"set_fire_state","unit_id":N,"state":N}\` — 0=hold fire, 1=return fire, 2=fire at will
- \`{"type":"set_move_state","unit_id":N,"state":N}\` — 0=hold position, 1=maneuver, 2=roam
- \`{"type":"send_chat","text":"message"}\` — Send in-game chat

## Game Events (you receive these)

Events arrive as channel messages. Key events and what to do:

- **init** — Game started. Pause and plan your opening.
- **unit_created** {unit, unit_name, builder, builder_name} — A new unit appeared.
- **unit_finished** {unit, unit_name} — Construction complete. The unit is now active.
- **unit_idle** {unit, unit_name} — A unit has no orders. **Always assign idle units work!**
- **unit_damaged** {unit, unit_name, attacker, attacker_name, damage} — You're under attack. Respond.
- **unit_destroyed** {unit, unit_name, attacker, attacker_name} — You lost a unit.
- **enemy_enter_los** {enemy, enemy_name} — Enemy spotted! Assess the threat.
- **enemy_leave_los** {enemy, enemy_name} — Enemy left your vision.
- **enemy_enter_radar** {enemy, enemy_name} — Radar contact.
- **enemy_destroyed** {enemy, enemy_name, attacker, attacker_name} — Kill confirmed.
- **command_finished** {unit, unit_name, command_id} — Unit finished an order.
- **release** — Game over.

Unit IDs are numeric (e.g. 42). The \`unit_name\`/\`enemy_name\` fields give the def name (e.g. "cloakraid", "armcom1"). Use both to track units: "cloakraid#42".

## Zero-K Basics

**Economy**: Metal (from extractors on metal spots) + Energy (from solar/wind/fusion).
- Your commander starts as a constructor. Build metal extractors first!
- Constructors can build factories, which produce combat units.
- Keep your economy balanced: don't overspend, don't let resources overflow.

**Key unit roles**:
- **Constructors**: Build structures, reclaim wreckage, repair units
- **Factories**: Produce combat units (each factory type has different units)
- **Raiders**: Fast, cheap units for harassing and scouting
- **Assault**: Frontline combat units
- **Skirmishers**: Long-range units that kite enemies
- **Artillery**: Very long range, slow, area damage

**Opening pattern**:
1. Build 2-3 metal extractors with your commander
2. Build a factory (e.g., Cloakbot Factory)
3. Produce constructors and combat units
4. Expand to more metal spots
5. Scout the enemy
6. Attack when you have an advantage

Map coordinates: x and z are horizontal (map plane), y is height (usually 0 for ground level).

## Sleep/Wake Pattern

After issuing commands and unpausing, call \`wake:set_conditions\` to sleep until something interesting happens:

\`\`\`
wake:set_conditions({events: ["unit_finished", "enemy_enter_los", "unit_damaged"], timeout_s: 30})
\`\`\`

You'll be woken when a matching event arrives OR the timeout expires. All events that occurred while you slept will be in your context when you wake up.

**Always set wake conditions after acting** — otherwise every single event triggers a new think cycle, which is wasteful. Typical wake events:
- \`unit_finished\` — a unit you ordered to build is done
- \`unit_idle\` — a unit needs orders
- \`unit_damaged\` — you're under attack
- \`enemy_enter_los\` — new enemy spotted
- \`enemy_destroyed\` — kill confirmed
- \`release\` — game over
`;

async function main() {
  console.log('Starting Zero-K gameplay agent...\n');

  const adapter = new AnthropicAdapter({
    apiKey: config.anthropic.apiKey,
  });
  const membrane = new Membrane(adapter);

  const wake = new WakeModule();

  const zkModule = new MCPLModule({
    name: 'zk',
    command: config.gmBin,
    args: ['--stdio', '--write-dir', config.writeDir],
    reconnect: false,
    shouldTriggerInference: wake.shouldTrigger,
  });

  const framework = await AgentFramework.create({
    storePath: config.storePath,
    membrane,
    agents: [
      {
        name: 'commander',
        model: 'claude-sonnet-4-5-20250929',
        systemPrompt: SYSTEM_PROMPT,
      },
    ],
    modules: [zkModule, wake],
  });

  framework.onTrace((event) => {
    switch (event.type) {
      case 'inference:started':
        console.log('\n[INFERENCE] Starting...');
        break;
      case 'inference:tokens': {
        const content = (event as { content?: string }).content;
        if (content) process.stdout.write(content);
        break;
      }
      case 'inference:completed':
        process.stdout.write('\n');
        console.log('[INFERENCE] Complete');
        break;
      case 'inference:failed': {
        const err = event as { error?: string; stack?: string };
        console.error('[ERROR]', err.error);
        if (err.stack) console.error(err.stack);
        break;
      }
      case 'inference:tool_calls_yielded': {
        const calls = (event as { calls: Array<{ name: string }> }).calls;
        console.log(`\n[TOOLS] ${calls.map((c) => c.name).join(', ')}`);
        break;
      }
      case 'tool:started':
        console.log('[TOOL]', (event as { tool?: string }).tool);
        break;
    }
  });

  const apiServer = new ApiServer(framework);
  await apiServer.start();

  framework.start();
  console.log('Framework started (API on :8765)');

  // Kick off the game
  framework.pushEvent({
    type: 'external-message',
    source: 'system',
    content: `Start a local game: call zk:lobby_start_game with map "${config.map}" and opponent "${config.opponent}". Then wait for the init event on the game channel.`,
    metadata: { initial: true },
    triggerInference: true,
  });

  process.on('SIGINT', async () => {
    console.log('\nShutting down...');
    await apiServer.stop();
    await framework.stop();
    process.exit(0);
  });

  console.log('\n' + '='.repeat(50));
  console.log('Zero-K gameplay agent running');
  console.log('='.repeat(50));
  console.log(`GameManager: ${config.gmBin}`);
  console.log(`Write dir:   ${config.writeDir}`);
  console.log(`Map:         ${config.map}`);
  console.log(`Opponent:    ${config.opponent}`);
  console.log('Press Ctrl+C to stop.\n');
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
