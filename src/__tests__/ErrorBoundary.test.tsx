import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ErrorBoundary } from "../components/ErrorBoundary";

function Bomb({ message }: { message: string }) {
  throw new Error(message);
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ErrorBoundary", () => {
  it("renders children when nothing throws", () => {
    render(
      <ErrorBoundary>
        <div>all good</div>
      </ErrorBoundary>,
    );
    expect(screen.getByText("all good")).toBeInTheDocument();
    expect(screen.queryByText(/Crashed/)).not.toBeInTheDocument();
  });

  it("catches a render error and shows the crash UI with the error message", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <ErrorBoundary name="ChatView">
        <Bomb message="boom" />
      </ErrorBoundary>,
    );
    expect(screen.getByText("ChatView Crashed")).toBeInTheDocument();
    expect(screen.getByText("boom")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /reload/i })).toBeInTheDocument();
    expect(spy).toHaveBeenCalled();
  });

  it('falls back to "View" as the default name', () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <ErrorBoundary>
        <Bomb message="kaboom" />
      </ErrorBoundary>,
    );
    expect(screen.getByText("View Crashed")).toBeInTheDocument();
  });

  it("shows a generic message when the error has no message text", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    function EmptyBomb(): never {
      throw new Error();
    }
    render(
      <ErrorBoundary>
        <EmptyBomb />
      </ErrorBoundary>,
    );
    // The fallback copy for empty messages
    expect(
      screen.getByText(/unexpected error occurred/i),
    ).toBeInTheDocument();
  });

  it("recovers when the error state is reset (Reload click)", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    let shouldThrow = true;
    function MaybeBomb({ children }: { children: React.ReactNode }) {
      if (shouldThrow) throw new Error("transient");
      return <>{children}</>;
    }
    render(
      <ErrorBoundary>
        <MaybeBomb>
          <div>recovered</div>
        </MaybeBomb>
      </ErrorBoundary>,
    );
    expect(screen.getByText(/Crashed/)).toBeInTheDocument();

    shouldThrow = false;
    const reloadSpy = vi.fn();
    Object.defineProperty(window, "location", {
      writable: true,
      value: { ...window.location, reload: reloadSpy },
    });
    fireEvent.click(screen.getByRole("button", { name: /reload/i }));
    expect(reloadSpy).toHaveBeenCalledTimes(1);
  });
});
