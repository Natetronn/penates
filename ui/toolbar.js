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
