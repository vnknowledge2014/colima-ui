import { useEffect, useRef, useCallback } from "react";
import { createPortal } from "react-dom";

export interface ContextMenuItem {
  label: string;
  icon?: React.ReactNode;
  action: () => void;
  danger?: boolean;
  disabled?: boolean;
  divider?: boolean;
}

interface ContextMenuProps {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export default function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const activeIndex = useRef(-1);

  // Viewport boundary adjustment
  const adjustedPos = useRef({ x, y });
  useEffect(() => {
    const el = menuRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    let ax = x, ay = y;
    if (x + rect.width > window.innerWidth - 8) ax = window.innerWidth - rect.width - 8;
    if (y + rect.height > window.innerHeight - 8) ay = window.innerHeight - rect.height - 8;
    if (ax < 8) ax = 8;
    if (ay < 8) ay = 8;
    adjustedPos.current = { x: ax, y: ay };
    el.style.left = `${ax}px`;
    el.style.top = `${ay}px`;
  }, [x, y]);

  // Click outside
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) onClose();
    };
    document.addEventListener("mousedown", handler, true);
    return () => document.removeEventListener("mousedown", handler, true);
  }, [onClose]);

  // Keyboard navigation
  const actionableItems = items.filter((i) => !i.divider && !i.disabled);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") { onClose(); return; }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        activeIndex.current = (activeIndex.current + 1) % actionableItems.length;
        focusItem();
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        activeIndex.current = (activeIndex.current - 1 + actionableItems.length) % actionableItems.length;
        focusItem();
      }
      if (e.key === "Enter" && activeIndex.current >= 0) {
        e.preventDefault();
        actionableItems[activeIndex.current]?.action();
        onClose();
      }
    },
    [actionableItems, onClose],
  );

  const focusItem = () => {
    const el = menuRef.current;
    if (!el) return;
    const btns = el.querySelectorAll<HTMLButtonElement>(".ctx-item:not([disabled])");
    btns.forEach((b, i) => b.classList.toggle("ctx-active", i === activeIndex.current));
  };

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  return createPortal(
    <div className="ctx-overlay">
      <div ref={menuRef} className="ctx-menu" style={{ left: x, top: y }}>
        {items.map((item, i) =>
          item.divider ? (
            <div key={i} className="ctx-divider" />
          ) : (
            <button
              key={i}
              className={`ctx-item${item.danger ? " ctx-danger" : ""}`}
              disabled={item.disabled}
              onClick={() => { item.action(); onClose(); }}
              onMouseEnter={() => { activeIndex.current = -1; focusItem(); }}
            >
              {item.icon && <span className="ctx-icon">{item.icon}</span>}
              {item.label}
            </button>
          ),
        )}
      </div>
    </div>,
    document.body,
  );
}
