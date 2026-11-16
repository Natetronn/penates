// The optional toolbar above a wrapped site (src-tauri/src/runner.rs puts it there). Its buttons ask the
// runner to act on the site; the runner keeps the address current through window.penatesAddress.
const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
for (const b of document.querySelectorAll("[data-action]")) {
  b.addEventListener("click", () => invoke("toolbar_action", { action: b.dataset.action }).catch((e) => console.error(e)));
}
const address = document.getElementById("address");
address.addEventListener("focus", () => address.select());
window.penatesAddress = (url) => {
  address.value = url;
  address.title = url;
};
