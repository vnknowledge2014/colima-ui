import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { confirm, confirmState } from "./confirm.svelte";
import ConfirmDialog from "../components/ConfirmDialog.svelte";

describe("confirm store & ConfirmDialog accessibility", () => {
  beforeEach(() => {
    confirmState.open = false;
    confirmState.resolve = null;
    confirmState.options = { message: "" };
  });

  afterEach(() => {
    cleanup();
  });

  it("resolves a previous pending confirm promise with false when superseded", async () => {
    const first = confirm({ title: "First", message: "Delete A?" });
    const second = confirm({ title: "Second", message: "Delete B?" });

    await expect(first).resolves.toBe(false);
    expect(confirmState.open).toBe(true);
    expect(confirmState.options.title).toBe("Second");

    confirmState.resolve?.(true);
    confirmState.open = false;
    confirmState.resolve = null;
    await expect(second).resolves.toBe(true);
  });

  it("renders role='dialog' with aria-modal='true' and dismisses on Escape", async () => {
    render(ConfirmDialog);
    const pending = confirm({ title: "Confirm Prune", message: "Prune unused images?" });

    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toHaveAttribute("aria-labelledby", "confirm-dialog-title");

    await fireEvent.keyDown(window, { key: "Escape" });

    await expect(pending).resolves.toBe(false);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
