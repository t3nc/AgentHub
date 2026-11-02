import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
// 第三方样式先引入，让 index.css 里的工作台令牌能覆盖它
import "@xyflow/react/dist/style.css";
import "@xterm/xterm/css/xterm.css";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
