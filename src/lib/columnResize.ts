export function columnResize(node: HTMLElement): { destroy: () => void } {
  const header = node.querySelector<HTMLElement>(".vtable-header");
  if (!header) return { destroy: () => {} };
  const cells = Array.from(header.querySelectorAll<HTMLElement>(".vtable-header-cell"));
  if (cells.length < 3) return { destroy: () => {} };

  const MIN_WIDTH = 60;
  const handles: { el: HTMLElement; onDown: (e: PointerEvent) => void }[] = [];

  // Skip the checkbox column (0) and the last actions column (nothing to
  // resize against): cells 1..n-2 map to --col-1..--col-(n-2), which the pages
  // reference from their grid-template-columns: var(--cols) template.
  for (let i = 1; i < cells.length - 1; i++) {
    const handle = document.createElement("div");
    handle.className = "vtable-col-resize";
    const onDown = (e: PointerEvent) => {
      e.preventDefault();
      const startX = e.clientX;
      const startWidth = cells[i].getBoundingClientRect().width;
      node.classList.add("resizing");
      const onMove = (ev: PointerEvent) => {
        const width = Math.max(MIN_WIDTH, startWidth + (ev.clientX - startX));
        node.style.setProperty(`--col-${i}`, `${Math.round(width)}px`);
      };
      const onUp = () => {
        node.classList.remove("resizing");
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
      };
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
    };
    handle.addEventListener("pointerdown", onDown);
    cells[i].appendChild(handle);
    handles.push({ el: handle, onDown });
  }

  return {
    destroy() {
      for (const { el, onDown } of handles) {
        el.removeEventListener("pointerdown", onDown);
        el.remove();
      }
    },
  };
}
