<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    step,
    currentStep,
    totalSteps,
    tooltipPos,
    onComplete,
    handlePrev,
    handleNext
  } = $props<{
    step: { title: string; body: string };
    currentStep: number;
    totalSteps: number;
    tooltipPos: { left: number; top: number };
    onComplete: () => void;
    handlePrev: () => void;
    handleNext: () => void;
  }>();
</script>

<div
  class="tour-tooltip"
  style="left: {tooltipPos.left}px; top: {tooltipPos.top}px;"
>
  <div class="tour-tooltip-title">
    {step.title}
  </div>
  <div class="tour-tooltip-body">
    {step.body}
  </div>
  <div class="tour-tooltip-footer">
    <!-- Step dots -->
    <div class="tour-dots">
      {#each Array(totalSteps) as _, i (i)}
        <div class="tour-dot {i === currentStep ? 'active' : i < currentStep ? 'done' : ''}"></div>
      {/each}
    </div>

    <!-- Actions -->
    <div class="tour-tooltip-actions">
      <button
        class="btn btn-ghost"
        style="font-size: var(--text-xs); padding: 4px 10px;"
        onclick={onComplete}
      >
        Skip
      </button>
      {#if currentStep > 0}
        <button
          class="btn btn-ghost"
          style="font-size: var(--text-xs); padding: 4px 10px;"
          onclick={handlePrev}
        >
          ← Back
        </button>
      {/if}
      <button
        class="btn btn-primary"
        style="font-size: var(--text-xs); padding: 4px 12px;"
        onclick={handleNext}
      >
        {#if currentStep < totalSteps - 1}
          Next →
        {:else}
          <Icon name="Check" size={12} style="vertical-align: middle;" /> Finish
        {/if}
      </button>
    </div>
  </div>
</div>
