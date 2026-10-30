import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, screen, fireEvent } from "@testing-library/svelte";
import InteractiveChoiceCard from "./InteractiveChoiceCard.svelte";
import type { ParsedChoicePrompt } from "../../lib/aiToolParser";

afterEach(cleanup);

describe("InteractiveChoiceCard.svelte", () => {
  it("renders single-choice options and fires onSelect on option click", async () => {
    const onSelect = vi.fn();
    const choice: ParsedChoicePrompt = {
      question: "Which container would you like to inspect?",
      multi: false,
      options: [
        { label: "nginx-proxy", description: "Running on port 80", value: "nginx-proxy" },
        { label: "postgres-db", description: "Database storage", value: "postgres-db" },
      ],
    };

    render(InteractiveChoiceCard, { choice, onSelect });

    expect(screen.getByText("Which container would you like to inspect?")).toBeInTheDocument();
    expect(screen.getByText("nginx-proxy")).toBeInTheDocument();
    expect(screen.getByText("Running on port 80")).toBeInTheDocument();
    expect(screen.getByText("postgres-db")).toBeInTheDocument();

    const nginxRow = screen.getByText("nginx-proxy").closest(".choice-option-row")!;
    await fireEvent.click(nginxRow);

    expect(onSelect).toHaveBeenCalledWith("nginx-proxy");
    expect(screen.getByText(/Selected: nginx-proxy/)).toBeInTheDocument();
  });

  it("handles insert button click on single-choice option without triggering onSelect", async () => {
    const onSelect = vi.fn();
    const onInsert = vi.fn();
    const choice: ParsedChoicePrompt = {
      question: "Select profile to restart",
      multi: false,
      options: [
        { label: "default", value: "colima restart default" },
        { label: "k8s", value: "colima restart k8s" },
      ],
    };

    render(InteractiveChoiceCard, { choice, onSelect, onInsert });

    const insertBtns = screen.getAllByRole("button", { name: /insert into prompt composer/i });
    expect(insertBtns.length).toBe(2);

    await fireEvent.click(insertBtns[0]);

    expect(onInsert).toHaveBeenCalledWith("colima restart default");
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("supports multi-choice toggle and submission with count", async () => {
    const onSelect = vi.fn();
    const onInsert = vi.fn();
    const choice: ParsedChoicePrompt = {
      question: "Select volumes to prune",
      multi: true,
      options: [
        { label: "vol-data", value: "vol-data" },
        { label: "vol-cache", value: "vol-cache" },
        { label: "vol-logs", value: "vol-logs" },
      ],
    };

    render(InteractiveChoiceCard, { choice, onSelect, onInsert });

    expect(screen.getByText("Multi-select")).toBeInTheDocument();

    const submitBtn = screen.getByRole("button", { name: /submit/i });
    expect(submitBtn).toBeDisabled();

    const opt1 = screen.getByText("vol-data").closest(".choice-option-row")!;
    const opt3 = screen.getByText("vol-logs").closest(".choice-option-row")!;

    await fireEvent.click(opt1);
    await fireEvent.click(opt3);

    expect(screen.getByRole("button", { name: /submit \(2\)/i })).not.toBeDisabled();

    // Test insert multi
    const insertMultiBtn = screen.getByRole("button", { name: /^insert$/i });
    await fireEvent.click(insertMultiBtn);
    expect(onInsert).toHaveBeenCalledWith("vol-data, vol-logs");

    // Test submit multi
    await fireEvent.click(screen.getByRole("button", { name: /submit \(2\)/i }));
    expect(onSelect).toHaveBeenCalledWith("vol-data, vol-logs");
  });

  it("locks options and prevents selection when isLatest is false", async () => {
    const onSelect = vi.fn();
    const choice: ParsedChoicePrompt = {
      question: "Archived question",
      multi: false,
      options: [{ label: "Option A", value: "A" }],
    };

    render(InteractiveChoiceCard, { choice, isLatest: false, onSelect });

    expect(screen.getByText("Previous turn")).toBeInTheDocument();

    const row = screen.getByText("Option A").closest(".choice-option-row")!;
    await fireEvent.click(row);

    expect(onSelect).not.toHaveBeenCalled();
  });

  it("allows custom text entry and submission", async () => {
    const onSelect = vi.fn();
    const choice: ParsedChoicePrompt = {
      question: "Select CPU cores",
      multi: false,
      options: [{ label: "2 cores", value: "2" }, { label: "4 cores", value: "4" }],
    };

    render(InteractiveChoiceCard, { choice, onSelect });

    const otherBtn = screen.getByRole("button", { name: /type other response…/i });
    await fireEvent.click(otherBtn);

    const input = screen.getByPlaceholderText(/type custom answer/i);
    await fireEvent.input(input, { target: { value: "8 cores" } });

    const sendBtn = screen.getByRole("button", { name: /send/i });
    await fireEvent.click(sendBtn);

    expect(onSelect).toHaveBeenCalledWith("8 cores");
  });

  it("navigates options with arrow keys and selects with Enter", async () => {
    const onSelect = vi.fn();
    const choice: ParsedChoicePrompt = {
      question: "Pick VM size",
      multi: false,
      options: [
        { label: "Small", value: "small" },
        { label: "Medium", value: "medium" },
        { label: "Large", value: "large" },
      ],
    };

    render(InteractiveChoiceCard, { choice, onSelect });

    const optionsContainer = document.querySelector(".choice-options")!;

    // Navigate down to Medium
    await fireEvent.keyDown(optionsContainer, { key: "ArrowDown" });
    // Navigate down to Large
    await fireEvent.keyDown(optionsContainer, { key: "ArrowDown" });
    // Select Large with Enter
    await fireEvent.keyDown(optionsContainer, { key: "Enter" });

    expect(onSelect).toHaveBeenCalledWith("large");
  });
});
