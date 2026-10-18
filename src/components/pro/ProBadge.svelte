<script lang="ts">
  import { t } from "../../lib/i18n.svelte";

  // The single "PRO" pill. Every Pro surface uses this rather than restyling a
  // badge inline, so the purple signal stays identical everywhere.
  let { label }: { label?: string } = $props();
</script>

<span class="badge badge-pro">{label ?? t('pro.gate.badge', { default: 'PRO' })}</span>
