/**
 * Zero-K Lobby Agent
 *
 * Connects to the Zero-K game lobby via the GameManager MCPL server.
 * The agent can interact with lobby chat, list battles and users,
 * and join/leave channels and battles.
 *
 * Usage:
 *   npm install
 *   npm start
 *
 * Required environment variables (see .env.example):
 *   ANTHROPIC_API_KEY - Anthropic API key
 *   ZK_USERNAME       - Zero-K lobby username
 *   ZK_PASSWORD       - Zero-K lobby password
 *
 * Optional:
 *   GAME_MANAGER_HOST - GameManager host (default: localhost)
 *   GAME_MANAGER_PORT - GameManager port (default: 9800)
 *   STORE_PATH        - Chronicle store path (default: ./data/store)
 */

import 'dotenv/config';
import { Membrane, AnthropicAdapter } from 'membrane';
import { AgentFramework, MCPLModule } from '@connectome/agent-framework';

const config = {
  anthropic: {
    apiKey: process.env.ANTHROPIC_API_KEY!,
  },
  zk: {
    username: process.env.ZK_USERNAME!,
    password: process.env.ZK_PASSWORD!,
  },
  gameManager: {
    host: process.env.GAME_MANAGER_HOST || 'localhost',
    port: Number(process.env.GAME_MANAGER_PORT) || 9800,
  },
  storePath: process.env.STORE_PATH || './data/store',
};

const required = ['ANTHROPIC_API_KEY', 'ZK_USERNAME', 'ZK_PASSWORD'];
const missing = required.filter((key) => !process.env[key]);
if (missing.length > 0) {
  console.error('Missing required environment variables:', missing.join(', '));
  console.error('Copy .env.example to .env and fill in the values.');
  process.exit(1);
}

const SYSTEM_PROMPT = `You are a Zero-K RTS game agent connected to the Zero-K lobby server. You can interact with the lobby and play games via the GameManager.

## Lobby Tools (zk:)
- zk:lobby_connect — Connect to the Zero-K lobby server
- zk:lobby_login — Authenticate with username and password
- zk:lobby_disconnect — Disconnect from the lobby
- zk:lobby_say — Send a chat message (target: channel name or username, place: 0=channel, 4=user)
- zk:lobby_join_channel / zk:lobby_leave_channel — Join/leave chat channels
- zk:lobby_list_battles — List open battles
- zk:lobby_list_users — List online users
- zk:lobby_join_battle / zk:lobby_leave_battle — Join/leave a battle room

## Game Channel Tools (zk:)
- zk:channel_open — Start a new game instance. Params: address: { map, game }
- zk:channel_close — Stop a running game. Params: channelId
- zk:channel_list — List active game instances
- zk:channel_publish — Send a command to a running game. Params: channelId, content (JSON command)

## Game Commands (sent via channel_publish)
Commands are JSON objects with a "type" field. Send as the text content of channel_publish.
- Move: {"type":"move","unit_id":N,"x":F,"y":F,"z":F,"queue":false}
- Stop: {"type":"stop","unit_id":N}
- Attack: {"type":"attack","unit_id":N,"target_id":N,"queue":false}
- Build: {"type":"build","unit_id":N,"build_def_id":N,"x":F,"y":F,"z":F,"facing":0,"queue":false}
- Patrol: {"type":"patrol","unit_id":N,"x":F,"y":F,"z":F,"queue":false}
- Fight: {"type":"fight","unit_id":N,"x":F,"y":F,"z":F,"queue":false}
- Guard: {"type":"guard","unit_id":N,"guard_id":N,"queue":false}
- Repair: {"type":"repair","unit_id":N,"repair_id":N,"queue":false}
- Set fire state: {"type":"set_fire_state","unit_id":N,"state":N} (0=hold, 1=return, 2=fire at will)
- Set move state: {"type":"set_move_state","unit_id":N,"state":N} (0=hold pos, 1=maneuver, 2=roam)
- Send chat: {"type":"send_chat","text":"message"}
Use queue:true to append to command queue instead of replacing current command.

## Game Events (received via channels/incoming)
You will receive game events as channel messages with JSON content:
- init — Game started (frame, saved_game)
- update — Periodic tick (frame number)
- unit_created, unit_finished, unit_destroyed — Your unit lifecycle
- unit_idle — A unit has nothing to do (assign it work!)
- unit_damaged — Your unit taking damage
- enemy_enter_los / enemy_leave_los — Enemy visibility changes
- enemy_enter_radar / enemy_leave_radar — Radar contacts
- enemy_destroyed — Confirmed kill
- command_finished — A unit completed its command
- message — In-game chat
- release — Game ended

## Strategy Basics
Zero-K is a real-time strategy game. Key principles:
- Build economy first: constructors build metal extractors on metal spots and energy generators
- Expand: claim more metal spots to increase income
- Scout: know what your opponent is building
- Counter: adapt your unit composition to counter the enemy's
- Don't let units idle: always assign commands to idle units
- Use terrain: high ground, chokepoints, and cover matter

## Lobby Credentials
Username: ${config.zk.username}
Password: ${config.zk.password}

## Behavior
- On startup: connect to the lobby and log in, then join the "main" chat channel
- In lobby: respond to chat naturally, help players, discuss strategy
- Push events from the lobby arrive as external messages (chat, battle updates, etc.)
- Game events from running instances arrive as channel messages
- When playing a game, react to events promptly — issue commands to idle units, respond to threats`;

async function main() {
  console.log('Starting Zero-K lobby agent...\n');

  const adapter = new AnthropicAdapter({
    apiKey: config.anthropic.apiKey,
  });
  const membrane = new Membrane(adapter);

  const zkModule = new MCPLModule({
    name: 'zk',
    host: config.gameManager.host,
    port: config.gameManager.port,
    reconnect: true,
    reconnectInterval: 5000,
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
    modules: [zkModule],
  });

  framework.onTrace((event) => {
    switch (event.type) {
      case 'inference:started':
        console.log('[INFERENCE] Starting...');
        break;
      case 'inference:completed':
        console.log('[INFERENCE] Complete');
        break;
      case 'inference:failed':
        console.error('[ERROR]', event.error);
        break;
      case 'tool:started':
        console.log('[TOOL]', event.tool);
        break;
    }
  });

  framework.start();
  console.log('Framework started');

  // Send initial instruction to connect and log in
  framework.pushEvent({
    type: 'external-message',
    source: 'system',
    content: 'Connect to the Zero-K lobby and log in. Then join the "main" chat channel.',
    metadata: { initial: true },
    triggerInference: true,
  });

  process.on('SIGINT', async () => {
    console.log('\nShutting down...');
    await framework.stop();
    process.exit(0);
  });

  console.log('\n' + '='.repeat(50));
  console.log('Zero-K lobby agent running');
  console.log('='.repeat(50));
  console.log(`GameManager: ${config.gameManager.host}:${config.gameManager.port}`);
  console.log('Press Ctrl+C to stop.\n');
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
