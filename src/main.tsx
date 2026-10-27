import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { native } from "./lib/bridge";
import "./styles/tokens.css";
import "./styles/app.css";
document.body.classList.add(native ? "desktop-app" : "browser-preview");
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
