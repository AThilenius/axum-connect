import { createRoot } from "react-dom/client";
import { App } from "~/app";

async function main() {
  const root = createRoot(document.getElementById("root")!);
  root.render(<App />);
}

main();
