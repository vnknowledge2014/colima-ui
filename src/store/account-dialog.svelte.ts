/**
 * Drives the single global SignInPanel, mirroring `upgrade.svelte.ts`.
 *
 * Any surface — the sidebar badge, Settings → Account — opens sign-in by calling
 * `showSignIn()`, so the panel is mounted once in `App.svelte` rather than
 * duplicated per call site.
 *
 * The panel is an offer, never a gate: nothing waits on it, and closing it
 * leaves the app fully usable.
 */
export const accountDialogState = $state({
  open: false,
});

export function showSignIn(): void {
  accountDialogState.open = true;
}

export function closeSignIn(): void {
  accountDialogState.open = false;
}
