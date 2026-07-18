import React from "react";
import ReactDOM from "react-dom/client";
// Self-hosted fonts — bundled locally, no network requests at runtime.
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
