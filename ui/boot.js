// Runs in <head>, before the page paints: apply the saved look (light/dark, accent, skin) so the
// window never flashes the default theme. The manager passes it in window.__PENATES_APPEARANCE.
window.penatesApplyAppearance = function (a) {
  const root = document.documentElement;
  if (!a.theme || a.theme === "system") root.removeAttribute("data-theme"); else root.dataset.theme = a.theme;
  root.dataset.accent = a.accent || "violet";
  if (!a.skin || a.skin === "default") root.removeAttribute("data-skin"); else root.dataset.skin = a.skin;
};
if (window.__PENATES_APPEARANCE) window.penatesApplyAppearance(window.__PENATES_APPEARANCE);
