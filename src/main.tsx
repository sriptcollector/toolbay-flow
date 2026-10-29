import React from "react";
import ReactDOM from "react-dom/client";

const isBar = window.location.hash === "#bar";

/**
 * Catches a crash while drawing a screen, so the window shows what happened and a
 * Reload button instead of going blank. The bar has no room for that: it reloads itself.
 */
class ErrorBoundary extends React.Component<{ children: React.ReactNode; autoReload?: boolean }, { error: Error | null }> {
  state: { error: Error | null } = { error: null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error) {
    console.error(error);
    if (this.props.autoReload) setTimeout(() => location.reload(), 2000);
  }
  render() {
    if (!this.state.error) return this.props.children;
    if (this.props.autoReload) return null;
    return (
      <div className="empty" style={{ padding: 48 }}>
        <div className="empty-title">Something went wrong on this screen</div>
        <div className="empty-body">
          <p>Your dictations, notes and settings are safe. Reload to carry on.</p>
          <p className="muted small">{this.state.error.message}</p>
          <button className="btn primary" onClick={() => location.reload()}>
            Reload
          </button>
        </div>
      </div>
    );
  }
}

async function boot() {
  if (import.meta.env.DEV && location.search.includes("mock")) {
    (await import("./mock")).installMocks();
  }
  if (!import.meta.env.DEV || !location.search.includes("mock")) {
    (await import("./ownKeys")).forwardOwnKeys();
  }
  const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
  if (isBar) {
    await import("@fontsource-variable/figtree");
    const { default: Bar } = await import("./bar/Bar");
    root.render(
      <ErrorBoundary autoReload>
        <Bar />
      </ErrorBoundary>,
    );
  } else {
    await Promise.all([import("@fontsource-variable/figtree"), import("@fontsource/eb-garamond/400.css")]);
    await import("./theme/flow-theme.css");
    await import("./hub/base.css");
    await import("./hub/hub.css");
    await import("./hub/pages.css");
    const { default: Hub } = await import("./hub/Hub");
    const [{ api, on }, { flushAll }] = await Promise.all([import("./api"), import("./hub/ui")]);
    // Quitting: save edits still waiting on a timer, then let Flow exit.
    on("flush-before-quit", () => {
      flushAll().finally(() => api.quitFlushed().catch(() => {}));
    });
    root.render(
      <React.StrictMode>
        <ErrorBoundary>
          <Hub />
        </ErrorBoundary>
      </React.StrictMode>,
    );
    // Show the window only after the first paint, so it never flashes white.
    requestAnimationFrame(() => requestAnimationFrame(() => import("./api").then(({ api }) => api.uiReady().catch(() => {}))));
  }
}
boot();
