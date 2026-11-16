// The optional toolbar above a wrapped site (src-tauri/src/runner.rs puts it there). Its buttons ask the
// runner to act on the site; the runner keeps the address current through window.penatesAddress.
const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
const act = (action) => {
  // Reload and Home always load a page: show it at once, before the site's first byte arrives.
  if (action === "reload" || action === "home") window.penatesLoading(true);
  return invoke("toolbar_action", { action }).catch((e) => console.error(e));
};
for (const b of document.querySelectorAll("[data-action]")) {
  b.addEventListener("click", () => act(b.dataset.action));
}
// The app's keys work here too, as they do in the site (see navigation_script in runner.rs).
const mac = /Mac/.test(navigator.platform);
addEventListener("keydown", (e) => {
  if (e.repeat) return;
  const k = e.key.toLowerCase(), cmd = mac ? e.metaKey : e.ctrlKey;
  const action =
    (e.altKey && e.key === "ArrowLeft") || (mac && e.metaKey && e.key === "[") ? "back"
    : (e.altKey && e.key === "ArrowRight") || (mac && e.metaKey && e.key === "]") ? "forward"
    : e.key === "F5" || (cmd && !e.shiftKey && k === "r") ? "reload"
    : (e.altKey && e.key === "Home") || (mac && e.metaKey && e.shiftKey && k === "h") ? "home"
    : e.key === "F1" || (cmd && !e.altKey && e.key === "/") ? "keys"
    : null;
  if (!action) return;
  e.preventDefault();
  act(action);
});
// While a page loads, Reload becomes Stop and a line runs along the bottom.
const reload = document.getElementById("reload");
let giveUp;
window.penatesLoading = (loading) => {
  // Never stuck: without a "finished" signal (a cancelled load), it clears after 30 seconds.
  clearTimeout(giveUp);
  if (loading) giveUp = setTimeout(() => window.penatesLoading(false), 30000);
  document.body.classList.toggle("loading", loading);
  reload.dataset.action = loading ? "stop" : "reload";
  reload.title = loading ? "Stop loading" : "Reload (F5)";
  reload.setAttribute("aria-label", loading ? "Stop loading" : "Reload");
};
const address = document.getElementById("address");
address.addEventListener("focus", () => address.select());
window.penatesAddress = (url) => {
  address.value = url;
  address.title = url;
};
// The runner may send the first address before this script runs, so ask for it once.
invoke("toolbar_action", { action: "address" }).then((url) => url && window.penatesAddress(url), () => {});
// The toolbar's color: a site's own color or a custom one (any CSS color), or "" for Penates's colors. Text
// and icons turn light or dark by the color's brightness, so the toolbar stays readable on any color.
const rgbOf = (css) => {
  const c = document.createElement("canvas");
  c.width = c.height = 1;
  const x = c.getContext("2d");
  x.fillStyle = css;
  x.fillRect(0, 0, 1, 1);
  return x.getImageData(0, 0, 1, 1).data; // sRGB, whatever the format (hex, rgb(), oklch()...)
};
window.penatesColor = (css) => {
  const root = document.documentElement.style;
  const names = ["--tb-bg", "--tb-fg", "--tb-muted", "--tb-field", "--tb-line", "--tb-hover"];
  const [r, g, b, a] = css && CSS.supports("color", css) ? rgbOf(css) : [0, 0, 0, 0];
  if (!a) return names.forEach((n) => root.removeProperty(n)); // no usable color: Penates's own
  const lin = (v) => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  const L = 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
  const dark = 1.05 / (L + 0.05) > (L + 0.05) / 0.05; // white text contrasts more than dark text (WCAG)
  const ink = dark ? "255, 255, 255" : "0, 0, 0";
  root.setProperty("--tb-bg", `rgb(${r}, ${g}, ${b})`);
  root.setProperty("--tb-fg", dark ? "#ffffff" : "#17152a");
  root.setProperty("--tb-muted", `rgba(${ink}, 0.72)`);
  root.setProperty("--tb-field", `rgba(${ink}, ${dark ? 0.12 : 0.06})`);
  root.setProperty("--tb-line", `rgba(${ink}, ${dark ? 0.2 : 0.14})`);
  root.setProperty("--tb-hover", `rgba(${ink}, ${dark ? 0.16 : 0.08})`);
};
invoke("toolbar_action", { action: "color" }).then((c) => window.penatesColor(c), () => {});
