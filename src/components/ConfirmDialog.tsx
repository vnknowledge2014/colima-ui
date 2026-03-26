import { useState, useCallback } from "react";
import { WarningIcon, BoltIcon, InfoIcon } from "./Icons";

/* ===== ConfirmDialog Component ===== */
interface ConfirmDialogProps {
  open: boolean;
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  variant?: "danger" | "warning" | "info";
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  open,
  title = "Confirm",
  message,
  confirmText = "Confirm",
  cancelText = "Cancel",
  variant = "danger",
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  if (!open) return null;

  const variantColors: Record<string, { bg: string; color: string; border: string }> = {
    danger: { bg: "rgba(248, 81, 73, 0.15)", color: "var(--accent-red)", border: "rgba(248, 81, 73, 0.4)" },
    warning: { bg: "rgba(210, 153, 34, 0.15)", color: "var(--accent-yellow)", border: "rgba(210, 153, 34, 0.4)" },
    info: { bg: "rgba(88, 166, 255, 0.15)", color: "var(--accent-blue)", border: "rgba(88, 166, 255, 0.4)" },
  };

  const v = variantColors[variant] || variantColors.danger;
  const IconComponent = variant === "danger" ? WarningIcon : variant === "warning" ? BoltIcon : InfoIcon;

  return (
    <div
      style={{
        position: "fixed", inset: 0, zIndex: 10000,
        background: "rgba(0, 0, 0, 0.6)", backdropFilter: "blur(4px)",
        display: "flex", alignItems: "center", justifyContent: "center",
      }}
      onClick={(e) => { if (e.target === e.currentTarget) onCancel(); }}
    >
      <div
        style={{
          background: "var(--bg-primary)", borderRadius: "var(--radius-lg)",
          border: "1px solid var(--border-primary)", boxShadow: "0 20px 60px rgba(0,0,0,0.5)",
          padding: "24px", minWidth: "380px", maxWidth: "480px",
          animation: "fadeInScale 0.15s ease-out",
        }}
      >
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: "10px", marginBottom: "16px" }}>
          <span style={{
            fontSize: "20px", width: "36px", height: "36px", borderRadius: "var(--radius-md)",
            background: v.bg, display: "flex", alignItems: "center", justifyContent: "center",
          }}>
            <IconComponent size={18} color={v.color} />
          </span>
          <h3 style={{ margin: 0, fontSize: "var(--text-base)", fontWeight: 600, color: "var(--text-primary)" }}>
            {title}
          </h3>
        </div>

        {/* Message */}
        <p style={{
          margin: "0 0 24px 0", fontSize: "var(--text-sm)", color: "var(--text-secondary)",
          lineHeight: 1.6, whiteSpace: "pre-line",
        }}>
          {message}
        </p>

        {/* Buttons */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px" }}>
          <button
            className="btn btn-ghost"
            onClick={onCancel}
            style={{ fontSize: "var(--text-sm)", padding: "8px 16px" }}
          >
            {cancelText}
          </button>
          <button
            onClick={onConfirm}
            style={{
              fontSize: "var(--text-sm)", padding: "8px 20px", borderRadius: "var(--radius-md)",
              border: `1px solid ${v.border}`, background: v.bg, color: v.color,
              fontWeight: 600, cursor: "pointer", transition: "all 0.15s ease",
            }}
            onMouseEnter={(e) => { e.currentTarget.style.background = v.color; e.currentTarget.style.color = "#fff"; }}
            onMouseLeave={(e) => { e.currentTarget.style.background = v.bg; e.currentTarget.style.color = v.color; }}
          >
            {confirmText}
          </button>
        </div>
      </div>

      <style>{`
        @keyframes fadeInScale {
          from { opacity: 0; transform: scale(0.95); }
          to { opacity: 1; transform: scale(1); }
        }
      `}</style>
    </div>
  );
}

/* ===== useConfirm Hook ===== */
interface ConfirmOptions {
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  variant?: "danger" | "warning" | "info";
}

export function useConfirm() {
  const [state, setState] = useState<{
    open: boolean;
    options: ConfirmOptions;
    resolve: ((value: boolean) => void) | null;
  }>({
    open: false,
    options: { message: "" },
    resolve: null,
  });

  const confirm = useCallback((options: ConfirmOptions | string): Promise<boolean> => {
    const opts = typeof options === "string" ? { message: options } : options;
    return new Promise((resolve) => {
      setState({ open: true, options: opts, resolve });
    });
  }, []);

  const handleConfirm = useCallback(() => {
    state.resolve?.(true);
    setState((s) => ({ ...s, open: false, resolve: null }));
  }, [state.resolve]);

  const handleCancel = useCallback(() => {
    state.resolve?.(false);
    setState((s) => ({ ...s, open: false, resolve: null }));
  }, [state.resolve]);

  const dialogProps = {
    open: state.open,
    title: state.options.title,
    message: state.options.message,
    confirmText: state.options.confirmText,
    cancelText: state.options.cancelText,
    variant: state.options.variant,
    onConfirm: handleConfirm,
    onCancel: handleCancel,
  };

  return { confirm, ConfirmDialogProps: dialogProps };
}
