# Zero-K Agent

An LLM plays a real-time strategy game.

This project connects Claude (or any MCP-compatible LLM) to [Zero-K](https://zero-k.info), a free open-source RTS built on the [Recoil](https://github.com/beyond-all-reason/spring) engine. The agent receives game events (unit positions, combat, economy) and issues commands (move, attack, build) — all through [MCPL](https://github.com/anima-research/mcpl), our extension of the [Model Context Protocol](https://modelcontextprotocol.io) that adds bidirectional channels, push events, and reversible feature sets.

```
┌──────────┐    MCPL      ┌──────────────┐   Unix IPC   ┌────────────┐   C FFI   ┌────────┐
│  Claude   │◄───stdio────►│ GameManager  │◄────JSON────►│ SAI Bridge │◄─────────►│ Engine │
│  (LLM)   │  JSON-RPC    │   (Rust)     │   socket     │  (.so)     │  vtable   │(Recoil)│
└──────────┘              └──────────────┘              └────────────┘           └────────┘
```

The GameManager is an [MCPL](https://github.com/anima-research/mcpl) server — backward-compatible with plain MCP clients, but unlocking bidirectional game channels for those that support it. Claude connects over stdio, calls tools to start games and join lobbies, and receives a live stream of game events through MCPL channels. Commands flow back the same way: Claude publishes JSON to the game channel, GameManager forwards it over IPC, and the SAI bridge translates it into engine C API calls.

### Why MCPL?

Standard MCP is request-response: the client asks, the server answers. That's fine for tools, but an RTS game is a firehose of events — units spawning, enemies appearing, frames ticking. MCPL adds **channels** (persistent bidirectional streams), **push events** (server-initiated messages), and **feature sets** with rollback semantics. The game channel carries events downstream and commands upstream, all within the same protocol session. See the [MCPL spec](https://github.com/anima-research/mcpl) for details.

## Components

### GameManager (`game-manager/`)

Rust binary that acts as the central orchestrator:

- **MCPL server** — stdio or TCP transport, JSON-RPC 2.0 (MCP backward-compatible)
- **Lobby client** — connects to zero-k.info:8200, handles login, chat, matchmaking
- **Engine manager** — spawns headless Spring instances with write-dir isolation
- **SAI IPC server** — Unix socket listener for SAI bridge connections
- **Channel routing** — each game instance becomes a bidirectional MCPL channel

### SAI Bridge (`sai-bridge/`)

Rust cdylib (shared library) loaded by the Recoil engine as a Skirmish AI:

- Implements the `init` / `release` / `handleEvent` C interface
- Connects to GameManager over Unix socket IPC
- Forwards game events (unit_created, enemy_enter_los, update, ...) as JSON
- Polls for commands (move, attack, build, ...) and dispatches them via the engine's 596-entry callback vtable
- Update events throttled to ~1/sec (every 30th frame)

### Agent App (`app/`)

TypeScript application using the Connectome's [Agent Framework](https://github.com/antra-tess/agent-framework) for multi-agent orchestration. Not yet wired for live play — the current milestone is the infrastructure layer.

## Tools

The GameManager exposes these MCP tools:

| Tool | Description |
|------|-------------|
| `lobby_connect` | Connect to Zero-K lobby server |
| `lobby_login` | Authenticate with credentials |
| `lobby_register` | Register a new account |
| `lobby_start_game` | Start a local game (map, opponent, headless mode) |
| `lobby_join_battle` | Join an existing multiplayer battle |
| `lobby_matchmaker_join` | Queue for matchmaking |
| `lobby_say` | Send chat messages |
| `lobby_list_battles` | List open battles |
| `lobby_list_users` | List online users |

## Game Events

Events flow from the engine through the SAI bridge to the LLM as `channels/incoming` messages:

| Event | Fields | Description |
|-------|--------|-------------|
| `init` | frame | Game initialized |
| `update` | frame | Game tick (~1/sec) |
| `unit_created` | unit, builder | New unit constructed |
| `unit_finished` | unit | Unit construction complete |
| `unit_idle` | unit | Unit has no orders |
| `unit_destroyed` | unit, attacker | Unit killed |
| `enemy_enter_los` | enemy | Enemy spotted |
| `enemy_destroyed` | enemy, attacker | Enemy killed |
| `message` | player, text | In-game chat |

## Game Commands

Commands are sent via `channels/publish` as JSON:

```json
{"type": "move", "unit_id": 42, "x": 1024, "y": 0, "z": 2048}
{"type": "attack", "unit_id": 42, "target_id": 99}
{"type": "build", "unit_id": 42, "build_def_id": 7, "x": 512, "y": 0, "z": 512}
{"type": "patrol", "unit_id": 42, "x": 1500, "y": 0, "z": 1500}
{"type": "fight", "unit_id": 42, "x": 2000, "y": 0, "z": 2000}
{"type": "guard", "unit_id": 42, "guard_id": 43}
{"type": "repair", "unit_id": 42, "repair_id": 43}
{"type": "send_chat", "text": "glhf"}
```

All movement commands support `"queue": true` for shift-queuing.

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Recoil/Spring engine](https://github.com/beyond-all-reason/spring) installed at `~/.spring/engine/`
- Zero-K game files in `~/.spring/` (install via [Zero-K launcher](https://zero-k.info) or Chobby)
- A map (e.g., SimpleChess) in `~/.spring/maps/`

### Build

```bash
# GameManager
cd game-manager && cargo build

# SAI bridge (produces libSkirmishAI.so)
cd sai-bridge && cargo build --release
```

### Run with Claude Code

Add to your `.mcp.json`:

```json
{
  "mcpServers": {
    "zk-game-manager": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "path/to/game-manager/Cargo.toml", "--", "--stdio", "--write-dir", "/path/to/write-dir"]
    }
  }
}
```

Then ask Claude to start a game:

> Start a local game on SimpleChess against NullAI

Claude will call `lobby_start_game`, receive game events through the channel, and can issue commands back. Against a NullAI (which does nothing), winning is a matter of walking across the map and destroying the idle enemy commander.

### Integration Tests

```bash
cd game-manager

# Full test suite (tiers 1-3)
python3 tests/integration_test.py --tier 3

# Quick engine launch test only
python3 tests/integration_test.py --tier 1

# Verbose (shows JSON-RPC traffic)
python3 tests/integration_test.py --tier 3 -v

# Fresh write-dir (no cached archives)
python3 tests/integration_test.py --tier 3 --fresh
```

Test tiers:
1. **Engine Launch** — game starts, infolog.txt created
2. **SAI Boot** — Init event received, Update events flowing, SAI connected
3. **Command Round-Trip** — chat command delivered, unit events observed

## Architecture

The project is part of a larger agent framework that includes a branchable event store (Chronicle), LLM abstraction layer (Membrane), and multi-agent orchestration. The Zero-K agent is designed to eventually support:

- **Multi-agent roles** — strategist, tactician, economist operating at different frequencies
- **Hypothesis testing** — rewind game state via savestates, explore alternative strategies
- **Compilation pipeline** — promote successful LLM strategies to Lua scripts, then to compiled Rust
- **Competitive play** — matchmaking on the Zero-K ladder via the lobby protocol

## License

MIT
