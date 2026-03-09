<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ProGate — Feature gate for ImpForge Pro/Enterprise features -->
<script lang="ts">
  import { license } from '$lib/stores/license.svelte';

  interface Props {
    feature: string;
    children: any;
    tier?: 'pro' | 'enterprise';
  }

  let { feature, children, tier = 'pro' }: Props = $props();

  let available = $derived(
    tier === 'enterprise' ? license.isEnterprise : license.isPro
  );

  let tierLabel = $derived(tier === 'enterprise' ? 'ENTERPRISE' : 'PRO');
</script>

{#if available}
  {@render children()}
{:else}
  <div class="flex items-center gap-3 p-4 rounded-lg bg-muted/50 border border-dashed border-muted-foreground/25">
    <span class="inline-flex items-center rounded-md bg-primary/10 px-2.5 py-1 text-xs font-semibold text-primary ring-1 ring-inset ring-primary/20">
      {tierLabel}
    </span>
    <div class="flex flex-col gap-0.5">
      <p class="text-sm font-medium text-muted-foreground">
        {feature}
      </p>
      <p class="text-xs text-muted-foreground/70">
        Upgrade to ImpForge {tier === 'enterprise' ? 'Enterprise' : 'Pro'} to unlock this feature
      </p>
    </div>
  </div>
{/if}
