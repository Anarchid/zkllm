# Zero-K Agent Fleet: Implementation Proposal v3

**Version:** 3.0
**Date:** February 2026
**Status:** Draft
**Builds on:** v2 proposal, Recoil engine AI interface analysis, MCPL specification v0.4

---

## Overview

A multi-agent system that learns to play Zero-K through externalized cognition — writing and refining perception tools, strategic documents, and execution macros across rewindable training games, then deploying accumulated knowledge in non-rewindable tournament play.

The system combines:
- **In-context learning** for sample-efficient adaptation (including to specific opponents)
- **Externalized artifacts** that persist across sessions and survive context limits
- **Compiled execution** from interpreted scripts to native Rust, progressively elevated
- **Narrative reasoning** for temporal projection and intent inference — capabilities GOFAI lacks

The agent fleet doesn't learn by updating weights. It learns by building better infrastructure for itself.

---

## Key Insight: What LLMs Add to RTS AI

Traditional game AI (GOFAI) excels at:
- Fast computation (per-frame decisions)
- Threat mapping and influence fields
- Pathfinding and target prioritization
- Reactive micro (kiting, focus fire)

Traditional game AI cannot do:
- **Intent inference**: "That group is *heading toward* my expansion, not just near it"
- **Temporal reasoning**: "They'll arrive in 40s, my turret needs 60s — abort"
- **Narrative synthesis**: "I lost because I attacked into superior force without scouting"
- **Opponent modeling**: "This player always early-expands — I should scout and punish"
- **Counterfactual analysis**: "If I had retreated instead of fighting, I'd have saved 300 metal"

LLMs excel at exactly these capabilities. The architecture plays to each system's strengths:
- **Compiled macros** handle per-frame execution (GOFAI-style, fast)
- **LLM agents** handle narrative reasoning and strategic decisions (slow, but powerful)

### The ICL Advantage

In-context learning enables adaptation that weight-trained AI cannot match:
- Adapt to a specific opponent's tendencies *within a single session*
- Incorporate new unit stats after a balance patch without retraining
- Learn from a handful of examples rather than millions of games
- Explain decisions in natural language for debugging

A weight-trained bot that doesn't know what an Odin is will never learn without retraining. An ICL agent just needs the unit stats in context.

---

## System Architecture

### Two Components

```
┌───────────────────────────────────────────────────────────────────────┐
│                    Agent Framework (MCPL Host)                          │
│                    Persistent process, TypeScript                       │
│                                                                         │
│  Agents ──► Membrane ──► LLM                                           │
│  Modules: ArtifactStore, DecisionLog, MCPLModule                       │
│  State: Chronicle (branchable event store)                             │
│                                                                         │
│  Single MCPL connection:                                                │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  MCPLModule → GameManager                                        │  │
│  │  (always connected)                                              │  │
│  └──────────────────────────────┬───────────────────────────────────┘  │
└─────────────────────────────────┼──────────────────────────────────────┘
                                  │ TCP (MCPL)
                                  ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    GameManager MCPL Server                                │
│                    Persistent process, Rust                               │
│                                                                          │
│  Feature Sets:                                                           │
│  ┌──────────────────────────────┐  ┌─────────────────────────────────┐  │
│  │  game.* (reversible)    ⟲    │  │  lobby.* (non-reversible)       │  │
│  │                              │  │                                 │  │
│  │  game.state     rollback ✓  │  │  lobby.connection               │  │
│  │  game.perception            │  │  lobby.battles                  │  │
│  │  game.commands              │  │  lobby.chat                     │  │
│  │  game.macros    rollback ✓  │  │  lobby.replays                  │  │
│  │  game.events                │  │                                 │  │
│  └──────────────────────────────┘  └─────────────────────────────────┘  │
│                                                                          │
│  Channels (game instances):                                              │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐                        │
│  │ game:live-1│  │ game:rpl-1 │  │ game:hyp-1 │                        │
│  │ (playing)  │  │ (replay)   │  │ (rewind)   │                        │
│  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘                        │
│        │ IPC           │ IPC           │ IPC                            │
│  ┌─────▼──────┐  ┌─────▼──────┐  ┌─────▼──────┐                        │
│  │ Engine+SAI │  │ Engine+SAI │  │ Engine+SAI │                        │
│  │ (process)  │  │ (process)  │  │ (process)  │                        │
│  └────────────┘  └────────────┘  └────────────┘                        │
│                                                                          │
│  ZK lobby protocol (TCP to zero-k.info:8200)                            │
│  Engine process management (spawn, kill, savestate)                      │
│  Lobby protocol ported from Chobby (Lua) and yylobby (TS)              │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Why One Server, Not Two?

The SkirmishAI.so only exists **during a running game**. It cannot handle save/load (it dies on engine restart) and has no lobby awareness. A persistent GameManager wrapping both concerns solves this:

- **Reversibility**: `game.*` feature sets declare `rollback: true`. The GameManager survives engine restarts, so `state/rollback` has a stable recipient. The SAI.so is ephemeral — it doesn't need to manage its own lifecycle.
- **Partial reversibility via feature sets**: MCPL's `rollback` is per-feature-set, not per-server. `game.state` is reversible; `lobby.chat` is not. One server, clean boundary.
- **No coordination problem**: The Lobby knows *when* to start a game, the game manager knows *how*. Same process = no IPC for orchestration.
- **Channels = game instances**: Each running engine is an MCPL channel. Multiple concurrent games, replays alongside live play, hypothesis-testing branches — all natural channel operations.

```
              GameManager alive (always)
◄─────────────────────────────────────────────────────────────►

──┬──────────┬────────┬──────────────────┬────────┬──────────┬──
  │  Lobby   │ Engine │      Game        │ Engine │  Lobby   │
  │  join    │ load   │  (sim running)   │ exit   │  results │
  │  chat    │ init   │  SAI alive       │        │  next?   │
──┴──────────┴────────┴──────────────────┴────────┴──────────┴──

  ◄───────── All handled by GameManager ─────────────────────►
```

The Agent Framework has a single, always-on MCPL connection. Game lifecycle is expressed through channel open/close events.

---

## SkirmishAI.so (In-Engine Component)

### Design: Thin Bridge, No MCPL

The .so is a thin bridge between the engine and the GameManager. It does **not** host an MCPL server — the GameManager handles all protocol communication. The SAI's job is to execute commands, run scripts, and forward events.

It does **not** require a companion Lua gadget. Gadgets are synced Lua distributed inside game archives, requiring game-developer buy-in. The SkirmishAI installs independently like any other AI.

The Recoil AI C API (`SSkirmishAICallback`) provides:
- **Full game state queries**: units, positions, velocities, health, economy, terrain, LOS
- **Command execution**: move, attack, build, patrol, guard, cloak, all unit orders
- **Pause control**: `COMMAND_PAUSE` with `SPauseCommand { enable, reason }`
- **Lua messaging**: `COMMAND_CALL_LUA_RULES` for any game-specific extensions

### Internal Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                    SkirmishAI.so (Rust cdylib)                        │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │                  Sim Thread (engine calls us)                   │  │
│  │                                                                 │  │
│  │  handleEvent() dispatch:                                       │  │
│  │                                                                 │  │
│  │  EVENT_INIT:                                                   │  │
│  │    Store SSkirmishAICallback pointer                           │  │
│  │    Initialize Lua runtime, load bootstrap scripts              │  │
│  │    Connect to GameManager via IPC (Unix socket)                │  │
│  │                                                                 │  │
│  │  EVENT_UPDATE (every sim frame, ~30 Hz):                       │  │
│  │    1. Tick active macros in Lua runtime                        │  │
│  │    2. Drain request channel (from GameManager)                 │  │
│  │       For each request:                                        │  │
│  │         Perception/Analysis → execute in Lua, return result    │  │
│  │         GameCommand → execute via C API, return confirmation   │  │
│  │         ScriptLoad → load into Lua sandbox, return status      │  │
│  │         MacroManage → activate/adjust/deactivate macro         │  │
│  │    3. Send results via response channel                        │  │
│  │                                                                 │  │
│  │  EVENT_ENEMY_ENTER_LOS, EVENT_UNIT_DESTROYED, etc.:           │  │
│  │    Evaluate significance filter                                │  │
│  │    If significant → forward to GameManager via event channel   │  │
│  │                                                                 │  │
│  │  EVENT_RELEASE:                                                │  │
│  │    Disconnect from GameManager                                 │  │
│  │    Clean up Lua runtime                                        │  │
│  │                                                                 │  │
│  └───────────────────────────────────────┬────────────────────────┘  │
│                                          │                           │
│  ┌───────────────────────────────────────┴────────────────────────┐  │
│  │                  IPC Thread                                     │  │
│  │                                                                 │  │
│  │  Unix socket to GameManager                                    │  │
│  │  ├── Receive requests → request channel → sim thread           │  │
│  │  ├── Send results from sim thread → GameManager                │  │
│  │  └── Send game events from sim thread → GameManager            │  │
│  │                                                                 │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │                  Lua Runtime (mlua + LuaJIT)                    │  │
│  │                                                                 │  │
│  │  Bound functions from SSkirmishAICallback:                     │  │
│  │  ├── Game.get_enemies_in(x, z, radius) → [{id, def, pos, hp}]│  │
│  │  ├── Game.get_unit(id) → {pos, vel, hp, def, orders, ...}    │  │
│  │  ├── Game.get_economy() → {metal, energy, income, drain}      │  │
│  │  ├── Game.get_unit_def(def_id) → {name, cost, speed, ...}    │  │
│  │  ├── Game.give_order(unit_id, cmd, params)  [macros only]     │  │
│  │  └── (~50-80 bound functions)                                  │  │
│  │                                                                 │  │
│  │  Sandboxed environments:                                       │  │
│  │  ├── perception_env: read-only Game.* bindings                │  │
│  │  └── macro_env: read-only + Game.give_order                   │  │
│  │                                                                 │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │                  Compiled Hot Path (Rust)                       │  │
│  │                                                                 │  │
│  │  Promoted from Lua by Toolsmith when proven stable:            │  │
│  │  ├── threat_assessment_v12() — was Lua, now native Rust       │  │
│  │  ├── kite_vs_heavy_v8() — was Lua, now native Rust            │  │
│  │  └── economy_snapshot_v3() — was Lua, now native Rust         │  │
│  │                                                                 │  │
│  │  Initial bootstrap (looted from ZKGBAI Rust port):            │  │
│  │  ├── basic_threat_assessment()                                 │  │
│  │  ├── unit_categorization()                                     │  │
│  │  └── resource_flow_analysis()                                  │  │
│  │                                                                 │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### Threading Model

The Recoil AI interface is **single-threaded** — all callbacks happen on the sim thread.

```
Sim thread (owned by engine):
    handleEvent() called at ~30 Hz
    ├── Lua macro execution (synchronous, per-frame)
    ├── Request execution (synchronous, from IPC channel)
    └── Event forwarding (non-blocking sends to GameManager)

IPC thread (owned by us):
    Unix socket to GameManager process
    ├── Inbound: GameManager requests → channel → sim thread
    ├── Outbound: sim thread → channel → GameManager responses
    └── Outbound: sim thread → channel → game events

Communication: std::sync::mpsc channels (lock-free, bounded)
```

### Latency Budget

```
Tool call round-trip (AF → GameManager → SAI → GameManager → AF):

AF sends tools/call via MCPL to GameManager  ~1ms  (local TCP)
GameManager forwards to SAI via IPC           ~0ms  (Unix socket)
Sim thread picks up on next EVENT_UPDATE      0-33ms (up to 1 frame at 30fps)
Lua tool execution                            ~1-5ms (typical perception query)
Result sent to GameManager via IPC            ~0ms  (Unix socket)
GameManager sends MCPL response to AF         ~1ms  (local TCP)
                                              ──────
Total:                                        ~3-40ms

At Tactician's 0.1-0.2 Hz rate: negligible vs. 5-10s inference time.
Extra IPC hop adds <1ms vs. the old direct-MCPL design.
```

### Event Significance Filtering

Not every game event becomes a push event. The .so filters on the sim thread to prevent flooding:

- `EVENT_ENEMY_ENTER_LOS`: Only push if unit cost exceeds threshold, or first sighting of a unit type
- `EVENT_UNIT_DESTROYED`: Only push for high-value units, or if part of an active engagement
- `EVENT_UNIT_IDLE`: Only push for factories or builders (not individual combat units)

### Build and Deployment

```
AI/Skirmish/AgentBridge/0.1/
├── AIInfo.lua                      # Metadata for engine AI discovery
├── AIOptions.lua                   # Configurable: IPC socket path, log level
├── libSkirmishAI.so                # Rust-compiled bridge (single file)
└── bootstrap/                      # Initial Lua tools (loaded at EVENT_INIT)
    ├── perception/
    │   ├── threat.lua              # Adapted from CAI
    │   ├── economy.lua             # Adapted from CAI
    │   └── army_tracker.lua        # Adapted from CAI
    ├── analysis/
    │   └── engagement_sim.lua      # New or adapted
    └── macros/
        ├── kiting.lua              # Adapted from CAI
        └── defense.lua             # Adapted from CAI
```

Installs like any other Recoil AI. No game modifications required.

### SAI Rust Build

```toml
# Cargo.toml (SkirmishAI component)
[lib]
crate-type = ["cdylib"]
name = "SkirmishAI"

[dependencies]
mlua = { version = "0.10", features = ["luajit", "serialize"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
# No tokio needed — IPC thread is lightweight, no async runtime required

[build-dependencies]
bindgen = "0.69"  # Auto-generate Rust bindings from Recoil C headers
```

### FFI Surface

Minimal — one exported function, one stored struct:

```rust
// Exported to engine
#[no_mangle]
pub extern "C" fn handleEvent(ai_id: c_int, topic: c_int, data: *const c_void) -> c_int;

// Stored from EVENT_INIT
static CALLBACK: *const SSkirmishAICallback;  // ~200 function pointers, auto-generated by bindgen
```

### Game Joining: How the AI Enters Play

The SkirmishAI.so is a standard Recoil SkirmishAI. How it gets loaded depends on context:

**Self-Hosted Games (Scrimmage, Local Testing)**

The AI is defined in the start script before the game launches. The Lobby configures it directly:

```lua
-- In startscript.txt
[AI0]
{
    Name = AgentBridge;
    ShortName = AgentBridge;
    Version = 0.1;
    Team = 0;
    Host = 0;
}
```

Standard path. No special mechanisms needed.

**Remote / Competitive Multiplayer**

Competitive autohosts typically block AI player addition at the lobby protocol level — you can't configure an AI slot pre-game. The solution: a **bootstrap widget** that hands control to the SkirmishAI after the game starts.

```lua
-- bootstrap_agentbridge.lua (client-side widget, ~10 lines)
function widget:GetInfo()
    return { name = "AgentBridge Bootstrap", layer = 0 }
end

function widget:Initialize()
    Spring.SendCommands("aicontrol AgentBridge 0.1")
    widgetHandler:RemoveWidget(self)  -- one-shot, self-removes
end
```

This fires `/aicontrol` on game init, which triggers `NETMSG_AI_CREATED` — the engine loads the SkirmishAI.so to control the player's own team. The AI runs locally on the player's machine; the server sees only standard unit commands. No network-level distinction between human and AI commands.

**Engine Permission Model**

The `/aicontrol` path is governed by `aiControlFlags`, a per-player server-side flag:
- **Defaults to `false`** (allowed) — `/aicontrol` works unless explicitly blocked
- Only the game host can toggle it, via server commands `aictrl <player-name>` or `aictrlbynum <id>`
- Additional permission check: player must be allied to the target team, or be the host

In practice: most servers leave this at default (allowed), since the flag was designed for specific tournament lockdown scenarios. The bootstrap widget fires on `Initialize()` — before any host reaction — providing a reliable window.

**Fallback: If `/aicontrol` Is Blocked**

If a host explicitly sets `aiControlFlags` for the player, the bootstrap widget approach won't work. This is an edge case (requires active host intervention), but if needed:
- Negotiate with tournament organizers to allow AI participation
- Use a companion widget for direct unit control (slower, less capable, but doesn't require SAI loading)
- Self-host the game instead

**Single Code Path**

Critically, the SkirmishAI.so itself is identical in both cases. It doesn't know or care whether it was loaded via start script or `/aicontrol`. The MCPL server starts either way, the Agent Framework connects the same way, and the agents play the same way.

### Optional: Companion Widget

A client-side Lua widget (unsynced, unpermissioned) can supplement the .so for data the AI C API doesn't expose:
- Screen renders and visual analysis
- Minimap as rendered
- UI-layer state

This is distinct from the bootstrap widget above (which is a one-shot loader). The companion widget would be a persistent data-access supplement. Widgets don't require game-dev buy-in — they're client-installed. Communication via `COMMAND_CALL_LUA_UI`. This is optional and late-stage.

---

## GameManager MCPL Server

### What It Is

A unified, persistent Rust process that handles everything between AF and the game: lobby protocol, engine management, SAI communication, savestates, and reversibility. It is the single MCPL server that AF connects to.

### Implementation

**Lobby protocol** ported to Rust from two reference implementations:
- **Chobby** (Lua, in `~/.spring/games/chobby.sdd`) — the canonical ZK lobby client
- **yylobby** (TypeScript, in `yylobby/`) — clean layered architecture, easier to read

**Zero-K lobby protocol**: Text-based TCP on `zero-k.info:8200`. Format: `CommandName JSON\n`. Handles: authentication (MD5), channels, chat, battle listing/joining, user management.

```rust
struct GameManager {
    // MCPL server (connection to AF)
    mcpl_server: McplServer,

    // Lobby protocol
    lobby_conn: Option<LobbyConnection>,  // TCP to zero-k.info:8200

    // Engine instances (channels)
    engines: HashMap<ChannelId, EngineInstance>,

    // Savestate index (for rollback)
    checkpoints: BTreeMap<CheckpointId, SavestateRef>,
}

struct EngineInstance {
    process: Child,
    sai_socket: Option<UnixStream>,  // IPC to SAI.so
    channel_id: ChannelId,
    loaded_scripts: Vec<ScriptRef>,   // for reconstruction on rollback
    active_macros: Vec<MacroRef>,     // for reconstruction on rollback
    status: GameStatus,
}

enum GameStatus {
    Loading,
    Running { frame: u32 },
    Paused { frame: u32 },
    Ended { result: GameResult },
}
```

### MCPL Feature Sets

```jsonc
{
  "featureSets": {
    // === Reversible (game state) ===
    "game.state": {
      "description": "Game lifecycle, save/load/rewind",
      "uses": ["tools"],
      "rollback": true,
      "hostState": false
    },
    "game.perception": {
      "description": "Query game state via perception tools",
      "uses": ["tools"]
    },
    "game.commands": {
      "description": "Issue game commands (move, attack, build)",
      "uses": ["tools"],
      "rollback": true
    },
    "game.macros": {
      "description": "Manage autonomous macros",
      "uses": ["tools"],
      "rollback": true
    },
    "game.events": {
      "description": "Game event push notifications",
      "uses": ["pushEvents"]
    },
    "game.context": {
      "description": "Game state context injection",
      "uses": ["contextHooks.beforeInference"]
    },

    // === Non-reversible (lobby) ===
    "lobby.connection": {
      "description": "Connect to lobby servers",
      "uses": ["tools"]
    },
    "lobby.battles": {
      "description": "Browse and join battles",
      "uses": ["tools", "pushEvents"]
    },
    "lobby.chat": {
      "description": "Lobby chat",
      "uses": ["tools", "channels.publish"],
      "scoped": true
    },
    "lobby.replays": {
      "description": "Replay browsing and loading",
      "uses": ["tools"]
    }
  }
}
```

### MCPL Tools

```
# Lobby (non-reversible)
lobby:connect(server, credentials)     → success/error
lobby:disconnect()                     → success
lobby:list_battles(filters?)           → [{id, map, players, type, ...}]
lobby:join_battle(battle_id)           → success/error
lobby:create_battle(settings)          → battle_id
lobby:leave_battle()                   → success
lobby:chat(channel, message)           → success
lobby:get_player_info(name)            → {rank, games, win_rate, ...}
lobby:list_replays(filters?)           → [{path, map, result, date, ...}]

# Game lifecycle (reversible via state/rollback)
game:start(config)                     → channel_id  (launches engine, opens channel)
game:start_vs_ai(map, ai, difficulty)  → channel_id  (local scrimmage)
game:load_replay(path)                 → channel_id  (replay analysis)
game:stop(channel_id)                  → success     (kills engine, closes channel)
game:pause(channel_id)                 → success
game:resume(channel_id)               → success
game:save_checkpoint(channel_id, name) → checkpoint_id
game:get_result(channel_id)           → {winner, duration, stats}

# Perception (proxied to SAI)
game:perception(channel_id, tool, args) → result
game:command(channel_id, cmd, args)     → confirmation
game:script_load(channel_id, script)    → status
game:macro_activate(channel_id, ...)    → macro_id
game:macro_deactivate(channel_id, id)   → success
```

### MCPL Push Events

```
# Lobby events
lobby:connected / lobby:disconnected
lobby:battle_listed / lobby:battle_updated
lobby:chat_received(from, channel, message)

# Game events (via channels)
game:channel_opened(channel_id, info)    (engine started, SAI connected)
game:channel_closed(channel_id, result)  (engine stopped)
game:event(channel_id, event_type, data) (significant game events from SAI)
```

### Rollback Flow

When AF issues `state/rollback` for `game.state`:

```
1. AF → GameManager: state/rollback { featureSet: "game.state", checkpoint: "chk_7200" }
2. GameManager looks up chk_7200 → savestate file + recorded script/macro state
3. GameManager kills current engine process (SAI dies)
4. GameManager relaunches engine from savestate
5. Engine loads → SAI.so initializes → connects to GameManager via IPC
6. GameManager replays script loads and macro activations into fresh SAI
7. GameManager → AF: channels/changed { updated: [{ id: "game:live-1", ... }] }
8. GameManager → AF: rollback success { checkpoint: "chk_7200" }
```

The SAI is unaware this happened — it just initializes normally and receives commands.

---

## The Perception Architecture

### Problem: Game State Explosion

A Zero-K frame contains:
- Thousands of unit positions, velocities, health, orders
- Hundreds of projectiles in flight with physics
- Terrain, LOS, radar coverage, fog of war
- Economy flows, build queues, factory states

This cannot fit in LLM context. Raw state injection is impossible.

### Solution: Agents Build Their Own Perception

```
Game State (raw, huge, inaccessible to LLM)
       │
       ▼
┌───────────────────────────────────────────────────┐
│  Agent-Written Perception Tools                    │
│  Running in SkirmishAI.so (Lua or compiled Rust)  │
│                                                    │
│  • threat_assessment(sector) → summary            │
│  • army_composition() → structured data           │
│  • economy_snapshot() → key metrics               │
│  • infer_enemy_intent(group) → prediction         │
└───────────────────────────────────────────────────┘
       │
       ▼
Structured Summaries (small, semantic, fits context)
       │
       ▼
LLM Agent
```

The agents never see raw state. They call perception tools that return summaries. The tools themselves become artifacts that agents improve over time.

### Bootstrap from Existing AIs

| Source AI | Language | Adaptation Path |
|-----------|----------|-----------------|
| **CAI** | Lua (gadget, bundled with ZK) | Adapt `Spring.*` API calls to our embedded Lua bindings. Structure stays similar. Most directly reusable for bootstrap Lua tools. |
| **CircuitAI** | C++ | Extract algorithms, reimplement as Rust perception tools or Lua scripts. |
| **ZKGBAI** | Java (original), **Rust (port available)** | Rust port looted directly into compiled hot-path tier. Perception, threat analysis, build order logic go straight into Rust side for initial tests. |

**Day one bootstrap strategy:**
- ZKGBAI Rust port → compiled Rust perception tools (immediate, no adaptation)
- CAI Lua → adapted Lua scripts for tools not yet in Rust (API rebinding)
- CircuitAI C++ → reference for algorithm design, ported as needed

### Tool Categories

**Perception** (read game state, return summaries)
```lua
threat_assessment(sector_x, sector_z, radius)
  → { level: 450, composition: {raider: 8, assault: 4}, high_threat_units: [...] }

army_summary(team)
  → { groups: [...], total_value: 2400, factory_count: 3 }

economy_status()
  → { metal: 450, energy: 800, income: +18, drain: -12, stall_risk: false }
```

**Analysis** (compute derived insights)
```lua
estimate_travel_time(group, destination)
  → { seconds: 38, path: [...], confidence: 0.9 }

estimate_build_completion(structure_id, additional_builders?)
  → { seconds: 45, current_rate: "2.1%/s", bottleneck: "none" }

predict_engagement(friendly_group, enemy_group)
  → { winner: "enemy", friendly_survivors: 2, enemy_survivors: 8, confidence: 0.7 }

infer_intent(enemy_group, trajectory_frames)
  → { likely_targets: [{target: "expansion_E", prob: 0.8}], behavior: "raiding" }
```

**Effectors** (execute game commands)
```lua
move_group(group_id, target_position, formation)
attack_move(group_id, target_position)
build_unit(factory_id, unit_type, repeat_count)
build_structure(builder_id, structure_type, position)
```

**Macros** (compound behaviors, run autonomously at frame rate)
```lua
activate_macro("kite_vs_heavy", { group: "raiders", params: {retreat_hp: 40} })
  → macro_id

adjust_macro(macro_id, { params: {retreat_hp: 60} })

deactivate_macro(macro_id)

-- Macro runs every frame in Lua (or compiled Rust), no LLM involved
-- Terminates when conditions met or agent deactivates
```

### Replay REPL (Analyst/Toolsmith Only)

Post-game, the Analyst and Toolsmith get interactive query access. In replay mode, the SkirmishAI.so has access to the full game state with fog of war lifted (via cheats API or replay observer mode).

```lua
-- Analyst investigating a lost engagement
replay_query("get_enemies_in(2400, 1800, 600)")
  → { units: [{id: 891, type: "Scorcher", hp: 560, pos: ...}, ...] }

replay_query("track_unit_type_count('Scorcher', from=0, to=7200)")
  → [(0, 0), (1800, 2), (3600, 5), (5400, 8), (7200, 12)]

-- Toolsmith testing a new perception function
replay_query("test_threat_v2(2400, 1800, 600)")
  → { level: 580, ... }  -- compare to actual outcome
```

---

## Agent Roles

### Tick Rate Reality

LLM inference latency dominates. Benchmarks with capable local models (e.g., K2) show ~500ms minimum for single-word responses. With meaningful context, expect:

| Agent | Inference Time | Effective Rate | Decisions/Minute |
|-------|----------------|----------------|------------------|
| Strategist | 15-30s | 0.03-0.07 Hz | 2-4 |
| Economist | 8-15s | 0.07-0.12 Hz | 4-7 |
| Tactician | 5-10s | 0.1-0.2 Hz | 6-12 |
| Analyst | N/A | Async | Post-game |
| Toolsmith | N/A | Async | Between games |
| Librarian | 2-5s | 0.2-0.5 Hz | On-demand |

These rates are acceptable because agents aren't doing per-frame micro — they're orchestrating macros and making strategic judgments that require reasoning.

### Agent Descriptions

**Strategist** (0.03-0.07 Hz)
- High-level game plan: tech path, build priorities, expansion timing
- Reads opponent model, selects counter-strategy
- Operates at the "what should we be trying to do" level

**Tactician** (0.1-0.2 Hz)
- Macro orchestration: activate, parameterize, and terminate combat macros
- **Narrative reasoning**: project futures, infer enemy intent, evaluate options
- Key capability: temporal reasoning that GOFAI cannot do
- Example decision: "They'll arrive in 40s, turret needs 60s — abort and cloak"

**Economist** (0.07-0.12 Hz)
- Resource flow optimization
- Factory allocation and build queue management
- Expansion timing decisions
- Detects and resolves floating/stalling

**Analyst** (async, post-game or on rewind)
- Post-action review with full replay access (fog lifted)
- REPL queries against historical game state
- Hypothesis generation: "We lost because X"
- Artifact update proposals with evidence

**Toolsmith** (async, between games)
- Writes and refines Lua scripts (perception, macros, strategies)
- Tests scripts against replay data via REPL
- **Promotes stable scripts to Rust** for compiled hot-path execution
- Debugs failed macros by examining execution traces

**Librarian** (on-demand, 0.2-0.5 Hz)
- Maintains artifact indices and summaries
- Resolves conflicts between artifact versions
- Answers queries: "What's our current build order for anti-air?"
- Manages context loading: decides what each agent should see

### The Tactician's Unique Capability: Narrativization

The Tactician's key contribution is turning event streams into actionable narratives. A common flaw of GOFAI bots is their inability to sequence observations into temporal conclusions:

**GOFAI sees:**
```
threat_level_at_E = 500
defense_at_E = 200
if threat > defense: retreat
```

**Tactician reasons:**
```
"I see 8 Scorchers at (1200, 800) moving toward expansion E at (2400, 1800).

Query: estimate_travel_time(scorchers, expansion_E) → 38 seconds
Query: estimate_build_completion(lotus_at_E) → 30 seconds
Query: predict_engagement(lotus, scorchers) → lotus loses, kills 2

The Lotus will finish before they arrive, but won't survive the fight.
Net outcome if I finish: lose builder + lotus, kill 2 scorchers = -100m
Net outcome if I abort: lose sunk cost = -70m

Better to abort. Cloak the builder, save 110 metal."
```

This requires intent inference, temporal projection, counterfactual evaluation, and judgment under uncertainty. GOFAI cannot do this. LLMs can.

### Agent Access Matrix

| Agent | Live Game | Replay | Lobby |
|-------|-----------|--------|-------|
| Strategist | Perception tools, strategic commands, macros | — | — |
| Tactician | Perception tools, tactical commands, macros | — | — |
| Economist | Economy perception, build commands | — | — |
| Analyst | — | Full REPL (fog lifted) | Pre-game opponent lookup |
| Toolsmith | — | Full REPL (testing) | — |
| Librarian | — | — | — (artifacts only) |

---

## The Compilation Pipeline

Strategic insights progressively compile into faster execution layers:

```
Tier 2: LLM Inference (5-30 seconds per decision)
        │  Handles: novel situations, strategic reasoning, judgment calls
        │
        │  "Scorchers seem weak against Lotus at choke points"
        │
        ▼
Tier 1: Lua Scripts (interpreted, per-frame, hot-reloadable)
        │  Handles: known patterns, parameterized behaviors
        │
        │  function auto_lotus_at_chokes(params)
        │    if detect_scorchers() and near_choke() then place_lotus() end
        │  end
        │
        ▼
Tier 0: Compiled Rust (native speed, requires rebuild)
        │  Handles: proven stable tools, performance-critical hot paths
        │
        │  fn threat_assessment(x: f32, z: f32, radius: f32) -> ThreatSummary
        │
        (fastest)
```

Each tier up: slower, more flexible, handles novel situations.
Each tier down: faster, more rigid, handles known situations.

**Toolsmith's role in compilation:**
1. Write tool/macro as Lua script
2. Test against replay data
3. Deploy to live games, monitor correctness
4. When stable across N games with zero errors: write equivalent Rust implementation
5. Test Rust version for equivalence against Lua reference
6. Promote to compiled tier
7. Lua version stays as fallback/reference

**Bootstrap strategy:**
- ZKGBAI Rust port → immediate Tier 0 tools (no Lua phase needed)
- CAI Lua → Tier 1 scripts (adapt API bindings)
- Agent-written improvements start as Tier 1, promote to Tier 0 over time

---

## Artifact Store

All learning externalizes into versioned artifacts stored in Chronicle.

### Artifact Types

**Strategic Documents**
- Opponent models: tendencies, preferred strategies, weaknesses
- Build order playbook: conditions for selection, expected timings
- Map analysis: terrain features, choke points, expansion sites
- Unit matchup tables: with confidence levels and evidence citations

**Policies**
- Decision heuristics: "If floating metal > 500 and enemy air scouted, build AA"
- Engagement rules: force ratio thresholds, retreat conditions
- Timing windows: "Anti-air must be up by 8:00 against this opponent"

**Tools (Scripts and Compiled)**
- Perception tools: threat assessment, army tracking, economy monitoring
- Macros: kiting, raiding, defense behaviors
- Analysis tools: engagement prediction, timing estimation
- Strategies: complete automated routines for specific situations

**Meta-Artifacts**
- Learning log: timestamped insights with confidence and evidence
- Open questions: hypotheses to test in next scrimmage
- Performance tracking: win rates by strategy, by opponent, over time

### Versioning and Confidence

Every artifact has:
- Version history (stored in Chronicle branches)
- Confidence level (how proven is this?)
- Evidence citations (which games/decisions support this?)
- Last validated (when did we last confirm this works?)

When tournament performance degrades, the system can:
1. Identify which artifacts were used in losses
2. Compare to artifacts used in wins
3. Roll back suspect artifacts to earlier versions
4. Flag for Analyst review

---

## Rewind Protocol

### Purpose

Rewind is for **hypothesis testing**, not outcome fishing. Every rewind produces learning artifacts regardless of whether the alternative succeeded.

### Mechanism

```
1. Create checkpoint
   Game Bridge saves game state (Spring savestate)
   Chronicle creates branch point
   Decision journal marks checkpoint with game context

2. Game continues, outcome observed
   Original timeline recorded on current Chronicle branch

3. Analyst reviews (post-game or mid-scrimmage)
   Uses replay REPL to investigate what happened
   Generates hypothesis: "We should have retreated instead of engaging"

4. Restore checkpoint
   Lobby relaunches engine with savestate (or Game Bridge restores in-game)
   Chronicle switches to new branch
   AF modules receive revert notification, reset ephemeral state
   Lua scripts and macros reset to checkpoint state

5. Play alternative
   Execute alternative strategy
   Record decisions on new branch

6. Compare and record
   Both branches exist in Chronicle
   Analyst compares outcomes
   Updates artifacts with findings
```

### Branch Lineage

```
chk_0_gamestart
└── chk_3000_early
    └── chk_5000_midgame
        ├── chk_7200_preengagement
        │   └── chk_9000_original_loss
        └── chk_7200_hypothesis_retreat
            └── chk_8500_alternative_win
```

### Constraints

- **Limited budget**: N rewinds per scrimmage to prevent degenerate loops
- **Hypothesis required**: Every rewind must have a stated hypothesis
- **Recording required**: Outcomes recorded regardless of success
- **No outcome fishing**: Can't rewind just because you lost

---

## MCPL Protocol Details

The GameManager is the single MCPL server. The Agent Framework is the MCPL host with one persistent connection.

### Per-Agent Permissions

| Agent | game.* Features | lobby.* Features |
|-------|----------------|------------------|
| Strategist | perception, commands, macros, events, context | — |
| Tactician | perception, commands, macros, events.engagement, context | — |
| Economist | perception.economy, commands (build only), events.economy | — |
| Analyst | perception, replay, context | lobby.replays, lobby.chat (read) |
| Toolsmith | perception, replay, macros.manage | — |
| Librarian | (no game access) | — |

### Context Hooks

**beforeInference**: GameManager injects game state summary before each agent inference:
```
<game_state channel="game:live-1" frame="7200" game_time="12:00">
  <economy metal="450" energy="800" income="+18/-12" />
  <army value="2400" groups="3" />
  <enemy_known value="3100" composition="raider-heavy" />
  <map_control friendly="35%" contested="20%" />
  <active_macros>
    <macro id="12" name="defend_base" status="running" />
  </active_macros>
</game_state>
```

**afterInference**: GameManager logs the decision for credit assignment.

### State Management

Game state is too large for the MCPL host to manage. Checkpoints are opaque references (`hostState: false`):

```jsonc
// Checkpoint creation (response from game:save_checkpoint)
{ "state": { "checkpoint": "chk_7200", "parent": "chk_5000" } }

// Rollback (AF → GameManager)
{ "method": "state/rollback", "params": { "featureSet": "game.state", "checkpoint": "chk_7200" } }
```

The GameManager manages checkpoints internally — each maps to an engine savestate file plus recorded script/macro state. Chronicle tracks the checkpoint tree for branching.

---

## Training Loop

### Lifecycle Choreography

```
┌────────────────────────────────────────────────────────────────────────┐
│                    Full Training Session                                 │
│                                                                         │
│  Startup:                                                               │
│    GameManager starts (persistent Rust process)                         │
│    Agent Framework starts, connects to GameManager via MCPL             │
│                                                                         │
│  Scrimmage Loop:                                                        │
│                                                                         │
│    1. AF → GM: game:start_vs_ai(map, ai, difficulty)                   │
│    2. GM launches engine process, SAI.so loads, connects via IPC       │
│    3. GM → AF: channels/changed { added: ["game:scrimmage-1"] }        │
│    4. Agents play game via game:* tools (routed through GM to SAI)     │
│       ├── Strategist sets plan                                          │
│       ├── Tactician manages macros, narrativizes events                │
│       ├── Economist manages resources                                   │
│       └── Decision journal records everything                          │
│    5. Game ends → SAI disconnects from GM                               │
│    6. GM → AF: channels/changed { removed: ["game:scrimmage-1"] }      │
│    7. GM → AF: push/event { game_ended, result }                       │
│    8. AF → GM: game:load_replay(last_game)                             │
│    9. GM launches replay engine → new channel opens                     │
│    10. Analyst reviews via replay REPL                                   │
│    11. Toolsmith refines tools, tests against replay                    │
│    12. Artifacts updated                                                │
│    13. Repeat from step 1                                               │
│                                                                         │
│  Tournament Loop:                                                       │
│                                                                         │
│    1. AF → GM: lobby:connect(server, credentials)                      │
│    2. AF → GM: lobby:list_battles() → select appropriate one           │
│    3. AF → GM: lobby:join_battle(id) (as human player)                 │
│    4. AF → GM: lobby:chat("gl hf")                                     │
│    5. Engine starts with bootstrap widget installed                     │
│       Widget fires /aicontrol → SAI loads, connects to GM via IPC      │
│    6. GM → AF: channels/changed { added: ["game:tournament-1"] }       │
│    7-9. Same as scrimmage steps 4-7 (no rewinds)                       │
│    10. Post-game: replay analysis, artifact confidence update          │
│    11. AF → GM: lobby:chat("gg wp")                                    │
│    12. Find next game                                                   │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

### Curriculum Scheduling

```
early training:    90% scrimmage / 10% tournament
mid training:      60% scrimmage / 40% tournament
late training:     30% scrimmage / 70% tournament

triggers to shift back toward scrimmage:
    - tournament win rate plateaus
    - new opponent strategy with no counter
    - major artifact revision needed
    - new opponent encountered (need to build model)
```

### Opponent Adaptation (ICL Advantage)

Against a specific opponent:
1. First game: observe tendencies, build initial opponent model
2. Between games: update opponent model artifact with observed patterns
3. Second game: Strategist loads opponent model, selects counter-strategy
4. Iterate: refine model, test counters, converge on winning approach

This adaptation happens in minutes/hours, not weeks of retraining.

---

## Context Management

### Tiered Loading

| Tier | Loaded | Contents |
|------|--------|----------|
| 0 | Always | Agent role, current game phase, active objectives |
| 1 | Always | Relevant policies for current situation |
| 2 | On demand | Unit matchup tables, opponent model, map analysis |
| 3 | On trigger | Tool documentation, historical examples, edge cases |

### Librarian's Role

The Librarian maintains indices and answers queries:
- "What artifacts are relevant to early-game defense against raiders?"
- "What's our current opponent model for player X?"
- "Which policy applies when floating metal with enemy air?"

Other agents query the Librarian rather than searching artifacts directly.

### Decision Journaling

Every significant decision is logged:
```
{
  timestamp: "frame 7200",
  agent: "Tactician",
  game_state_hash: "a1b2c3...",
  loaded_artifacts: ["policy:raider_response", "opponent:player_x"],
  perception_queries: ["threat_assessment(E)", "estimate_travel_time(...)"],
  reasoning: "Scorchers heading to E, turret won't finish in time, aborting",
  action: ["cancel_build(lotus)", "activate_ability(builder, cloak)"],
  expected_outcome: "Save builder, lose sunk cost",
  confidence: 0.7
}
```

---

## Implementation Phases

### Phase 0: GameManager + Game Harness (Weeks 1-3)

**Goal:** GameManager can launch a game, communicate with SAI, and relay commands from AF.

- GameManager Rust binary with MCPL server (basic tools only)
- SkirmishAI.so with Rust FFI to Recoil AI C API
- IPC between GameManager and SAI (Unix socket)
- Embedded Lua runtime in SAI with bindings to callback functions
- `game:start_vs_ai` launches engine, SAI connects, channel opens
- Bootstrap perception tools from ZKGBAI Rust port
- MCPLModule in AF (generic MCPL client, first real consumer of the protocol)

**Deliverable:** Manual play test — human issues commands through AF → GameManager → SAI, sees game state summaries.

### Phase 1: Single Agent Baseline (Weeks 4-6)

**Goal:** One general agent can play coherently and learn across games.

- Single agent (no specialization) with basic artifacts
- Lua perception tools bootstrapped from CAI
- GameManager handles local game launching (no multiplayer yet)
- Simple training loop: play → review → update artifacts → repeat

**Deliverable:** Agent references past learnings, artifacts improve across games.

### Phase 2: Agent Specialization (Weeks 7-10)

**Goal:** Specialized agents collaborate effectively.

- Split into Strategist / Tactician / Economist / Analyst / Toolsmith / Librarian
- Implement tick scheduler for different rates
- Context management with tiered loading
- Feature set permissions per agent
- Replay REPL via `game:load_replay` → replay channel

**Deliverable:** Specialization produces better artifacts than single agent.

### Phase 3: Macros and Compilation (Weeks 11-14)

**Goal:** Compiled execution at frame rate; Toolsmith promotes scripts to Rust.

- Macro activation/deactivation system in Lua runtime
- Macro testing sandbox against replay data
- Toolsmith workflow: write Lua → test → deploy → promote to Rust
- Performance profiling of Lua vs Rust execution

**Deliverable:** Macros handle routine micro; inference budget per game decreases.

### Phase 4: Rewind (Weeks 15-18)

**Goal:** Hypothesis testing via rewind in scrimmage.

- `game:save_checkpoint` / `state/rollback` in GameManager
- GameManager manages savestate files + script/macro state for reconstruction
- Chronicle branch management for alternative timelines
- Module revert interface for ephemeral state

**Deliverable:** Analyst-driven rewinds produce actionable improvements.

### Phase 5: Tournament Play (Weeks 19-22)

**Goal:** Compete against other AIs and human players.

- Lobby protocol in GameManager (ported from Chobby + yylobby to Rust)
- Bootstrap widget for `/aicontrol` in competitive multiplayer
- Opponent model artifacts from pre-game reconnaissance
- Artifact locking during tournament games
- Pre/post-game chat via `lobby:chat`

**Deliverable:** Win rate against specific opponents improves across games.

### Phase 6: Scaling and Evaluation (Weeks 23+)

- Progressive opponent difficulty
- Human opponent testing
- Full metrics dashboard
- Stress testing under load
- Community engagement

---

## Evaluation Metrics

**Primary:** Tournament win rate over time, by opponent type/difficulty.

**Learning Quality:**
- Artifact stability (are documents converging or churning?)
- Compilation depth (fraction of decisions handled by macros/compiled Rust)
- Scrimmage-to-tournament transfer (do learnings hold up?)
- Rewind efficiency (insights per rewind)

**Efficiency:**
- Inference budget per game (trending down as more compiles to lower tiers)
- Context utilization (are agents loading useful artifacts?)
- Decision attribution accuracy (does Analyst correctly identify key decisions?)

**Adaptation:**
- Games to positive win rate against new opponent
- Opponent model accuracy (predicted vs actual behavior)

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| **Credit assignment** | Decision journal with full trace; Analyst has replay REPL |
| **Artifact sprawl** | Librarian agent; deprecation rules; confidence decay |
| **Macro bugs** | Testing sandbox; replay-based validation before deployment |
| **Inference latency** | Compilation pipeline; macros handle time-critical execution |
| **Opponent diversity** | Multiple AI opponents; eventually human testing |
| **Context overflow** | Tiered loading; Librarian curation; compression strategies |
| **Rewind abuse** | Budget limits; hypothesis requirement; artifact recording |
| **Game API gaps** | Targeted PRs into game; companion widget for unsynced data |
| **Lua sandbox escape** | Restricted environments; no file/network access from scripts |
| **Frame budget** | Compiled hot path for expensive tools; macro profiling |

---

## Open Questions

1. **Branch isolation:** When rewinding, do all agents see the rewind, or can parallel branches test alternatives simultaneously?

2. **Artifact locking:** How is tournament read-only mode enforced — module flag or separate artifact snapshot?

3. **Macro testing:** Where does sandbox testing happen — separate Spring instance via Lobby, or mocked?

4. **Cross-game persistence:** How much context carries between games in a series against same opponent?

5. **Human opponents:** How do we handle the increased unpredictability and potential for unconventional strategies?

6. ~~**Lobby protocol version**~~ **Resolved**: ZK uses its own text-based TCP protocol (`CommandName JSON\n`) on `zero-k.info:8200`. Already implemented in yylobby (TypeScript).

7. **Savestate reliability:** How reliable are Spring savestates for deterministic replay-from-checkpoint? Any known issues?

8. **Tournament host policies:** Some competitive hosts may set `aiControlFlags` to block `/aicontrol`. Need to survey which hosts do this in practice, and whether tournament organizers would whitelist AI participants.

---

## Afcomech Prerequisites

The following afcomech components need to exist or be extended before this system can be built:

| Component | Status | Needed For |
|-----------|--------|------------|
| Yielding stream architecture | **Merged** (AF + Membrane) | Parallel agents at different tick rates |
| MCPL implementation | **CM integration merged**; spec v0.4 | GameManager communication |
| MCPLModule (generic MCPL client for AF) | Not started | AF ↔ GameManager connection |
| Tick scheduler | Not started | Agent tick rate management |
| Module revert interface | Not started | Rewind-consistent state management |
| Multi-agent inference | Not started (unblocked by yielding stream) | Multiple simultaneous agent streams |
| Artifact store module | Not started | Versioned knowledge persistence |
| Decision journal module | Not started | Credit assignment and analysis |

---

## Notes

This proposal is deliberately ambitious but phased. Each phase produces a working system:

- Phase 0: GameManager + game harness works (also: first real MCPL client/server pair)
- Phase 1: Agent can play and learn
- Phase 2: Agents can specialize and collaborate
- Phase 3: Macros compile decisions to frame rate
- Phase 4: Can test hypotheses via rewind
- Phase 5: Can compete in tournaments
- Phase 6: Can compete seriously

The system is designed to be observable. Every decision is logged with reasoning. Every artifact has history. You can watch the agents develop understanding — or watch them develop *misunderstanding* and debug why.

The ICL advantage is key: this system can adapt to new units, new opponents, and new strategies without retraining. A balance patch doesn't break it — just update the unit stats in context. A new opponent doesn't require thousands of games — just a few to build an opponent model.

The goal isn't to beat AlphaStar. It's to build a system that learns the way we wish AI systems learned: by writing things down, testing hypotheses, and building tools for itself.
