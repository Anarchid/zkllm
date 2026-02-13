/**
 * WakeModule — Agent-controlled event filtering.
 *
 * Lets the agent specify which SAI events should wake it up.
 * Events that don't match are still delivered to context but
 * don't trigger inference.
 *
 * Conditions persist until the agent calls set_conditions again.
 * The agent should call set_conditions at the end of every
 * think cycle to set what should wake it next.
 */

import type {
  Module,
  ModuleContext,
  ProcessState,
  ProcessEvent,
  EventResponse,
  ToolDefinition,
  ToolCall,
  ToolResult,
} from '@connectome/agent-framework';

interface WakeConditions {
  events: string[];
  timeout_s: number;
}

export class WakeModule implements Module {
  readonly name = 'wake';
  private ctx: ModuleContext | null = null;
  private conditions: WakeConditions | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private lastTriggerTime = 0;
  private static DEBOUNCE_MS = 500;

  /**
   * Callback for MCPLModule's shouldTriggerInference.
   * Arrow function to preserve `this` binding.
   */
  shouldTrigger = (content: string, _metadata: Record<string, unknown>): boolean => {
    // No conditions set (initial state) — trigger on everything
    if (!this.conditions) return true;

    // Debounce: collapse same-frame events into one inference
    const now = Date.now();
    if (now - this.lastTriggerTime < WakeModule.DEBOUNCE_MS) return false;

    // Try to extract SAI event type from the message content.
    try {
      const jsonMatch = content.match(/\{[^]*\}/);
      if (jsonMatch) {
        const parsed = JSON.parse(jsonMatch[0]);
        if (parsed.type && this.conditions.events.includes(parsed.type)) {
          this.lastTriggerTime = now;
          return true;
        }
      }
    } catch {
      // Not JSON — don't trigger
    }

    return false;
  };

  async start(ctx: ModuleContext): Promise<void> {
    this.ctx = ctx;
  }

  async stop(): Promise<void> {
    this.clearTimer();
    this.ctx = null;
  }

  getTools(): ToolDefinition[] {
    return [
      {
        name: 'set_conditions',
        description:
          'Set wake conditions. You will sleep until a matching event arrives or the timeout ' +
          'expires. All events are still recorded — you will see them when you wake up. ' +
          'Conditions persist until you call set_conditions again.',
        inputSchema: {
          type: 'object' as const,
          properties: {
            events: {
              type: 'array',
              items: { type: 'string' },
              description:
                'SAI event types to wake on (e.g. unit_finished, enemy_enter_los, unit_damaged, unit_idle)',
            },
            timeout_s: {
              type: 'number',
              description: 'Maximum sleep duration in seconds (default: 30)',
            },
          },
          required: ['events'],
        },
      },
    ];
  }

  async handleToolCall(call: ToolCall): Promise<ToolResult> {
    const input = call.input as { events: string[]; timeout_s?: number };

    if (!input.events || !Array.isArray(input.events) || input.events.length === 0) {
      return { success: false, error: 'events must be a non-empty array of event type strings' };
    }

    const timeout_s = input.timeout_s ?? 30;

    // Clear previous timer
    this.clearTimer();

    // Set new conditions
    this.conditions = { events: input.events, timeout_s };

    // Start timeout
    this.timer = setTimeout(() => {
      this.timer = null;
      this.ctx?.pushEvent({
        type: 'inference-request',
        agentName: 'commander',
        reason: 'wake-timeout',
        source: 'wake',
        triggerInference: true,
      } as any);
    }, timeout_s * 1000);

    return {
      success: true,
      data: `Sleeping. Will wake on: [${input.events.join(', ')}] or after ${timeout_s}s.`,
    };
  }

  async onProcess(event: ProcessEvent, _state: ProcessState): Promise<EventResponse> {
    // Handle system bootstrap messages
    if (event.type === 'external-message' && event.source === 'system') {
      return {
        addMessages: [{
          participant: 'user',
          content: [{ type: 'text', text: String(event.content) }],
        }],
        requestInference: true,
      };
    }
    return {};
  }

  private clearTimer(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }
}
