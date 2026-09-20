import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { logFrontend } from "./lib/api";
import "./index.css";

window.addEventListener("error", (event) => {
  void logFrontend("error", event.message, event.filename);
});

window.addEventListener("unhandledrejection", (event) => {
  void logFrontend("error", `unhandled rejection: ${String(event.reason)}`);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
