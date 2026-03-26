import { useState, useEffect, useCallback, useRef } from "react";
import { CheckIcon } from "./Icons";

interface TourStep {
  targetSelector: string;
  title: string;
  body: string;
  position?: "right" | "bottom" | "left";
}

interface GettingStartedTourProps {
  onComplete: () => void;
  onNavigate?: (page: string) => void;
}

const TOUR_STEPS: TourStep[] = [
  {
    targetSelector: '[data-tour-id="sidebar-nav"]',
    title: "Sidebar Navigation",
    body: "Navigate between features using the sidebar. It's organized into sections: Overview, Docker, Infrastructure, and Tools.",
    position: "right",
  },
  {
    targetSelector: '[data-tour-id="nav-dashboard"]',
    title: "Dashboard",
    body: "Monitor your VMs and Docker resources at a glance. See real-time stats for instances, containers, images, and more.",
    position: "right",
  },
  {
    targetSelector: '[data-tour-id="nav-instances"]',
    title: "Instance Management",
    body: "Create and manage Colima VM instances. Start, stop, restart, or delete instances with configurable resources.",
    position: "right",
  },
  {
    targetSelector: '[data-tour-id="nav-containers"]',
    title: "Docker Containers",
    body: "View and manage Docker containers. Run new containers, inspect logs, execute commands, and monitor resource usage.",
    position: "right",
  },
  {
    targetSelector: '[data-tour-id="nav-terminal"]',
    title: "Terminal Access",
    body: "SSH into your Colima instances directly from the browser. Open multiple terminal sessions in tabs.",
    position: "right",
  },
  {
    targetSelector: '[data-tour-id="nav-dockerfile"]',
    title: "Dockerfile Generator",
    body: "Generate Dockerfiles with pre-built templates and AI assistance. Supports Node.js, Python, Go, Rust, and more.",
    position: "right",
  },
  {
    targetSelector: '[data-tour-id="nav-settings"]',
    title: "Settings & System",
    body: "Check system dependency status, view Docker disk usage, and manage application preferences.",
    position: "right",
  },
];

export default function GettingStartedTour({ onComplete }: GettingStartedTourProps) {
  const [currentStep, setCurrentStep] = useState(0);
  const [targetRect, setTargetRect] = useState<DOMRect | null>(null);
  const [tooltipPos, setTooltipPos] = useState({ left: 0, top: 0 });
  const rafRef = useRef<number>(0);

  const updatePosition = useCallback(() => {
    const step = TOUR_STEPS[currentStep];
    const el = document.querySelector(step.targetSelector);
    if (!el) return;

    const rect = el.getBoundingClientRect();
    setTargetRect(rect);

    // Position tooltip to the right of the target
    const tooltipWidth = 320;
    const tooltipHeight = 200;
    const gap = 16;
    let left = rect.right + gap;
    let top = rect.top;

    // If no room on right, position below
    if (left + tooltipWidth > window.innerWidth) {
      left = rect.left;
      top = rect.bottom + gap;
    }

    // Ensure tooltip stays in viewport
    if (top + tooltipHeight > window.innerHeight) {
      top = window.innerHeight - tooltipHeight - 20;
    }
    if (left < 10) left = 10;
    if (top < 10) top = 10;

    setTooltipPos({ left, top });
  }, [currentStep]);

  useEffect(() => {
    updatePosition();
    const handleResize = () => updatePosition();
    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("resize", handleResize);
      cancelAnimationFrame(rafRef.current);
    };
  }, [updatePosition]);

  const handleNext = () => {
    if (currentStep < TOUR_STEPS.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      onComplete();
    }
  };

  const handlePrev = () => {
    if (currentStep > 0) {
      setCurrentStep(currentStep - 1);
    }
  };

  const step = TOUR_STEPS[currentStep];
  const padding = 6;

  // Build clip-path polygon to create a spotlight hole
  const clipPath = targetRect
    ? `polygon(
        0% 0%, 0% 100%, 100% 100%, 100% 0%, 0% 0%,
        ${targetRect.left - padding}px ${targetRect.top - padding}px,
        ${targetRect.right + padding}px ${targetRect.top - padding}px,
        ${targetRect.right + padding}px ${targetRect.bottom + padding}px,
        ${targetRect.left - padding}px ${targetRect.bottom + padding}px,
        ${targetRect.left - padding}px ${targetRect.top - padding}px
      )`
    : "none";

  return (
    <div className="tour-overlay">
      {/* Dark backdrop with spotlight hole */}
      <div
        className="tour-backdrop"
        style={{ clipPath }}
        onClick={handleNext}
      />

      {/* Highlight ring around target */}
      {targetRect && (
        <div
          className="tour-highlight"
          style={{
            left: targetRect.left - padding,
            top: targetRect.top - padding,
            width: targetRect.width + padding * 2,
            height: targetRect.height + padding * 2,
          }}
        />
      )}

      {/* Tooltip */}
      <div
        className="tour-tooltip"
        style={{ left: tooltipPos.left, top: tooltipPos.top }}
      >
        <div className="tour-tooltip-title">
          {step.title}
        </div>
        <div className="tour-tooltip-body">
          {step.body}
        </div>
        <div className="tour-tooltip-footer">
          {/* Step dots */}
          <div className="tour-dots">
            {TOUR_STEPS.map((_, i) => (
              <div
                key={i}
                className={`tour-dot ${i === currentStep ? "active" : i < currentStep ? "done" : ""}`}
              />
            ))}
          </div>

          {/* Actions */}
          <div className="tour-tooltip-actions">
            <button
              className="btn btn-ghost"
              style={{ fontSize: "var(--text-xs)", padding: "4px 10px" }}
              onClick={onComplete}
            >
              Skip
            </button>
            {currentStep > 0 && (
              <button
                className="btn btn-ghost"
                style={{ fontSize: "var(--text-xs)", padding: "4px 10px" }}
                onClick={handlePrev}
              >
                ← Back
              </button>
            )}
            <button
              className="btn btn-primary"
              style={{ fontSize: "var(--text-xs)", padding: "4px 12px" }}
              onClick={handleNext}
            >
              {currentStep < TOUR_STEPS.length - 1 ? "Next →" : <><CheckIcon size={12} style={{ display: "inline", verticalAlign: "middle" }} /> Finish</>}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
