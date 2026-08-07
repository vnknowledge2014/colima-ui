export interface ConfirmOptions {
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  variant?: "danger" | "warning" | "info";
}

export const confirmState = $state({
  open: false,
  options: { message: "" } as ConfirmOptions,
  resolve: null as ((value: boolean) => void) | null,
});

export function confirm(options: ConfirmOptions | string): Promise<boolean> {
  const opts = typeof options === "string" ? { message: options } : options;
  return new Promise((resolve) => {
    confirmState.options = opts;
    confirmState.resolve = resolve;
    confirmState.open = true;
  });
}
