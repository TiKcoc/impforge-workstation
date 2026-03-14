# Adaptive Onboarding Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add user profiling (role + experience) to the onboarding wizard so the UI adapts to each user type — beginners see 4 buttons, experts see all 13 modules.

**Architecture:** Extend existing OnboardingWizard from 4→6 steps. Add `userRole` and `userExperience` to AppSettings. Filter the `activities` array in +layout.svelte via `$derived` based on profile. Module visibility stored in settings store, changeable anytime.

**Tech Stack:** Svelte 5 runes ($state, $derived), tauri-plugin-store, existing BenikUI styling

---

### Task 1: Extend Settings Store with Profile Fields

**Files:**
- Modify: `src/lib/stores/settings.svelte.ts`

**Step 1: Add profile types and fields to AppSettings**

Add to the interface after `onboardingComplete`:
```typescript
// User Profile (Adaptive Onboarding)
userRole: 'developer' | 'office' | 'freelancer' | 'manager' | 'marketing' | 'student' | 'entrepreneur' | 'custom' | '';
userExperience: 'beginner' | 'intermediate' | 'expert' | '';
```

Add to DEFAULT_SETTINGS:
```typescript
userRole: '',
userExperience: '',
```

Add cases in loadSettings switch:
```typescript
case 'userRole':
    settings[key] = value as AppSettings['userRole'];
    break;
case 'userExperience':
    settings[key] = value as AppSettings['userExperience'];
    break;
```

**Step 2: Add module visibility helper**

Export a new function:
```typescript
export function getVisibleModules(): string[] {
    const role = settings.userRole;
    const MODULE_MAP: Record<string, string[]> = {
        developer: ['home','chat','github','docker','ide','agents','ai','browser','evaluation','news','settings'],
        office: ['home','chat','news','settings'],
        freelancer: ['home','chat','browser','news','settings'],
        manager: ['home','chat','agents','evaluation','news','settings'],
        marketing: ['home','chat','browser','news','settings'],
        student: ['home','chat','ai','news','settings'],
        entrepreneur: ['home','chat','agents','browser','news','settings'],
        custom: ['home','chat','github','docker','n8n','ide','agents','evaluation','ai','browser','news','settings'],
    };
    if (!role || role === 'custom') return MODULE_MAP.custom;
    return MODULE_MAP[role] ?? MODULE_MAP.custom;
}
```

---

### Task 2: Add Profiling Steps to OnboardingWizard

**Files:**
- Modify: `src/lib/components/OnboardingWizard.svelte`

Extend from 4 to 6 steps:
- Step 0: Welcome (unchanged)
- Step 1: **NEW** — "What describes you best?" (role selection)
- Step 2: **NEW** — "How experienced are you with AI?" (experience level)
- Step 3: AI Setup (was step 1)
- Step 4: Integrations (was step 2)
- Step 5: Ready (was step 3, now shows profile summary)

Step 1 UI: 8 role cards in a 2x4 grid, each with icon + label, click to select.
Step 2 UI: 3 experience cards (Beginner/Intermediate/Expert) with descriptions.

Save selections via `saveSetting('userRole', selectedRole)` on next/finish.

---

### Task 3: Filter Navigation in Layout

**Files:**
- Modify: `src/routes/+layout.svelte`

Replace static `activities` array with a `$derived` filtered version:
```typescript
import { getVisibleModules } from '$lib/stores/settings.svelte';

const allActivities = [
    { id: 'home', ... }, { id: 'chat', ... }, ...all 11...
];

let visibleActivities = $derived(
    allActivities.filter(a => getVisibleModules().includes(a.id))
);
```

Use `visibleActivities` instead of `activities` in the sidebar rendering.

---

### Task 4: Commit and verify

Run: `cargo check && cargo test --workspace`
Commit with conventional commit message.
Push to dev branch.
