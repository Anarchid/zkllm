# Zero-K Agent Fleet: Implementation Proposal

## Overview

A multi-agent system that learns to play Zero-K through externalized cognition — writing and refining tools, documents, and policies across rewindable training games, then deploying accumulated knowledge in non-rewindable tournament play. The goal is to combine the sample-efficiency of in-context learning with robust mechanical performance, without requiring traditional weight-training self-play.

## Core Thesis

Traditional game-playing AI learns by updating weights over millions of games. LLMs with ICL can reason about strategy from very few examples, but suffer from context limits, slow inference, and fragile memory. This system bridges the gap by having agents **externalize their learning into persistent artifacts** — tools, analytical documents, opponent models, and execution macros — that survive across games and rewinds.

The agent fleet doesn't learn by getting better weights. It learns by building better infrastructure for itself.

---

## Architecture

### 1. Agent Fleet

A collection of specialized agents collaborating through shared artifacts and a message bus. Each agent has a defined role but can propose changes to any artifact.

| Agent | Role | Operates During |
|---|---|---|
| **Strategist** | High-level game plan, build order selection, adaptation | Real-time (slow tick) |
| **Tactician** | Micro-level unit control decisions, engagement calls | Real-time (fast tick) |
| **Economist** | Resource flow optimization, expansion timing | Real-time (medium tick) |
| **Analyst** | Post-action review, hypothesis generation, artifact updates | Between games / on rewind |
| **Toolsmith** | Writes and refines Lua widgets, macros, analytical utilities | Between games / on rewind |
| **Librarian** | Manages artifact organization, resolves conflicts, maintains indices | Continuous |

The tick rates matter. The Strategist doesn't need to re-evaluate every frame — maybe every 10–30 seconds. The Tactician needs faster decisions but can delegate to compiled macros for moment-to-moment execution. This layering keeps inference costs manageable.

### 2. Artifact Store

All learning is externalized into a versioned artifact repository. Artifact types:

**Strategic Documents**
- Opponent model (tendencies, likely responses to specific pressures)
- Build order playbook (with conditions for selection)
- Map analysis templates (terrain features, choke points, expansion spots)
- Unit matchup tables (with confidence levels and evidence citations)

**Policies**
- Decision trees for common situations ("if floating metal > X and enemy air scouted, then...")
- Engagement rules (when to fight, when to retreat, at what force ratios)
- Exploration directives (what to test next in scrimmage)

**Tools (Lua Widgets / Gadgets)**
- Scouting automation (patrol patterns, threat detection)
- Kiting macros (per-unit-pair, with optimal range parameters)
- Economy dashboards (parsed into agent-readable summaries)
- Terrain analysis utilities (line-of-sight calculations, pathing estimates)
- Game state serializer (for rewind checkpointing)

**Meta-Artifacts**
- Learning log (timestamped insights with confidence and evidence)
- Open questions queue (hypotheses to test in next scrimmage)
- Performance tracking (win rates, resource efficiency trends, per-strategy outcomes)

All artifacts are versioned. When an artifact update degrades tournament performance, the system can roll back.

### 3. Game Harness

The interface between the agent fleet and the Spring engine.

**State Reader:** Extracts game state into structured representations the agents can consume. Unit positions, health, economy stats, fog-of-war boundaries, terrain. Runs at high frequency; agents sample it at their own tick rates.

**Command Injector:** Translates agent decisions into Spring engine commands. Move, attack, build, patrol, etc. Also executes Lua widget activations.

**Savestate Manager:** Creates and restores Spring engine savestates for rewind functionality. Maintains a timeline of checkpoints with metadata (game clock, agent notes, reason for checkpoint).

**Replay Logger:** Records full game telemetry for post-game analysis. Every command issued, every state snapshot, every agent decision with its reasoning.

---

## Training Loop

### Phase 1: Scrimmage (Rewindable)

```
objective: maximize learning artifacts produced
opponent: built-in AI (progressive difficulty) or self-play

loop:
    create_checkpoint(t=0)
    
    play_game:
        at each decision point:
            load relevant artifacts into context
            decide action (with explicit reasoning trace)
            log decision rationale + artifact references
        
        on interesting_event (unexpected outcome, novel situation):
            create_checkpoint()
            flag for analyst review
        
        on death / loss of major engagement:
            analyst reviews what happened
            generates hypotheses
            optionally rewind to pre-engagement checkpoint
            test alternative approach
            compare outcomes
            update artifacts with findings
    
    post_game:
        analyst conducts full review
        toolsmith implements/refines tools based on identified needs
        librarian organizes new artifacts, resolves conflicts
        strategist updates playbook
        queue open questions for next scrimmage
```

**Rewind Protocol:**
- Rewind is for **hypothesis testing**, not outcome fishing.
- Every rewind must be accompanied by: (1) a hypothesis, (2) an alternative action, (3) a commitment to record findings regardless of outcome.
- Rewinds that don't produce artifact updates are flagged.
- Budget: limited rewinds per game to prevent degenerate loops.

### Phase 2: Tournament (Non-Rewindable)

```
objective: win
opponent: same as scrimmage tier, or human opponents

loop:
    play_game:
        artifacts are read-only during the game
        no rewinds available
        full decision logging continues
    
    post_game:
        compare performance to scrimmage expectations
        identify gaps between "what we thought we knew" and "what worked"
        update artifact confidence levels
        flag artifacts that didn't transfer
```

### Phase 3: Curriculum Scheduling

```
early training:    90% scrimmage / 10% tournament
mid training:      60% scrimmage / 40% tournament
late training:     30% scrimmage / 70% tournament

triggers to shift back toward scrimmage:
    - tournament win rate plateaus
    - new opponent strategy encountered with no counter
    - major artifact revision needed
```

---

## The Compilation Pipeline

A key mechanism: **strategic insights get progressively compiled into faster execution layers.**

```
                    Slow (LLM inference)
                         │
    Observation:  "Scorchers seem weak to Lotuses"
                         │
    Hypothesis:   "Lotus placement at choke X counters 
                   Scorcher raids at cost ratio ~3:1"
                         │
    Tested:        Confirmed over 5 scrimmage rewinds
                         │
    Policy:       "If scouting detects Scorcher-heavy build,
                   pre-place Lotuses at choke points"
                         │
    Macro:         Lua widget auto-places Lotuses at 
                   identified choke points on trigger
                         │
                    Fast (no inference needed)
```

This mirrors how humans develop game skill: conscious analysis → deliberate practice → muscle memory. Each layer up is slower but more flexible; each layer down is faster but more rigid.

The agents should be encouraged to push learnings down this stack whenever confidence is high, and to pull them back up (reverting to deliberate reasoning) when encountering novel situations where the compiled response might not apply.

---

## Context Management Strategy

The biggest operational challenge: what does each agent load into context, and when?

**Tiered Context Loading:**

| Priority | Always loaded | Contents |
|---|---|---|
| Tier 0 | Yes | Agent role description, current game state summary, active objectives |
| Tier 1 | Yes | Relevant policies for current game phase (early/mid/late) |
| Tier 2 | On demand | Unit matchup tables, opponent model, map analysis |
| Tier 3 | On trigger | Specific tool documentation, historical examples, edge cases |

**The Librarian's Role:** Maintains an index of all artifacts with summaries and relevance tags. Other agents query the Librarian to decide what to load. This prevents the "I forgot which ladders I visited" problem — the Librarian is a dedicated context management agent.

**Decision Journaling:** Every significant decision is logged with: timestamp, game state hash, loaded context, reasoning, chosen action, expected outcome. This creates the trace needed for credit assignment in post-game analysis.

---

## Interface with Spring Engine

### Lua Widget Layer

A set of custom Lua widgets running inside the Spring engine that expose:

- **`agentbridge_state`** — Serializes current game state to JSON at configurable intervals. Unit data, economy, map control, fog status.
- **`agentbridge_command`** — Accepts commands from the agent fleet via a local socket or file-based IPC. Validates and executes them.
- **`agentbridge_savestate`** — Wraps Spring's savestate functionality with metadata tagging. Creates named checkpoints that the harness can restore.
- **`agentbridge_macro`** — Hosts compiled macros written by the Toolsmith. Exposes them as callable functions with parameter binding.

### IPC Mechanism

Candidates:
- **Local TCP socket** — lowest latency, most flexible. Preferred.
- **Shared file** — simpler, good enough for slower tick rates. Fallback.
- **Named pipe** — middle ground.

The harness should abstract over this so agents don't care about transport.

---

## Evaluation and Metrics

**Primary:** Tournament win rate over time, broken down by opponent type/difficulty.

**Secondary:**
- Artifact count and churn (are documents stabilizing or constantly rewritten?)
- Compilation depth (what fraction of decisions are handled by macros vs. live inference?)
- Scrimmage-to-tournament transfer ratio (do scrimmage learnings hold up?)
- Rewind efficiency (insights produced per rewind)
- Inference budget per game (total LLM calls, trending down as more gets compiled)

**Diagnostic:**
- Decision attribution accuracy (did the post-game analyst correctly identify which decisions mattered?)
- Context loading relevance (did agents load artifacts they actually used?)

---

## Risks and Open Questions

**Credit Assignment.** The hardest unsolved problem. A 30-minute game has hundreds of decisions. Which ones caused the loss? The decision journal helps, but the Analyst agent still needs to be good at causal reasoning over long chains. This likely needs iteration on the Analyst's prompting and possibly structured frameworks for post-game review.

**Artifact Sprawl.** Without discipline, the artifact store becomes a junk drawer. The Librarian agent is critical. It may need explicit policies: deprecation rules, confidence decay, mandatory consolidation passes.

**Macro Correctness.** Lua tools written by an LLM will have bugs. The Toolsmith needs a testing harness — ideally a sandbox Spring instance where widgets can be validated before deployment. Silent widget failures during tournament play would be catastrophic and hard to diagnose.

**Inference Latency.** Even at reduced tick rates, LLM inference may be too slow for time-critical decisions. The compilation pipeline is the long-term answer, but early games will be mechanically clumsy. Acceptable — the system should be evaluated on its learning trajectory, not initial performance.

**Opponent Diversity.** Self-play risks convergence on a narrow meta. Built-in AIs have predictable patterns. Ideally, the system eventually plays human opponents or multiple different AI opponents to force generalization.

**When to Rewind.** The agents need to develop judgment about what's worth investigating vs. accepting as noise. A lost engagement might be bad luck (RNG, opponent did something unusual) or a genuine knowledge gap. Wasting rewinds on noise slows learning.

---

## Implementation Phases

### Phase 0: Harness (weeks 1–3)
- Spring engine Lua bridge: state reader, command injector, savestate manager
- IPC layer (TCP socket preferred)
- Basic replay logger
- Verify: can an external process read game state and issue commands reliably?

### Phase 1: Single Agent Baseline (weeks 4–6)
- One general-purpose agent (no specialization yet)
- Basic artifact store (file-based, versioned with git)
- Scrimmage loop against easiest built-in AI
- Objective: agent can play a coherent game and write down what it learned
- Verify: artifacts improve across games, agent references past learnings

### Phase 2: Agent Specialization (weeks 7–10)
- Split into Strategist / Tactician / Economist / Analyst / Toolsmith / Librarian
- Implement message bus and artifact access control
- Context management system with tiered loading
- Verify: agents collaborate effectively, specialization produces better artifacts than single agent

### Phase 3: Rewind and Compilation (weeks 11–14)
- Full rewind protocol with hypothesis tracking
- Compilation pipeline (insight → policy → macro)
- Macro testing sandbox
- Tournament/scrimmage scheduling
- Verify: tournament performance improves; compiled macros execute correctly

### Phase 4: Scaling and Evaluation (weeks 15+)
- Progressive opponent difficulty
- Human opponent testing
- Full metrics dashboard
- Artifact quality analysis
- Stress-testing the agent framework under load

---

## Notes

This proposal is deliberately ambitious. Not everything needs to be built — the phased approach means each phase produces a working (if limited) system that can be evaluated independently. Phase 1 alone is a useful proof of concept: can an LLM agent learn to play Zero-K better over time by writing things down?

The system is also designed to be interesting to observe. Every decision is logged with reasoning. Every artifact has a history. You can watch the agents develop understanding — or watch them develop *misunderstanding* and debug why. This is as much a research instrument as a game-playing system.
