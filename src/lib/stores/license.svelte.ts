// SPDX-License-Identifier: Apache-2.0
// ImpForge License Store — Svelte 5 Runes

import { invoke } from '@tauri-apps/api/core';

export type Tier = 'community' | 'pro' | 'enterprise';

export interface LicenseInfo {
  tier: Tier;
  email: string;
  devices: number;
  expires: string;
  valid: boolean;
}

let tier = $state<Tier>('community');
let info = $state<LicenseInfo | null>(null);
let loading = $state(false);
let error = $state<string | null>(null);

export const license = {
  get tier() { return tier; },
  get info() { return info; },
  get loading() { return loading; },
  get error() { return error; },
  get isPro() { return tier === 'pro' || tier === 'enterprise'; },
  get isEnterprise() { return tier === 'enterprise'; },
  get isCommunity() { return tier === 'community'; },

  async load() {
    loading = true;
    error = null;
    try {
      const result = await invoke<string>('billing_get_tier');
      tier = result as Tier;
      info = await invoke<LicenseInfo | null>('billing_get_license');
    } catch (e) {
      tier = 'community';
      info = null;
    } finally {
      loading = false;
    }
  },

  async activate(key: string): Promise<boolean> {
    loading = true;
    error = null;
    try {
      await invoke('billing_activate_license', { licenseKey: key });
      await this.load();
      return true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      return false;
    } finally {
      loading = false;
    }
  },

  reset() {
    tier = 'community';
    info = null;
    error = null;
  }
};
