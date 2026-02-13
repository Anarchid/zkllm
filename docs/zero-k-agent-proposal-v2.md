# Zero-K Agent Fleet: Implementation Proposal v2

**Version:** 2.0
**Date:** February 2026
**Status:** Draft
**Builds on:** Original proposal (v1), afcomech architecture review, MCPL specification

---

## Overview

A multi-agent system that learns to play Zero-K through externalized cognition — writing and refining perception tools, strategic documents, and execution macros across rewindable training games, then deploying accumulated knowledge in non-rewindable tournament play.

The system combines:
- **In-context learning** for sample-efficient adaptation (including to specific opponents)
- **Externalized artifacts** that persist across sessions and survive context limits
- **Compiled macros** that execute at game speed without inference latency
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
- **Lua macros** handle per-frame execution (GOFAI-style, fast)
- **LLM agents** handle narrative reasoning and strategic decisions (slow, but powerful)

### The ICL Advantage

In-context learning enables adaptation that weight-trained AI cannot match:
- Adapt to a specific opponent's tendencies *within a single session*
- Incorporate new unit stats after a balance patch without retraining
- Learn from a handful of examples rather than millions of games
- Explain decisions in natural language for debugging

A weight-trained bot that doesn't know what an Odin is will never learn without retraining. An ICL agent just needs the unit stats in context.

---

## Architecture

### System Topology

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Agent Framework (MCPL Host)                          │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                         Event Loop                                  │ │
│  │   ProcessQueue ──► Module Dispatch ──► State Updates                │ │
│  │        ▲                                                            │ │
│  │        │ tick events, push events, tool results                     │ │
│  └────────┼────────────────────────────────────────────────────────────┘ │
│           │                                                              │
│  ┌────────┴────────────────────────────────────────────────────────────┐ │
│  │                      Tick Scheduler                                  │ │
│  │   Strategist (0.03-0.05 Hz) ──► tick:strategist                     │ │
│  │   Economist  (0.1 Hz)       ──► tick:economist                      │ │
│  │   Tactician  (0.1-0.2 Hz)   ──► tick:tactician                      │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                    Agent Fleet (via Membrane)                        │ │
│  │                                                                      │ │
│  │  REAL-TIME (during game)              POST-HOC (replay/between)     │ │
│  │  ┌───────────┐ ┌───────────┐          ┌───────────┐ ┌───────────┐   │ │
│  │  │Strategist │ │ Tactician │          │  Analyst  │ │ Toolsmith │   │ │
│  │  │ (0.05 Hz) │ │ (0.2 Hz)  │          │  (async)  │ │  (async)  │   │ │
│  │  └───────────┘ └───────────┘          └───────────┘ └───────────┘   │ │
│  │  ┌───────────┐                        ┌───────────┐                  │ │
│  │  │ Economist │                        │ Librarian │                  │ │
│  │  │ (0.1 Hz)  │                        │(continuous)│                 │ │
│  │  └───────────┘                        └───────────┘                  │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                          Modules                                     │ │
│  │  ┌─────────────┐  ┌───────────────┐  ┌────────────┐                 │ │
│  │  │ MCPL Client │  │ ArtifactStore │  │DecisionLog │                 │ │
│  │  │ (to game)   │  │  (Chronicle)  │  │ (Chronicle)│                 │ │
│  │  └──────┬──────┘  └───────────────┘  └────────────┘                 │ │
│  └─────────┼───────────────────────────────────────────────────────────┘ │
│            │                                                             │
└────────────┼─────────────────────────────────────────────────────────────┘
             │ JSON-RPC / socket (MCPL protocol)
             ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    SkirmishAI.so (MCPL Server)                           │
│                    Running inside Spring Engine                          │
│                                                                          │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                    Lua Script Environment                          │  │
│  │                                                                    │  │
│  │  Agent-written scripts:        Bootstrapped from existing AIs:    │  │
│  │  ├── perception/               ├── perception/threat.lua (CAI)    │  │
│  │  │   └── custom_radar.lua      ├── macros/kiting.lua (CAI)        │  │
│  │  ├── macros/                   └── analysis/engagement.lua        │  │
│  │  │   └── expansion_defense.lua                                    │  │
│  │  └── strategies/                                                  │  │
│  │      └── anti_scorcher.lua                                        │  │
│  │                                                                    │  │
│  │  Active macros (running every frame):                             │  │
│  │  • macro_12: kite_vs_heavy(group_a, params={retreat_hp: 40})     │  │
│  │  • macro_15: patrol_route(raiders, waypoints=[...])              │  │
│  │                                                                    │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                                                          │
│  Frame loop (30 Hz):                                                     │
│  1. Tick all active macros (pure Lua, fast)                             │
│  2. Check for incoming tool calls from agents (MCPL)                    │
│  3. Emit push events for significant game events (MCPL)                 │
│  4. Respond to context hook requests (MCPL)                             │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Game Bridge Protocol

The SkirmishAI.so needs bidirectional communication with the Agent Framework:

| Capability | Use |
|------------|-----|
| **Push Events** | Game notifies agents of engagements, economy thresholds, scouting intel |
| **Context Hooks** | Inject game state summary before inference; log decisions after |
| **Permissions** | Per-agent access control (Tactician gets commands, Analyst gets replay only) |
| **State Rollback** | Checkpoint/restore for rewind-based learning |
| **Scoped Access** | Whitelist which command types each agent can issue |

**Protocol decision deferred.** Two options under consideration:

1. **MCPL** (MCP Live) — A draft extension to MCP that adds push events, context hooks, and state management. Provides a standard protocol with built-in permission model. However, MCPL is currently spec-only with no implementation; we'd be writing it from scratch.

2. **Custom protocol** — Purpose-built for game integration. Potentially simpler, but no ecosystem benefits.

This decision can wait until after the Agent Framework's yielding stream rework is complete. The game harness design is protocol-agnostic at the architectural level — what matters is the *capabilities* (push events, context hooks, rollback), not the wire format.

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

These rates are *fine* because agents aren't doing per-frame micro — they're orchestrating macros and making strategic judgments.

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
- Debugs failed macros by examining execution traces

**Librarian** (on-demand, 0.2-0.5 Hz)
- Maintains artifact indices and summaries
- Resolves conflicts between artifact versions
- Answers queries: "What's our current build order for anti-air?"
- Manages context loading: decides what each agent should see

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

Instead of "game state → context", agents use:

```
Game State (raw, huge, inaccessible to LLM)
       │
       ▼
┌──────────────────────────────────────────────┐
│  Agent-Written Perception Tools (Lua)         │
│  Running in SkirmishAI.so                     │
│                                               │
│  • threat_assessment(sector) → summary        │
│  • army_composition() → structured data       │
│  • economy_snapshot() → key metrics           │
│  • infer_enemy_intent(group) → prediction     │
└──────────────────────────────────────────────┘
       │
       ▼
Structured Summaries (small, semantic, fits context)
       │
       ▼
LLM Agent
```

The agents never see raw state. They call perception tools that return summaries. The tools themselves become artifacts that agents improve over time.

### Bootstrap from Existing AIs

Zero-K has mature AI codebases with excellent perception and micro code:

| Source AI | What to Extract |
|-----------|-----------------|
| CAI (Circuit AI) | Threat assessment, unit grouping, economy management, combat micro |
| CircuitAI | Refined versions of CAI systems, engagement logic |
| ZKGBAI | Build order framework, expansion logic, strategic planning |

Day one, we load perception and micro routines from these AIs as the initial toolset. Agents can use them immediately, then gradually improve or replace components.

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

**Effectors** (execute actions)
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

-- Macro runs every frame in Lua, no LLM involved
-- Terminates when conditions met or agent deactivates
```

### Replay REPL (Analyst/Toolsmith Only)

Post-game, the Analyst and Toolsmith get interactive query access:

```lua
-- Analyst investigating a lost engagement
replay_scrub(frame=7200)
replay_query("get_units_in_area(2400, 1800, 600)")
  → { friendly: [...], enemy: [...] }

replay_query("track_unit_count('Scorcher', 0, 7200)")
  → [(0, 0), (1800, 2), (3600, 5), (5400, 8), (7200, 12)]

-- Toolsmith testing a new perception function
replay_query("test_threat_v2(2400, 1800, 600)")
  → { level: 580, ... }  -- compare to actual outcome
```

This enables iterative investigation that pre-built tools can't anticipate.

---

## The Tactician's Role: Narrative Reasoning

The Tactician's unique contribution is **narrativizing event streams** — turning raw observations into actionable conclusions through temporal and intentional reasoning.

### Example: The Expansion Dilemma

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
Net outcome if I finish: lose builder (180m) + lotus (100m), kill 2 scorchers (180m) = -100m
Net outcome if I abort: lose sunk cost (70m) = -70m

Better to abort. Cloak the builder, save 110 metal."
```

This requires:
1. Projecting enemy movement (intent inference)
2. Comparing timelines (temporal reasoning)
3. Evaluating counterfactuals (if I do X vs Y)
4. Making a judgment call under uncertainty

GOFAI cannot do this. LLMs can.

### The Decision Space

At 0.1-0.2 Hz (one decision every 5-10 seconds), the Tactician manages:

**Macro Lifecycle**
- Activate: "Start kiting macro for group A against that blob"
- Parameterize: "Increase retreat threshold, they have too much burst"
- Terminate: "Threat passed, release the group for other duties"
- Replace: "Kiting isn't working, switch to hit-and-run"

**Attention Routing**
- Subscribe to relevant events (engagements in sector B)
- Ignore irrelevant events (economy alerts — that's Economist's job)
- Set priority sectors for perception queries

**Intervention**
- Override macro: "Retreat NOW, I see reinforcements incoming"
- Emergency stop: Prevent macro from walking into obvious trap
- Manual command: When no macro fits the situation

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

**Tools (Lua Scripts)**
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

## The Compilation Pipeline

Strategic insights progressively compile into faster execution layers:

```
                    Slow (LLM inference, 5-30 seconds)
                         │
    Observation:  "Scorchers seem weak against Lotus at chokes"
                         │
    Hypothesis:   "Lotus at choke X counters Scorcher raids at ~3:1 cost"
                         │
    Tested:        Confirmed over 5 scrimmage rewinds
                         │
    Policy:       "If scouting detects Scorcher-heavy, pre-place Lotus at chokes"
                         │
    Macro:         Lua script auto-places Lotus on trigger condition
                         │
                    Fast (frame-rate Lua, no inference needed)
```

Each layer up: slower, more flexible, handles novel situations.
Each layer down: faster, more rigid, handles known situations.

Agents should:
- Push learnings down the stack when confidence is high
- Pull them back up when encountering novelty that compiled responses can't handle

---

## Rewind Protocol

### Purpose

Rewind is for **hypothesis testing**, not outcome fishing. Every rewind produces learning artifacts regardless of whether the alternative succeeded.

### Mechanism (via MCPL)

```
1. Create checkpoint
   SkirmishAI: state/response with checkpoint: "chk_7200_preengagement"
   Chronicle: creates branch point

2. Game continues, outcome observed
   Original timeline: loss at frame 9000

3. Analyst reviews, generates hypothesis
   "We should have retreated instead of engaging"

4. Restore checkpoint (MCPL state/rollback)
   SkirmishAI: restores Spring savestate
   Chronicle: switches to new branch
   Modules: receive onRevert(), reset ephemeral state

5. Play alternative
   Execute alternative strategy
   Observe outcome

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

- **Limited budget**: N rewinds per game to prevent degenerate loops
- **Hypothesis required**: Every rewind must have a stated hypothesis
- **Recording required**: Outcomes recorded regardless of success
- **No outcome fishing**: Can't rewind just because you lost

---

## Training Loop

### Phase 1: Scrimmage (Rewindable)

```
objective: maximize learning artifacts produced
opponent: built-in AI (progressive difficulty) or self-play

loop:
    create_checkpoint(t=0)

    play_game:
        at each agent tick:
            load relevant artifacts via Librarian
            query perception tools
            decide action (with explicit reasoning trace)
            log decision to DecisionJournal

        on significant_event (unexpected outcome, novel situation):
            create_checkpoint()
            flag for Analyst review

        on major_loss (engagement, expansion, etc.):
            Analyst reviews what happened (replay REPL)
            generates hypothesis
            optionally rewind to checkpoint
            test alternative approach
            compare outcomes
            update artifacts with findings

    post_game:
        Analyst conducts full review
        Toolsmith implements/refines tools based on identified needs
        Librarian organizes new artifacts, resolves conflicts
        Strategist updates playbook
        queue open questions for next scrimmage
```

### Phase 2: Tournament (Non-Rewindable)

```
objective: win
opponent: same tier or human opponents

loop:
    play_game:
        artifacts are read-only during game
        no rewinds available
        full decision logging continues

    post_game:
        compare performance to scrimmage expectations
        identify "what we thought we knew" vs "what actually worked"
        update artifact confidence levels
        flag artifacts that didn't transfer to tournament
```

### Phase 3: Curriculum Scheduling

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

This adaptation happens in minutes/hours, not weeks of retraining. A weight-trained bot cannot do this.

---

## Context Management

### The Challenge

Each agent needs relevant information in context, but context is limited and inference is slow. Loading everything wastes tokens; loading nothing makes agents blind.

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

Other agents query the Librarian rather than searching artifacts directly. This prevents "forgot which artifacts I read" problems.

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

Post-game, Analyst can trace decisions back to see:
- What information the agent had
- What reasoning it applied
- Whether the outcome matched expectations
- Which artifacts influenced the decision

---

## Protocol Details (If MCPL)

*This section describes how integration would work if MCPL is chosen. If a custom protocol is used instead, these capabilities would be implemented with different wire formats but equivalent semantics.*

### Feature Sets

```jsonc
{
  "featureSets": {
    // Perception (all agents)
    "game.perception.*": {
      "description": "Query game state summaries",
      "uses": ["tools"]
    },

    // Commands (role-restricted)
    "game.commands.strategic": {
      "description": "Build priorities, expansion orders",
      "uses": ["tools"],
      "scoped": true
    },
    "game.commands.tactical": {
      "description": "Unit movement and combat orders",
      "uses": ["tools"],
      "scoped": true
    },

    // Macros
    "game.macros.*": {
      "description": "Activate and manage Lua macros",
      "uses": ["tools"]
    },

    // Replay (Analyst/Toolsmith only)
    "game.replay.*": {
      "description": "REPL access to replay state",
      "uses": ["tools"]
    },

    // Events
    "game.events.*": {
      "description": "Push notifications for game events",
      "uses": ["pushEvents"]
    },

    // Context hooks
    "game.context.state": {
      "description": "Inject game summary before inference",
      "uses": ["contextHooks.beforeInference"]
    },
    "game.context.decisions": {
      "description": "Log decisions after inference",
      "uses": ["contextHooks.afterInference"]
    }
  }
}
```

### Per-Agent Permissions

| Agent | Feature Sets Enabled |
|-------|---------------------|
| Strategist | perception.*, commands.strategic, macros.*, events.*, context.* |
| Tactician | perception.*, commands.tactical, macros.*, events.engagement, context.* |
| Economist | perception.economy, commands.strategic (build only), events.economy |
| Analyst | perception.*, replay.*, context.decisions |
| Toolsmith | perception.*, replay.*, macros.manage |
| Librarian | (no game access, operates on artifacts only) |

### State Management

The game server uses `hostState: false` — game state is too large for the host to manage. Checkpoints are opaque references that the game interprets.

```jsonc
// Tool response includes checkpoint
{
  "result": {
    "content": [...],
    "state": {
      "checkpoint": "chk_7200",
      "parent": "chk_5000"
    }
  }
}

// Rollback request
{
  "method": "state/rollback",
  "params": {
    "featureSet": "game.commands.strategic",
    "checkpoint": "chk_7200"
  }
}
```

---

## Implementation Phases

### Phase 0: Game Harness (Weeks 1-3)

**Goal:** External process can read game state and issue commands reliably.

- Implement SkirmishAI.so as MCPL server
- Basic tool set: perception queries, command execution
- IPC via JSON-RPC over TCP socket
- Verify: Claude Code can play a simple game via tool calls

**Deliverable:** Manual play test where human issues commands through MCPL interface.

### Phase 1: Single Agent Baseline (Weeks 4-6)

**Goal:** One general agent can play coherently and learn across games.

- Single agent (no specialization) with basic artifacts
- Perception tools bootstrapped from CAI
- File-based artifact store (versioned with Chronicle)
- Scrimmage loop against easiest AI

**Deliverable:** Agent references past learnings, artifacts improve across games.

### Phase 2: Agent Specialization (Weeks 7-10)

**Goal:** Specialized agents collaborate effectively.

- Split into Strategist / Tactician / Economist / Analyst / Toolsmith / Librarian
- Implement tick scheduler for different rates
- Context management with tiered loading
- Feature set permissions per agent

**Deliverable:** Specialization produces better artifacts than single agent.

### Phase 3: Rewind and Macros (Weeks 11-14)

**Goal:** Hypothesis testing via rewind; compiled execution via macros.

- Savestate integration (checkpoint/restore)
- Full rewind protocol with hypothesis tracking
- Macro activation/deactivation system
- Macro testing sandbox

**Deliverable:** Tournament performance improves; macros execute correctly.

### Phase 4: Opponent Adaptation (Weeks 15-18)

**Goal:** Adapt to specific opponents within a session.

- Opponent model artifacts
- Cross-game learning against same opponent
- Counter-strategy selection

**Deliverable:** Win rate against specific opponent improves across games.

### Phase 5: Scaling and Evaluation (Weeks 19+)

- Progressive opponent difficulty
- Human opponent testing
- Full metrics dashboard
- Stress testing under load

---

## Evaluation Metrics

**Primary:** Tournament win rate over time, by opponent type/difficulty.

**Learning Quality:**
- Artifact stability (are documents converging or churning?)
- Compilation depth (fraction of decisions handled by macros)
- Scrimmage-to-tournament transfer (do learnings hold up?)
- Rewind efficiency (insights per rewind)

**Efficiency:**
- Inference budget per game (trending down as more compiles)
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

---

## Open Questions

1. **Branch isolation:** When rewinding, do all agents see the rewind, or can parallel branches test alternatives simultaneously?

2. **Artifact locking:** How is tournament read-only mode enforced — module flag or separate artifact snapshot?

3. **Macro testing:** Where does sandbox testing happen — separate Spring instance, or mocked?

4. **Cross-game persistence:** How much context carries between games in a series against same opponent?

5. **Human opponents:** How do we handle the increased unpredictability and potential for unconventional strategies?

---

## Notes

This proposal is deliberately ambitious but phased. Each phase produces a working system:

- Phase 0: Game harness works
- Phase 1: Agent can play and learn
- Phase 2: Agents can specialize and collaborate
- Phase 3: Can test hypotheses via rewind
- Phase 4: Can adapt to opponents
- Phase 5: Can compete seriously

The system is also designed to be observable. Every decision is logged with reasoning. Every artifact has history. You can watch the agents develop understanding — or watch them develop *misunderstanding* and debug why.

The ICL advantage is key: this system can adapt to new units, new opponents, and new strategies without retraining. A balance patch doesn't break it — just update the unit stats in context. A new opponent doesn't require thousands of games — just a few to build an opponent model.

The goal isn't to beat AlphaStar. It's to build a system that learns the way we wish AI systems learned: by writing things down, testing hypotheses, and building tools for itself.
