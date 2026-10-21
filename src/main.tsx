import React from "react";
import ReactDOM from "react-dom/client";

const isBar = window.location.hash === "#bar";

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
    root.render(<Bar />);
  } else {
    await Promise.all([import("@fontsource-variable/figtree"), import("@fontsource/eb-garamond/400.css")]);
    await import("./theme/flow-theme.css");
    await import("./hub/base.css");
    await import("./hub/hub.css");
    await import("./hub/pages.css");
    const { default: Hub } = await import("./hub/Hub");
    root.render(
      <React.StrictMode>
        <Hub />
      </React.StrictMode>,
    );
    // Show the window only after the first paint, so it never flashes white.
    requestAnimationFrame(() => requestAnimationFrame(() => import("./api").then(({ api }) => api.uiReady().catch(() => {}))));
  }
}
boot();
