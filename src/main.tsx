import React from "react";
import ReactDOM from "react-dom/client";

const isBar = window.location.hash === "#bar";

async function boot() {
  if (import.meta.env.DEV && location.search.includes("mock")) {
    (await import("./mock")).installMocks();
  }
  const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
  if (isBar) {
    const { default: Bar } = await import("./bar/Bar");
    root.render(<Bar />);
  } else {
    await Promise.all([
      import("@fontsource/space-grotesk/400.css"),
      import("@fontsource/space-grotesk/500.css"),
      import("@fontsource/space-grotesk/600.css"),
      import("@fontsource/space-grotesk/700.css"),
      import("@fontsource/space-mono/400.css"),
    ]);
    await import("./theme/toolbay-theme.css");
    await import("./hub/hub.css");
    const { default: Hub } = await import("./hub/Hub");
    root.render(
      <React.StrictMode>
        <Hub />
      </React.StrictMode>,
    );
  }
}
boot();
