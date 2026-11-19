import { loadCore, replay } from "./flight.js";
self.onmessage = async ({ data }) => {
  try {
    const w = await loadCore(data.url);
    const steps = await replay(w, data.file);
    postMessage({
      ok: true,
      message: `Pilot signature and all ${steps} input steps verified locally.`,
    });
  } catch (e) {
    postMessage({ ok: false, message: e.message });
  }
};
