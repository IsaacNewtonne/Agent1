import React from "react";
import { createRoot } from "react-dom/client";
import CollabWorkspace from "./components/CollabWorkspace";
import "./styles.css";
import "./collab.css";

createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <CollabWorkspace />
  </React.StrictMode>,
);
