import "@fontsource/figtree/400.css";
import "@fontsource/figtree/500.css";
import "@fontsource/figtree/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/barlow-condensed/800.css";
import { jobEvents } from "./app/job";
import { isActive, serverLimits, state } from "./app/state";
import { backend } from "./backend";
import { setupFooter } from "./ui/footer";
import { setupWindowChrome } from "./ui/header";
import { setupProject } from "./ui/project";
import { render, schedule } from "./ui/render";
import { setupResourceEditors } from "./ui/resources";
import { setupSettings } from "./ui/settings";
import { setupSolver } from "./ui/solver";
import { advanceSteps, REVEAL_MS } from "./ui/steps";

async function init() {
  setupWindowChrome();
  setupProject();
  setupResourceEditors();
  setupSolver();
  setupFooter();
  setupSettings();

  await backend.listen(jobEvents);
  state.settings = await backend.loadSettings();
  state.showAllLogs = state.settings.results.showAllLogs;
  const limits = serverLimits();
  state.resources = state.settings.resources ?? (limits ? { cpu: limits.cpu, mem: limits.mem, gpu: 0 } : { ...state.resources });
  render();

  window.setInterval(() => {
    if (isActive()) render();
  }, 1000);
  window.setInterval(advanceSteps, REVEAL_MS);
  window.addEventListener("resize", schedule);
}

void init();
