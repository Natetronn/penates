// Penates manager front end. Plain JS, no build step; talks to the Rust side through Tauri IPC
// (src-tauri/src/commands.rs). Everything shown comes from textContent or properties, never HTML.

const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
const $ = (id) => document.getElementById(id);

function el(tag, props = {}, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "class") e.className = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else if (k.startsWith("aria-") || k.startsWith("data-") || k === "role") e.setAttribute(k, v);
    else if (v !== false && v != null) e[k] = v;
  }
  e.append(...kids.flat(Infinity).filter((x) => x != null && x !== false));
  return e;
}
// replaceChildren that accepts nested arrays (lists of rows)
const put = (parent, ...kids) => parent.replaceChildren(...kids.flat(Infinity).filter((x) => x != null && x !== false));
const plural = (n, one, many = one + "s") => `${n} ${n === 1 ? one : many}`;
const fmtBytes = (b) => (b < 1024 ? b + " B" : b < 1048576 ? Math.round(b / 1024) + " KB" : (b / 1048576).toFixed(1) + " MB");
const link = (text, onclick) => el("a", { href: "#", textContent: text, onclick: (e) => { e.preventDefault(); onclick(); } });

// ---- messages: a toast that stays visible however far the page is scrolled ----
let toastTimer;
function say(text, kind = "ok") {
  const t = $("toast");
  clearTimeout(toastTimer);
  if (!text) { t.hidden = true; return; }
  t.setAttribute("aria-live", kind === "err" ? "assertive" : "polite");
  t.setAttribute("role", kind === "err" ? "alert" : "status");
  t.className = "toast " + kind;
  $("toast-text").textContent = text;
  t.hidden = false;
  if (kind === "ok") toastTimer = setTimeout(() => say(""), 6000); // errors stay until dismissed
}
const fail = (e) => say(String(e), "err");
/** Say `text`, adding any warnings the Rust side returned (e.g. "launcher not installed"). */
function sayWith(text, warnings) {
  const w = (Array.isArray(warnings) ? warnings : [warnings]).filter(Boolean);
  w.length ? say(text + " Note: " + w.join("; "), "err") : say(text);
}
$("toast-close").onclick = () => say("");

/** Disable `btn` (optionally relabel it) while `work` runs; report errors. */
async function busy(btn, label, work) {
  const old = btn.textContent;
  btn.disabled = true; if (label) btn.textContent = label;
  try { return await work(); } catch (e) { fail(e); } finally { btn.disabled = false; btn.textContent = old; }
}

// ---- sections ----
const TABS = ["apps", "catalog", "create", "updates", "settings"];
let tab = "apps", editing = null, importing = null, submitting = null;
function show(t) {
  tab = t;
  if (t !== "apps") importing = submitting = null; // an unfinished edit is kept; it is still there on Apps
  document.querySelectorAll("nav button").forEach((b) => (b.dataset.tab === t ? b.setAttribute("aria-current", "page") : b.removeAttribute("aria-current")));
  for (const n of TABS) $("tab-" + n).hidden = n !== t;
  document.querySelector("main").dataset.tab = t; // the Catalog grid gets a wider page than the forms
  $("banner").hidden = t === "updates"; // the Updates tab shows the same notice inline
  render();
}
document.querySelectorAll("nav button").forEach((b) => (b.onclick = () => show(b.dataset.tab)));

function iconEl(app) {
  const holder = el("div", { class: "ph" });
  if (app.icon) {
    invoke("icon_data", { path: app.icon }).then((d) => holder.replaceWith(el("img", { src: d, alt: "" }))).catch(() => {});
  }
  return holder;
}

// ---- the ⋮ menu ----
function closeMenus(focusButton) {
  document.querySelectorAll(".menu").forEach((m) => m.remove());
  document.querySelectorAll("[aria-expanded=true]").forEach((b) => b.setAttribute("aria-expanded", "false"));
  if (focusButton) focusButton.focus();
}
document.addEventListener("click", (e) => { if (!e.target.closest(".actions")) closeMenus(); });
document.addEventListener("keydown", (e) => {
  const open = document.querySelector(".menu");
  if (!open) return;
  if (e.key === "Escape") { e.preventDefault(); closeMenus(open.parentElement.querySelector("[aria-expanded]")); }
  else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    const items = [...open.querySelectorAll("button:not(:disabled)")];
    const i = items.indexOf(document.activeElement);
    items[(i + (e.key === "ArrowDown" ? 1 : items.length - 1)) % items.length]?.focus();
    e.preventDefault();
  }
});
/** A destructive menu item that needs a second click. */
function confirmItem(label, again, fn) {
  const b = el("button", { class: "danger", textContent: label, onclick: async (ev) => {
    ev.stopPropagation();
    if (!b.dataset.sure) { b.dataset.sure = "1"; b.textContent = again; return; }
    closeMenus();
    try { await fn(); } catch (e) { fail(e); }
  } });
  return b;
}

// ---- update all your apps (the Updates tab) ----
async function updateAllApps() {
  const r = await invoke("update_all");
  const changed = r.updated.filter((o) => o.preset || o.icon_refreshed).length;
  const warnings = r.updated.map((o) => o.warning && `${o.app.name}: ${o.warning}`).concat(r.failed.map((f) => `${f.id}: ${f.error}`));
  sayWith(`Updated ${plural(r.updated.length, "app")} (${changed} changed).`, warnings);
  await checkApps();
  return r;
}

// ---- apps list ----
async function renderApps() {
  const root = $("tab-apps");
  if (editing) return renderEdit(root);
  if (importing) return renderImport(root);
  if (submitting) return renderSubmit(root);
  const { apps, broken } = await invoke("list_apps");
  const pending = pendingUpdates();
  put(root,
    el("div", { class: "toolbar" },
      el("span", { class: "hint" }, plural(apps.length, "app") + " installed"),
      el("button", { textContent: "Import…", onclick: () => { importing = { stage: "pick" }; renderApps(); } }),
      el("button", { textContent: "Export all", title: "Save every app to a .penates file in your export folder (Settings)", onclick: async () => {
        try { say("Saved to " + (await invoke("export_apps", { ids: [], withSettings: false }))); } catch (e) { fail(e); } } })),
    apps.length ? apps.map((a) => appRow(a, pending)) : el("p", { class: "hint" }, "No apps yet. Use the Create tab to add one."),
    broken.map(brokenRow));
}

function brokenRow(b) {
  return el("div", { class: "app broken" }, el("div", { class: "ph" }),
    el("div", { class: "meta" }, el("b", {}, b.id + " (can't be read)"), el("span", { class: "wrap" }, b.error)),
    el("button", { class: "danger", textContent: "Remove", onclick: async () => {
      try { await invoke("remove_app", { id: b.id, purge: false }); say(b.id + " removed."); renderApps(); } catch (e) { fail(e); } } }));
}

/** Update one app (newer preset, fresh site icon) and say what changed. */
async function updateApp(a) {
  const o = await invoke("update_app", { id: a.id });
  sayWith(`${a.name}: icon ${o.icon_refreshed ? "refreshed" : "unchanged"}` + (o.preset ? `, preset ${o.preset} applied` : "") + ".", o.warning);
  checkApps(); renderApps();
}
/** Apps with a newer catalog preset waiting, from the last update check. */
const pendingUpdates = () => new Set((checks.apps.report?.apps || []).map((u) => u.id));

function appMenu(a, actions, menuBtn) {
  const item = (label, fn) => el("button", { textContent: label, onclick: async () => { closeMenus(); try { await fn(); } catch (e) { fail(e); } } });
  const cache = item("Clear cache", async () => { const n = await invoke("clear_cache", { id: a.id }); say(`Cleared ${fmtBytes(n)} of cache for ${a.name}. You stay signed in.`); });
  invoke("cache_info", { id: a.id }).then((i) => { cache.textContent = `Clear cache (${fmtBytes(i.bytes)})`; cache.disabled = i.bytes === 0; }).catch(() => {});
  const menu = el("div", { class: "menu", role: "menu", "aria-label": "More actions for " + a.name },
    item("Update (preset and icon)", () => updateApp(a)),
    item("Share with the app catalog…", () => { submitting = { app: a }; renderApps(); }),
    item("Export app…", async () => say("Saved to " + (await invoke("export_apps", { ids: [a.id], withSettings: false })))),
    cache,
    confirmItem("Clear cookies & data…", "Click again: sign out & clear", async () => { await invoke("clear_data", { id: a.id }); say("Cleared cookies and data for " + a.name + "."); }),
    confirmItem("Remove app…", "Click again: remove (keeps data)", async () => { await invoke("remove_app", { id: a.id, purge: false }); say(a.name + " removed (data kept)."); renderApps(); }),
    confirmItem("Remove app and data…", "Click again: remove & sign out", async () => { await invoke("remove_app", { id: a.id, purge: true }); say(a.name + " and its data removed."); renderApps(); }));
  menu.querySelectorAll("button").forEach((b) => b.setAttribute("role", "menuitem"));
  actions.append(menu);
  menuBtn.setAttribute("aria-expanded", "true");
  menu.querySelector("button").focus();
}

function appRow(a, pending) {
  const open = () => invoke("run_app", { id: a.id }).catch(fail);
  const menuBtn = el("button", { textContent: "⋮", title: "More", "aria-label": "More actions for " + a.name, "aria-haspopup": "menu", "aria-expanded": "false" });
  const hasUpdate = pending.has(a.id);
  const actions = el("div", { class: "actions" },
    // The same three controls on every row, so Open and Edit line up; Update is in the ⋮ menu.
    el("button", { class: "primary", textContent: "Open", onclick: open }),
    el("button", { textContent: "Edit", onclick: () => { editing = structuredClone(a); renderApps(); } }),
    menuBtn);
  menuBtn.onclick = (ev) => {
    ev.stopPropagation();
    const wasOpen = actions.querySelector(".menu"); closeMenus();
    if (!wasOpen) appMenu(a, actions, menuBtn);
  };
  // Double-clicking the icon or name opens the app, like a launcher (the buttons keep their own clicks).
  const row = el("div", { class: "app opens", title: "Double-click to open " + a.name,
      ondblclick: (e) => { if (!e.target.closest(".actions")) open(); } },
    iconEl(a),
    el("div", { class: "meta" }, el("b", {}, a.name), el("span", { class: "url" }, a.url)),
    hasUpdate ? el("button", { type: "button", class: "chip new", title: "A newer preset is waiting in the app catalog. Show Updates",
        onclick: () => show("updates") }, "update available")
      : a.catalog_version ? el("span", { class: "chip", title: "Catalog preset version" }, "preset " + a.catalog_version) : null,
    actions);
  return row;
}

// ---- submit an app to the catalog ----
function renderSubmit(root) {
  const a = submitting.app;
  const notes = el("input", { type: "text", id: "sub-notes", placeholder: "e.g. Keep Google sign-in inside the window" });
  const preview = el("pre", { class: "preview", "aria-live": "polite" }, "Say what the preset fixes to see the entry.");
  const title = el("h2", {}, `Share ${a.name} with the App Catalog`);
  const how = el("span", {}, "GitHub opens in your browser with everything filled in, and you send it from there.");
  const go = el("button", { class: "primary", textContent: "Open GitHub", disabled: true });
  let current = null, timer;
  // Whether this adds a new site or suggests a change is decided by Penates (matching the catalog), so the
  // wording follows its answer.
  const describe = (kind) => {
    title.textContent = kind === "change" ? `Suggest a change to the ${a.name} preset` : `Submit ${a.name} to the app catalog`;
    how.textContent = kind === "change"
      ? `${hostOf(a.url)} is already in the catalog. GitHub opens in your browser with an issue suggesting your app's settings as the next version of its preset.`
      : `Everyone who adds ${hostOf(a.url)} would get these fixes. GitHub opens in your browser with the entry filled in; propose it there as a pull request.`;
    go.textContent = kind === "change" ? "Open GitHub to suggest it" : "Open GitHub to propose it";
  };
  const refresh = () => {
    clearTimeout(timer); go.disabled = true; current = null;
    if (!notes.value.trim()) { preview.textContent = "Say what the preset fixes to see the entry."; return; }
    timer = setTimeout(async () => {
      try { current = await invoke("catalog_submission", { id: a.id, notes: notes.value }); describe(current.kind); preview.textContent = current.json; go.disabled = false; }
      catch (e) { preview.textContent = String(e); }
    }, 250);
  };
  notes.addEventListener("input", refresh);
  go.onclick = async () => {
    if (!current) return;
    try {
      await invoke("open_url", { url: current.url });
      say(current.kind === "change" ? "Opened GitHub with your suggestion filled in." : "Opened GitHub with the entry filled in: choose \"Propose changes\" there.");
      submitting = null; renderApps();
    } catch (e) { fail(e); }
  };
  put(root, title,
    el("p", { class: "hint" }, how, " Penates sends nothing itself, and you need a GitHub account."),
    el("label", { class: "f" }, "What does it fix? (one line)", notes),
    el("h3", {}, "What Will Be Shared"),
    el("p", { class: "hint" }, "Check the extra sites before sharing: they can reveal addresses you use, such as a company sign-in page. Custom JavaScript is never included."),
    preview,
    el("div", { class: "formbar" }, go, el("button", { textContent: "Cancel", onclick: () => { submitting = null; renderApps(); } })));
  notes.focus();
}

// ---- import ----
function importFlags(s) {
  return [
    s.exists ? el("span", { class: "chip" }, "already exists") : null,
    s.custom_js ? el("span", { class: "chip warn" }, "custom JavaScript") : null,
    s.custom_css ? el("span", { class: "chip" }, "custom CSS") : null,
    s.user_agent ? el("span", { class: "chip" }, "custom user agent") : null,
    s.all_links_in_app ? el("span", { class: "chip warn" }, "keeps all links in the app") : null,
    s.camera_microphone || s.devtools ? el("span", { class: "chip", title: "Switched off on import; turn it back on under Edit if you want it" }, "camera/mic or devtools: switched off") : null,
  ];
}

function renderImportPick(root, cancel) {
  const url = el("input", { type: "text", id: "imp-url", class: "grow", placeholder: "https://example.com/github.penates" });
  const load = async (source, isUrl) => {
    try { importing = { stage: "review", data: await invoke("inspect_import", { source, isUrl }) }; renderApps(); } catch (e) { fail(e); }
  };
  const choose = el("button", { type: "button", id: "imp-file", textContent: "Choose file…", onclick: (ev) => busy(ev.target, "Choosing…", async () => {
    const f = await invoke("pick_file", { kind: "penates" });
    if (f) await load(new TextDecoder().decode(new Uint8Array(f.bytes)), false);
  }) });
  put(root, el("h2", {}, "Import Apps"),
    el("fieldset", {}, el("legend", {}, "From a File"),
      el("p", { class: "hint" }, "A .penates file exported from Penates. You review it before anything is added."),
      el("div", { class: "row" }, choose)),
    el("fieldset", {}, el("legend", {}, "From a Link"),
      el("label", { class: "f", htmlFor: "imp-url" }, "Address of a .penates file"),
      el("div", { class: "row" }, url, el("button", { textContent: "Load", onclick: () => load(url.value, true) }))),
    el("div", { class: "row" }, el("button", { textContent: "Cancel", onclick: cancel })));
}

function importResult(r) {
  const bits = [`Imported ${r.imported.length}`];
  if (r.skipped.length) bits.push(`skipped ${r.skipped.length}`);
  if (r.renamed.length) bits.push(`renamed ${r.renamed.length}`);
  if (r.js_removed.length) bits.push(`removed custom JavaScript from ${r.js_removed.length}`);
  if (r.permissions_reset.length) bits.push(`switched off camera/mic and devtools for ${r.permissions_reset.length}`);
  if (r.settings_applied) bits.push("applied settings");
  sayWith(bits.join(", ") + ".", r.errors);
}

function renderImport(root) {
  const cancel = () => { importing = null; renderApps(); };
  if (importing.stage === "pick") return renderImportPick(root, cancel);
  const d = importing.data;
  const anyJs = d.summary.some((s) => s.custom_js), anyExists = d.summary.some((s) => s.exists);
  const conflict = el("select", { id: "imp-conflict" }, [["rename", "Keep both (rename the new one)"], ["skip", "Skip apps that already exist"], ["overwrite", "Overwrite apps that already exist"]].map(([v, t]) => el("option", { value: v, textContent: t })));
  const allowJs = el("input", { type: "checkbox", id: "imp-js" });
  const withSettings = el("input", { type: "checkbox", id: "imp-settings" });
  const rows = d.summary.map((s) => el("div", { class: "app" },
    el("div", { class: "meta" }, el("b", {}, s.name), el("span", { class: "url" }, s.url),
      !s.all_links_in_app && s.extra_hosts.length ? el("span", { class: "wrap" }, "Also keeps these sites in the app: " + s.extra_hosts.join(", ")) : null),
    importFlags(s)));
  put(root, el("h2", {}, `Import ${plural(d.summary.length, "App")}`), rows,
    anyJs ? el("div", { class: "banner" }, el("span", {}, "Some of these apps include custom JavaScript. It runs inside the site and could read what you type. It is removed unless you allow it below. Only allow it if you trust where this file came from.")) : null,
    el("fieldset", {}, el("legend", {}, "Options"),
      anyExists ? el("label", { class: "f" }, "If an app already exists", conflict) : null,
      anyJs ? el("label", { class: "chk" }, allowJs, el("span", {}, "Keep custom JavaScript", el("small", {}, "Leave off unless you trust the source"))) : null,
      d.has_settings ? el("label", { class: "chk" }, withSettings, el("span", {}, "Also apply the settings in this file", el("small", {}, "Look, accent and the app catalogs in use. Other catalogs decide which presets your apps get."))) : null,
      el("p", { class: "hint" }, "Files contain configs and icons only, never logins or cookies. Camera, microphone and developer tools are always switched off on import.")),
    el("div", { class: "formbar" },
      el("button", { class: "primary", textContent: "Import", onclick: (ev) => busy(ev.target, "Importing…", async () => {
        importResult(await invoke("import_apps", { json: d.json, onConflict: conflict.value, allowJs: allowJs.checked, settings: withSettings.checked }));
        importing = null; checkApps(); renderApps();
      }) }),
      el("button", { textContent: "Cancel", onclick: cancel })));
}

// ---- edit ----
const FIELDS = {
  General: [["name", "Name", "text"], ["url", "URL", "text"], ["id", "Id", "readonly", { help: "Fixed when the app is made. Used for its files and by penates commands, e.g. penates run " }]],
  Window: [["width", "Width (px)", "number"], ["height", "Height (px)", "number"], ["zoom", "Zoom", "number", { step: 0.05 }],
    ["theme", "Window theme", "select", { options: [["system", "Follow system"], ["light", "Light"], ["dark", "Dark"]], help: "Windows and macOS. On Linux, sites follow your desktop's light/dark setting." }],
    ["start_as", "When it starts", "select", { options: [["window", "Open the window"], ["minimized", "Minimized"], ["tray", "Only in the tray"]], help: "\"Only in the tray\" needs the tray icon (under Behavior); without it the app starts minimized. Launching it again shows the window." }],
    ["start_maximized", "Start maximized", "bool", "Open full-frame; width and height apply when restored"],
    ["start_fullscreen", "Start fullscreen", "bool"], ["resizable", "Resizable", "bool"],
    ["always_on_top", "Always on top", "bool"], ["decorations", "Show title bar and borders", "bool", "Turn off for a frameless window"],
    ["toolbar", "Show a toolbar", "bool", "Back, forward, reload, start page, the address, the key list and Open in browser, above the site"],
    ["toolbar_color", "Toolbar color", "toolbarcolor"]],
  Behavior: [["single_instance", "Single window", "bool", "Launching again focuses the open window"],
    ["allow_notifications", "Allow notifications", "bool", "Let the site show desktop notifications"],
    ["allow_camera_microphone", "Allow camera and microphone", "bool", "For video calls. Off unless you turn it on"],
    ["tray", "Show system tray icon", "bool"],
    ["tray_grayscale", "Grayscale tray icon", "bool", "Draw the tray icon in black and white"],
    ["close_to_tray", "Closing the window keeps it in the tray", "bool", "Use the tray menu to show, hide or quit"],
    ["devtools", "Enable developer tools", "bool"],
    ["external_links_in_browser", "Open other sites in my browser", "bool", "Links outside this site and the hosts below leave the app"],
    ["user_agent", "User agent (blank = default)", "text"],
    ["internal_hosts", "Extra in-app hosts, one per line (*.example.com allowed)", "lines"]],
  Advanced: [["inject_css", "Custom CSS", "area"], ["inject_js", "Custom JavaScript (runs before page scripts)", "area"]],
};
/** Settings that only mean something when another one is on. */
const NEEDS = { tray_grayscale: "tray", close_to_tray: "tray" };

/** Any CSS color (hex, rgb(), hsl(), a name, oklch()...) as #rrggbb, or null if it is not a color. Drawing it on a
 *  one-pixel canvas gives its sRGB value whatever the format; transparency is dropped. */
function cssToHex(v) {
  if (!v || !CSS.supports("color", v.trim())) return null;
  const c = el("canvas", { width: 1, height: 1 }).getContext("2d");
  c.fillStyle = v.trim();
  c.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = c.getImageData(0, 0, 1, 1).data;
  return a ? "#" + [r, g, b].map((x) => x.toString(16).padStart(2, "0")).join("") : null;
}

/** Edit → Window → Toolbar color: the default, the site's, Penates's, or a custom color (a picker plus a field
 *  that takes any CSS color). The hidden input holds what is saved: default, site, penates or #rrggbb. */
function toolbarColorInput(a, k, label) {
  const value = el("input", { type: "hidden", id: "f-" + k, value: a[k] || "default" });
  const custom = value.value.startsWith("#");
  const mode = el("select", { "aria-label": label }, [["default", "Default (Settings)"], ["site", "Match the site"], ["penates", "Penates colors"], ["custom", "Custom…"]]
    .map(([v, t]) => el("option", { value: v, textContent: t, selected: (custom ? "custom" : value.value) === v })));
  const picker = el("input", { type: "color", value: custom ? value.value : "#6d3fe0", "aria-label": "Pick a color" });
  const text = el("input", { type: "text", value: custom ? value.value : "#6d3fe0", placeholder: "#5865f2, rgb(…), teal…", "aria-label": "Color" });
  const note = el("small", { class: "hint" });
  const row = el("div", { class: "row" }, picker, text);
  const sync = () => {
    row.hidden = mode.value !== "custom";
    value.value = mode.value === "custom" ? picker.value : mode.value;
  };
  mode.onchange = sync;
  picker.oninput = () => { text.value = picker.value; note.textContent = ""; sync(); };
  text.oninput = () => {
    const hex = cssToHex(text.value);
    note.textContent = text.value.trim() && !hex ? "Not a color. Try #5865f2, rgb(88 101 242) or a name like teal." : "";
    if (hex) { picker.value = hex; sync(); }
  };
  text.onchange = () => { const hex = cssToHex(text.value); if (hex) text.value = hex; };
  sync();
  return { input: value, node: el("label", { class: "f" }, label, mode, row, note, value), check: false };
}

function fieldInput(a, k, label, type, extra) {
  if (type === "toolbarcolor") return toolbarColorInput(a, k, label);
  if (type === "select") {
    const input = el("select", { id: "f-" + k }, extra.options.map(([v, t]) => el("option", { value: v, textContent: t, selected: (a[k] || extra.options[0][0]) === v })));
    return { input, node: el("label", { class: "f" }, label, input, extra.help ? el("small", { class: "hint" }, extra.help) : null), check: false };
  }
  if (type === "bool") {
    const input = el("input", { type: "checkbox", checked: !!a[k], id: "f-" + k });
    return { input, node: el("label", { class: "chk" }, input, el("span", {}, label, extra ? el("small", {}, extra) : null)), check: true };
  }
  if (type === "readonly") {
    const input = el("input", { type: "text", value: a[k], id: "f-" + k, readOnly: true });
    return { input, node: el("label", { class: "f" }, label, input, el("small", { class: "hint" }, extra.help + a[k])), check: false };
  }
  const val = type === "lines" ? (a[k] || []).join("\n") : a[k] ?? "";
  const input = type === "lines" || type === "area"
    ? el("textarea", { value: val, id: "f-" + k })
    : el("input", { type, value: val, id: "f-" + k, ...(type === "number" ? { step: extra?.step ?? 1 } : {}) });
  const node = el("label", { class: "f" }, label, input);
  if (type === "area" || type === "lines") node.style.gridColumn = "1/-1";
  return { input, node, check: false };
}

function editSections(a, inputs) {
  return Object.entries(FIELDS).map(([title, fields]) => {
    const grid = el("div", { class: title === "Window" ? "grid three" : "grid" }), checks = el("div", { class: "checks" });
    for (const [k, label, type, extra] of fields) {
      const f = fieldInput(a, k, label, type, extra);
      (f.check ? checks : grid).append(f.node);
      inputs[k] = [f.input, type];
    }
    // Behavior leads with its switches; other sections lead with their fields.
    return el("fieldset", {}, el("legend", {}, title), ...(title === "Behavior" ? [checks, grid] : [grid, checks]));
  });
}

function iconSection(a, inputs, keepEdits) {
  const candBox = el("div", { class: "icons" });
  const urlIn = el("input", { type: "text", id: "icon-url", class: "grow", placeholder: "https://example.com/logo.png", "aria-label": "Icon image address" });
  const pick = (call) => async () => { try { const s = await call(); keepEdits(s.app); sayWith("Icon updated.", s.warning); } catch (e) { fail(e); } };
  const upload = (ev) => busy(ev.target, "Choosing…", async () => {
    const f = await invoke("pick_file", { kind: "image" });
    if (f) await pick(() => invoke("set_icon_from_bytes", { id: a.id, bytes: f.bytes }))();
  });
  const find = (ev) => busy(ev.target, null, async () => {
    candBox.textContent = "Searching…";
    try {
      const cs = await invoke("icon_candidates", { url: inputs.url[0].value });
      put(candBox, cs.length
        ? cs.map((c) => el("button", { type: "button", title: c.src, "aria-label": "Use this icon", onclick: pick(() => invoke("set_icon_from_url", { id: a.id, url: c.src })) }, el("img", { src: c.data, alt: "" })))
        : el("span", { class: "hint" }, "No usable icons found on the site."));
    } catch (e) { candBox.textContent = ""; throw e; }
  });
  return [el("fieldset", {}, el("legend", {}, "Icon"),
    el("div", { class: "cur" }, iconEl(a), el("div", {},
      el("div", {}, a.custom_icon ? "Custom icon" : "Site default icon"),
      el("div", { class: "hint" }, "Changes apply as soon as you pick one."))),
    candBox,
    el("div", { class: "row" },
      el("button", { type: "button", textContent: "Find icons on site", onclick: find }),
      el("button", { type: "button", textContent: "Upload image…", onclick: upload }),
      el("button", { type: "button", textContent: "Use site default", onclick: pick(() => invoke("reset_icon", { id: a.id })) })),
    el("div", { class: "row" }, urlIn, el("button", { type: "button", textContent: "Use address", onclick: () => {
      if (!urlIn.value.trim()) return say("Paste an image address first.", "err");
      pick(() => invoke("set_icon_from_url", { id: a.id, url: urlIn.value }))();
    } })))];
}

async function renderEdit(root) {
  const a = editing, inputs = {};
  const collect = () => {
    const out = { ...a };
    for (const [k, [inp, type]] of Object.entries(inputs)) {
      if (type === "readonly") continue;
      out[k] = type === "bool" ? inp.checked : type === "number" ? parseFloat(inp.value) || 0 : type === "lines" ? inp.value.split("\n") : inp.value;
    }
    return out;
  };
  // An icon change is saved at once; keep whatever was typed in the other fields meanwhile.
  const keepEdits = (saved) => { editing = { ...collect(), icon: saved.icon, custom_icon: saved.custom_icon }; renderApps(); };
  const sections = editSections(a, inputs);
  const syncNeeds = () => { for (const [k, need] of Object.entries(NEEDS)) inputs[k][0].disabled = !inputs[need][0].checked; };
  inputs.tray[0].addEventListener("change", syncNeeds); syncNeeds();
  const saved = JSON.stringify(collect());
  const cancel = el("button", { type: "button", textContent: "Cancel", onclick: () => {
    if (JSON.stringify(collect()) !== saved && !cancel.dataset.sure) { cancel.dataset.sure = "1"; cancel.textContent = "Discard changes?"; return; }
    editing = null; renderApps();
  } });
  const form = el("form", { onsubmit: async (e) => {
    e.preventDefault();
    await busy(e.submitter || form.querySelector("button.primary"), "Saving…", async () => {
      const s = await invoke("save_app", { app: collect() });
      editing = null; sayWith("Saved. Restart the app to apply changes.", s.warning); renderApps();
    });
  } },
    el("div", { class: "toolbar" }, el("h2", { style: "margin:0" }, "Edit " + a.name),
      a.catalog_version ? el("span", { class: "chip" }, "preset " + a.catalog_version) : null),
    sections[0], ...iconSection(a, inputs, keepEdits), sections[1], sections[2], sections[3],
    el("div", { class: "formbar" }, el("button", { class: "primary", textContent: "Save" }), cancel));
  put(root, form);
}

// ---- create ----
function renderCreate() {
  const url = el("input", { type: "text", id: "c-url", placeholder: "https://example.com", autofocus: true });
  const name = el("input", { type: "text", id: "c-name", placeholder: "Name (filled in from the site)" });
  let nameTouched = false;
  name.addEventListener("input", () => (nameTouched = name.value !== ""));
  url.addEventListener("change", async () => {
    if (nameTouched || !url.value.trim()) return;
    try { const n = await invoke("suggest_name", { url: url.value }); if (!nameTouched) name.value = n; } catch {}
  });
  const open = el("input", { type: "checkbox", id: "c-open", checked: true });
  const toolbar = el("input", { type: "checkbox", id: "c-toolbar", checked: true });
  const iconUrl = el("input", { type: "text", id: "c-icon-url", placeholder: "https://example.com/logo.png" });
  // An icon file chosen with the system dialog: { name, bytes } until the app is created or it is cleared.
  let iconFile = null;
  const iconName = el("span", { class: "hint", id: "c-icon-name" }, "No file chosen");
  const clearIcon = el("button", { type: "button", textContent: "Clear", hidden: true, onclick: () => { iconFile = null; put(iconName, "No file chosen"); clearIcon.hidden = true; } });
  const chooseIcon = el("button", { type: "button", id: "c-icon-file", textContent: "Choose file…", onclick: (ev) => busy(ev.target, "Choosing…", async () => {
    const f = await invoke("pick_file", { kind: "image" });
    if (!f) return;
    iconFile = f; put(iconName, f.name); clearIcon.hidden = false;
  }) });
  put($("tab-create"),
    el("h2", {}, "Create an App"),
    el("p", { class: "hint" }, "Paste a site address. The name and icon are read from the site automatically. You can edit either one here or later under Edit."),
    el("p", { class: "hint" }, "For a well-known site, the ", link("App Catalog", () => show("catalog")), " has it ready-made, with its fixes."),
    el("form", { onsubmit: async (e) => {
      e.preventDefault();
      await busy(e.target.querySelector("button.primary"), "Creating…", async () => {
        const iconBytes = iconFile ? iconFile.bytes : null;
        const c = await invoke("create_app", { url: url.value, name: name.value || null, openAfter: open.checked, iconUrl: iconUrl.value.trim() || null, iconBytes, toolbar: toolbar.checked });
        url.value = name.value = iconUrl.value = "";
        clearIcon.click();
        checkApps(); show("apps");
        sayWith(`Created ${c.app.name}.` + (c.preset ? ` Preset ${c.preset} from the app catalog applied.` : ""), c.warnings);
      });
    } },
      el("div", { class: "grid" }, el("label", { class: "f" }, "Website address", url), el("label", { class: "f" }, "Name", name)),
      el("fieldset", {}, el("legend", {}, "Icon (Optional)"),
        el("p", { class: "hint" }, "Leave both empty to use the site's own icon."),
        el("div", { class: "grid" }, el("label", { class: "f" }, "Image address", iconUrl), el("div", { class: "f" }, el("span", {}, "Or choose a file"), el("div", { class: "row tight" }, chooseIcon, iconName, clearIcon)))),
      el("label", { class: "chk" }, toolbar, el("span", {}, "Show a toolbar", el("small", {}, "Back, forward, reload, start page, the address, the key list and Open in browser, above the site. Change it later under Edit."))),
      el("label", { class: "chk" }, open, el("span", {}, "Open after creating")),
      el("div", { class: "row" }, el("button", { class: "primary", textContent: "Create app" }))));
}

// ---- popular apps (the app catalog, browsable) ----
const hostOf = (url) => { try { return new URL(url).host.replace(/^www\./, ""); } catch { return url; } };
function letterTile(name) {
  return el("div", { class: "ph letter", "aria-hidden": "true" }, (name[0] || "?").toUpperCase());
}
/** The site's icon (fetched once and cached by the Rust side); the letter tile until it arrives. */
function catalogIcon(c) {
  const tile = letterTile(c.name);
  invoke("catalog_icon", { id: c.id, url: c.url, iconUrl: c.icon_url || null })
    .then((d) => tile.replaceWith(el("img", { src: d, alt: "" })))
    .catch(() => {}); // no icon: keep the letter
  return tile;
}
// ---- App Catalog: Popular picks, then every app by search, category and page ----
const CATALOG_PAGE = 24;
const catalogView = { search: "", category: "", page: 0 }; // kept while the manager is open

/** Twin of `CatalogApp::matches` (updates.rs): every word appears in the name, address, category or tags. */
function catalogMatches(c, search) {
  const hay = [c.name, c.url, c.category, ...c.tags].join(" ").toLowerCase();
  return search.toLowerCase().split(/\s+/).filter(Boolean).every((w) => hay.includes(w));
}

function catalogCard(c, reload) {
  const action = c.added
    ? el("button", { textContent: "Open", "aria-label": "Open " + c.name, onclick: () => invoke("run_app", { id: c.added }).catch(fail) })
    : el("button", { class: "primary", textContent: "Add", "aria-label": "Add " + c.name, onclick: (ev) => busy(ev.target, "Adding…", async () => {
        const r = await invoke("create_app", { url: c.url, name: c.name, openAfter: false, iconUrl: null, iconBytes: null });
        sayWith(`Added ${r.app.name}. Open it here, from the Apps tab or your app menu.`, r.warnings);
        checkApps(); reload();
      }) });
  return el("div", { class: "card" },
    el("div", { class: "card-head" }, catalogIcon(c), el("div", { class: "meta" }, el("b", {}, c.name), el("span", { class: "url" }, hostOf(c.url)))),
    el("p", { class: "notes" }, c.notes),
    el("div", { class: "card-foot" }, c.category ? el("span", { class: "chip" }, c.category) : el("span"),
      el("span", { class: "acts" }, c.added ? el("button", { textContent: "Edit", "aria-label": "Edit " + c.name, onclick: () => editAdded(c.added).catch(fail) }) : null, action)));
}

/** Edit an app you have from its catalog card: the same edit view as on Apps (Save or Cancel returns to Apps). */
async function editAdded(id) {
  const { apps } = await invoke("list_apps");
  const a = apps.find((x) => x.id === id);
  if (!a) return say("That app is no longer in your apps.");
  editing = structuredClone(a);
  show("apps");
}

async function renderCatalog() {
  const root = $("tab-catalog");
  if (!root.firstChild) put(root, el("h2", {}, "App Catalog"), el("p", { class: "hint" }, "Loading the app catalog…"));
  let l;
  try { l = await invoke("catalog_apps"); } catch (e) { return put(root, el("h2", {}, "App Catalog"), el("p", { class: "hint" }, "Could not load the app catalog: " + e)); }
  const reload = () => renderCatalog();
  const categories = [...new Set(l.apps.map((c) => c.category).filter(Boolean))].sort();
  const uncategorized = l.apps.some((c) => !c.category);
  if (catalogView.category && !categories.includes(catalogView.category) && catalogView.category !== "Other") catalogView.category = "";
  const popular = el("div", {}), results = el("div", {});
  const draw = () => {
    const v = catalogView;
    const shown = l.apps.filter((c) => (!v.category || (v.category === "Other" ? !c.category : c.category === v.category)) && catalogMatches(c, v.search));
    const pages = Math.max(1, Math.ceil(shown.length / CATALOG_PAGE));
    v.page = Math.min(v.page, pages - 1);
    const picks = l.apps.filter((c) => c.popular);
    put(popular, !v.search && !v.category && picks.length
      ? [el("h3", {}, "Popular"), el("div", { class: "cards" }, picks.map((c) => catalogCard(c, reload)))] : []);
    chips.forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.category === v.category)));
    put(results,
      el("p", { class: "hint", role: "status" }, shown.length === l.apps.length ? plural(l.apps.length, "app") : `${shown.length} of ${plural(l.apps.length, "app")}`),
      shown.length
        ? el("div", { class: "cards" }, shown.slice(v.page * CATALOG_PAGE, (v.page + 1) * CATALOG_PAGE).map((c) => catalogCard(c, reload)))
        : el("p", {}, "No apps match. Try another word or category."),
      pages > 1 ? el("div", { class: "pager" },
        el("button", { type: "button", textContent: "Previous", disabled: v.page === 0, onclick: () => { v.page--; draw(); } }),
        el("span", { class: "hint" }, `Page ${v.page + 1} of ${pages}`),
        el("button", { type: "button", textContent: "Next", disabled: v.page >= pages - 1, onclick: () => { v.page++; draw(); } })) : null);
  };
  const search = el("input", { type: "search", id: "catalog-search", class: "grow", value: catalogView.search, placeholder: "Search by name, address or tag", "aria-label": "Search the App Catalog",
    oninput: () => { catalogView.search = search.value; catalogView.page = 0; draw(); } });
  const chips = [["", "All"], ...categories.map((c) => [c, c]), ...(uncategorized ? [["Other", "Other"]] : [])].map(([value, label]) =>
    el("button", { type: "button", textContent: label, "data-category": value, onclick: () => { catalogView.category = value; catalogView.page = 0; draw(); } }));
  put(root, el("h2", {}, "App Catalog"),
    el("p", { class: "hint" }, `Ready-made apps from ${l.source}. Adding one also applies its fixes, such as keeping sign-in inside the window.`,
      l.problems.map((p) => el("span", { class: "warn-line" }, "Could not read " + p))),
    popular,
    el("h3", {}, "All Apps"),
    el("div", { class: "row" }, search),
    el("div", { class: "filters", role: "group", "aria-label": "Category" }, chips),
    results);
  draw();
}

// ---- updates ----
// Two checks: Penates itself (Tauri updater: signed, installs in place) and app presets.
// Each is in one of: idle (not checked yet), checking, done, error.
const checks = { penates: { state: "idle" }, apps: { state: "idle" } };
let version = "";

function selfUpdateBlock(u) {
  const install = el("button", { class: "primary", textContent: "Install and restart", onclick: (ev) => busy(ev.target, "Installing…", async () => {
    say("Downloading Penates " + u.version + "…");
    await invoke("install_self_update");
  }) });
  const download = el("button", { class: u.can_install ? "" : "primary", textContent: u.can_install ? "Release notes" : "Download", onclick: () => invoke("open_url", { url: u.releases_page }).catch(fail) });
  const how = u.can_install ? "" : u.package_manager ? " Update it the way you installed it: your package manager, or for the AUR, makepkg or an AUR helper." : " This install can't update itself; download the new version.";
  return el("div", { class: "banner" },
    el("span", {}, `Penates ${u.version} is available (you have ${u.current}).` + (u.notes ? " " + u.notes.split("\n")[0] : "") + how),
    u.can_install ? install : null, u.package_manager ? null : download);
}

/** After either check: the tab badge, the "new Penates" banner, and whichever view shows the results. */
function afterCheck() {
  const n = (checks.penates.update ? 1 : 0) + (checks.apps.report?.apps.length || 0);
  $("ubadge").hidden = !n; $("ubadge").textContent = n;
  put($("banner"), checks.penates.update ? selfUpdateBlock(checks.penates.update) : null);
  if (tab === "updates") renderUpdates();
  else if (tab === "apps" && !editing && !importing) renderApps().catch(fail); // show or hide Update buttons
}

/** Is there a newer Penates? */
async function checkPenates() {
  checks.penates = { state: "checking" }; afterCheck();
  try { checks.penates = { state: "done", update: await invoke("check_self_update") }; }
  catch (e) { checks.penates = { state: "error", error: String(e) }; }
  afterCheck();
}

/** Which of your apps have a newer preset in the app catalog? */
async function checkApps() {
  checks.apps = { state: "checking" }; afterCheck();
  try { checks.apps = { state: "done", report: await invoke("check_apps") }; }
  catch (e) { checks.apps = { state: "error", error: String(e) }; }
  afterCheck();
}

const refreshReport = () => Promise.all([checkPenates(), checkApps()]);

function penatesStatus() {
  const c = checks.penates;
  if (c.state === "idle") return el("p", { class: "hint" }, "Not checked yet.");
  if (c.state === "checking") return el("p", { class: "hint" }, "Checking…");
  if (c.state === "error") return el("p", { class: "hint" }, "Could not check for a new Penates: " + c.error);
  return c.update ? selfUpdateBlock(c.update) : el("p", {}, `Penates ${version} is up to date.`);
}

function appsStatus() {
  const c = checks.apps;
  if (c.state === "idle") return el("p", { class: "hint" }, "Not checked yet.");
  if (c.state === "checking") return el("p", { class: "hint" }, "Checking…");
  if (c.state === "error") return el("p", { class: "hint" }, "Could not check your apps: " + c.error);
  const r = c.report;
  if (!r.app_count) return el("p", {}, "No apps yet. The ", link("App Catalog", () => show("catalog")), ` has ${plural(r.catalog_size, "ready-made app")} (from ${r.source}), or make your own under `, link("Create", () => show("create")), ".");
  if (!r.apps.length) return el("p", {}, `All apps are up to date (${r.source}, ${plural(r.catalog_size, "app")}).`);
  return [el("p", {}, `${plural(r.apps.length, "app has an update", "apps have updates")} (${r.source}):`), r.apps.map((u) => el("div", { class: "app" },
    el("div", { class: "meta" }, el("b", {}, u.name), el("span", { class: "wrap" }, `Preset ${u.from ?? "none"} → ${u.to}: ${u.notes}`)),
    el("button", { textContent: "Update", onclick: (ev) => busy(ev.target, "Updating…", async () => {
      const o = await invoke("update_app", { id: u.id }); sayWith(u.name + " updated.", o.warning); checkApps();
    }) })))];
}

/** "Check now" (first time) or "Check again", disabled while that check runs. */
function checkButton(c, run) {
  return el("button", { textContent: c.state === "checking" ? "Checking…" : c.state === "idle" ? "Check now" : "Check again", disabled: c.state === "checking", onclick: run });
}

function renderUpdates() {
  const r = checks.apps.report;
  put($("tab-updates"),
    el("h2", {}, "Updates"),
    el("fieldset", {}, el("legend", {}, "Penates"),
      el("p", { class: "hint" }, "New versions of Penates itself, signed and checked before they install."),
      penatesStatus(),
      el("div", { class: "row" }, checkButton(checks.penates, checkPenates))),
    el("fieldset", {}, el("legend", {}, "Your Apps"),
      el("p", { class: "hint" }, "Fixes for known sites from the app catalog (presets, such as keeping sign-in in the window), and fresh site icons. An icon you picked yourself is never replaced."),
      appsStatus(),
      el("div", { class: "row" }, checkButton(checks.apps, checkApps),
        el("button", { class: "primary", textContent: "Update all apps", disabled: !r?.app_count,
          title: "Apply newer presets and refresh site icons for every app", onclick: (ev) => busy(ev.target, "Updating…", updateAllApps) }))),
    el("p", { class: "hint" }, "Whether Penates checks when it starts, and which app catalog it uses: ", link("Settings", () => show("settings")), "."));
}

// ---- appearance (tokens live in tokens.css; boot.js sets the attributes) ----
const ACCENTS = ["violet", "blue", "green", "orange", "rose", "slate"];
const SKINS = [["default", "Penates"], ["nord", "Nord"], ["solarized", "Solarized"], ["mono", "High contrast"]];
let appearance = window.__PENATES_APPEARANCE || { theme: "system", skin: "default", accent: "violet", toolbar_color: "site" };

/** Change the look in place: no re-render, so keyboard focus stays on the button just pressed. */
async function setAppearance(patch) {
  appearance = { ...appearance, ...patch };
  window.penatesApplyAppearance(appearance);
  document.querySelectorAll("[data-look]").forEach((b) => {
    const [key, value] = b.dataset.look.split(":");
    b.setAttribute("aria-pressed", String(appearance[key] === value));
  });
  try { await invoke("set_appearance", { appearance }); } catch (e) { fail(e); }
}

function lookButton(key, value, props, ...kids) {
  return el("button", { type: "button", "data-look": `${key}:${value}`, "aria-pressed": String(appearance[key] === value), onclick: () => setAppearance({ [key]: value }), ...props }, ...kids);
}

function appearanceSection() {
  return [el("h2", {}, "Appearance"),
    el("fieldset", {}, el("legend", {}, "Look"),
      el("div", { class: "skins", role: "group", "aria-label": "Look" },
        SKINS.map(([v, t]) => lookButton("skin", v, { class: "skin", "data-skin": v }, // the card shows its own skin
          el("span", { class: "dots" }, el("i", { style: "background:var(--card)" }), el("i", { style: "background:var(--chip)" }), el("i", { style: "background:var(--fg)" })),
          el("b", {}, t))))),
    el("fieldset", {}, el("legend", {}, "Light or Dark"),
      el("div", { class: "seg", role: "group", "aria-label": "Light or dark" },
        [["system", "System"], ["light", "Light"], ["dark", "Dark"]].map(([v, t]) => lookButton("theme", v, { textContent: t })))),
    el("fieldset", {}, el("legend", {}, "Accent Color"),
      el("div", { class: "swatches", role: "group", "aria-label": "Accent color" },
        ACCENTS.map((n) => lookButton("accent", n, { class: "swatch", title: n, "aria-label": n, "data-accent": n })))),
    el("fieldset", {}, el("legend", {}, "Toolbar Color"),
      el("div", { class: "seg", role: "group", "aria-label": "Toolbar color" },
        [["site", "Match each site"], ["penates", "Penates colors"]].map(([v, t]) => lookButton("toolbar_color", v, { textContent: t }))),
      el("p", { class: "hint" }, "For apps with a toolbar. Each app can choose its own under Edit → Window."))];
}

function updatesSection(us) {
  const save = (patch) => invoke("set_update_settings", { updates: { ...us, ...patch } }).then(() => Object.assign(us, patch));
  const onStart = el("input", { type: "checkbox", id: "s-check-start", checked: us.check_on_start,
    onchange: () => save({ check_on_start: onStart.checked }).then(() => say("Saved.")).catch(fail) });
  const catalogStatus = el("p", { class: "hint", role: "status" }, "Checking the app catalog…");
  const showCatalog = () => invoke("catalog_apps")
    .then((l) => put(catalogStatus, `Using ${l.source}: ${plural(l.apps.length, "app")}. `, link("Browse them", () => show("catalog")),
      l.problems.map((p) => el("span", { class: "warn-line" }, "Could not read " + p))))
    .catch((e) => put(catalogStatus, "Could not load the app catalog: " + e));
  const changed = (msg) => { say(msg); checkApps(); put(catalogStatus, "Checking the app catalog…"); showCatalog(); };
  showCatalog();
  // The Penates app catalog on or off, and your own catalogs, used together.
  const penates = el("input", { type: "checkbox", id: "s-penates-catalog", checked: us.penates_catalog,
    onchange: () => save({ penates_catalog: penates.checked }).then(() => changed("Saved.")).catch(fail) });
  const list = el("div", { class: "catalogs" });
  const showList = () => put(list, us.catalogs.length
    ? us.catalogs.map((src) => el("div", { class: "row catalog" }, el("code", { class: "url grow" }, src),
      el("button", { type: "button", textContent: "Remove", "aria-label": "Remove " + src, onclick: () =>
        save({ catalogs: us.catalogs.filter((c) => c !== src) }).then(() => { showList(); changed("Removed."); }).catch(fail) })))
    : el("p", { class: "hint" }, "None yet."));
  showList();
  const source = el("input", { type: "text", id: "s-catalog-add", class: "grow", placeholder: "https://…, file://… or a path to a catalog .json", "aria-label": "Catalog to add" });
  const add = async (src) => {
    src = src.trim();
    if (!src) return;
    if (us.catalogs.includes(src)) return say("That catalog is already in the list.");
    const n = await invoke("check_catalog", { source: src }).catch((e) => { throw "Could not use that catalog: " + e; });
    await save({ catalogs: [...us.catalogs, src] });
    source.value = "";
    showList();
    changed(`Added: ${plural(n, "app")}.`);
  };
  const addButton = el("button", { type: "button", textContent: "Add", onclick: (ev) => busy(ev.target, "Checking…", () => add(source.value).catch(fail)) });
  source.addEventListener("keydown", (e) => { if (e.key === "Enter") addButton.click(); });
  const choose = el("button", { type: "button", textContent: "Choose file…", onclick: (ev) => busy(ev.target, "Choosing…", async () => {
    const file = await invoke("pick_catalog_file");
    if (file) await add(file).catch(fail);
  }) });
  return [el("h2", {}, "Updates"),
    el("fieldset", {}, el("legend", {}, "Checking"),
      el("label", { class: "chk" }, onStart, el("span", {}, "Check for updates when Penates starts",
        el("small", {}, "Both kinds: a new Penates, and updates for your apps"))),
      el("p", { class: "hint" }, "To check now or install updates, go to ", link("Updates", () => show("updates")), ".")),
    el("fieldset", {}, el("legend", {}, "App Catalog"),
      el("p", { class: "hint" }, "Where your apps' fixes and the App Catalog come from."),
      el("label", { class: "chk" }, penates, el("span", {}, "Use the Penates app catalog",
        el("small", {}, "Ready-made apps for popular sites, kept up to date"))),
      el("h3", {}, "Your Catalogs"),
      el("p", { class: "hint" }, "Used together with it, for sites of your own or presets you are trying out. Yours come first: an entry with the same id as one in the Penates catalog replaces it, and the first catalog listed wins."),
      list,
      el("div", { class: "row" }, source, addButton, choose),
      catalogStatus)];
}

/** Settings → where exports and backups are saved: Downloads unless a folder is chosen. */
function exportFolderSection(info) {
  const shown = el("code", { class: "url" }, info.folder);
  const note = el("span", { class: "hint" }, info.chosen ? "" : " (your Downloads folder)");
  const reset = el("button", { type: "button", textContent: "Use Downloads", hidden: !info.chosen });
  const show = (i) => { put(shown, i.folder); put(note, i.chosen ? "" : " (your Downloads folder)"); reset.hidden = !i.chosen; };
  reset.onclick = () => invoke("set_export_folder", { dir: "" }).then((i) => { show(i); say("Exports go to your Downloads folder."); }).catch(fail);
  const choose = el("button", { type: "button", textContent: "Choose folder…", onclick: (ev) => busy(ev.target, "Choosing…", async () => {
    const dir = await invoke("pick_export_folder");
    if (!dir) return;
    show(await invoke("set_export_folder", { dir }));
    say("Exports go to " + dir + ".");
  }) });
  return el("fieldset", {}, el("legend", {}, "Export Folder"),
    el("p", { class: "hint" }, "Where Export, Export all and backups are saved. Existing files are never overwritten."),
    el("p", {}, shown, note),
    el("div", { class: "row" }, choose, reset));
}

async function renderSettings() {
  const [us, folder] = await Promise.all([invoke("get_update_settings"), invoke("get_export_folder")]);
  put($("tab-settings"), appearanceSection(), updatesSection(us),
    el("h2", {}, "Backup and Export"),
    exportFolderSection(folder),
    el("fieldset", {}, el("legend", {}, "Back Up Everything"),
      el("p", { class: "hint" }, "Saves every app plus these settings to one .penates file in your export folder. Restore it with Import on the Apps tab. It never includes logins or cookies."),
      el("div", { class: "row" }, el("button", { type: "button", textContent: "Back up apps and settings", onclick: async () => {
        try { say("Saved to " + (await invoke("export_apps", { ids: [], withSettings: true }))); } catch (e) { fail(e); } } }))),
    el("p", { class: "hint" }, "Each app's own window theme is under Edit → Window."),
    el("p", {}, el("button", { type: "button", textContent: "About Penates", onclick: openAbout })));
}

// ---- about ----
const RELEASES_URL = "https://github.com/Natetronn/penates/releases/latest";
const SITE_URL = "https://getpenates.com/";
function openAbout() { showAboutMain(false); $("about").showModal(); }

/** A link that opens in the system browser. */
const webLink = (text, url) => el("a", { href: "#", textContent: text, onclick: (e) => { e.preventDefault(); invoke("open_url", { url }).catch(fail); } });

/** Load a generated script (ui/credits.js, ui/licenses.js) once, and return the global it defines. */
function loadGenerated(file, global) {
  if (window[global]) return Promise.resolve(window[global]);
  return new Promise((resolve, reject) => {
    const s = el("script", { src: file });
    s.onload = () => resolve(window[global]);
    s.onerror = () => reject("could not load " + file);
    document.head.append(s);
  });
}

/** Every component's copyright notices and license text (THIRD-PARTY-LICENSES.txt). */
async function showLicenseTexts() {
  const page = $("about-credits");
  const back = el("button", { type: "button", textContent: "← Credits", onclick: showCredits });
  const text = el("pre", { class: "preview license-text", tabIndex: 0 }, "Loading…");
  put(page, el("div", { class: "about-head" }, el("h2", {}, "License Texts"), back), text);
  $("about-main").hidden = true; page.hidden = false; back.focus();
  try { text.textContent = await loadGenerated("licenses.js", "PENATES_LICENSES"); } catch (e) { text.textContent = String(e); }
}

function showAboutMain(focus = true) {
  $("about-credits").hidden = true; $("about-main").hidden = false;
  if (focus) $("about-credits-open").focus();
}

async function showCredits() {
  const page = $("about-credits"), list = el("div", { class: "credits-list", role: "list" }, el("span", { class: "hint" }, "Loading…"));
  const back = el("button", { type: "button", textContent: "← Back", onclick: () => showAboutMain() });
  put(page,
    el("div", { class: "about-head" }, el("h2", {}, "Credits"), back),
    el("dl", { class: "about-facts" },
      el("dt", {}, "Made by"), el("dd", {}, "Natetronn: the idea, direction, testing and the logo"),
      el("dt", {}, "With"), el("dd", {}, webLink("Claude", "https://www.anthropic.com/claude"), " by Anthropic: code, docs and design, written in conversation with Natetronn"),
      el("dt", {}, "Built on"), el("dd", {}, webLink("Tauri", "https://tauri.app/"), ", ", webLink("Rust", "https://www.rust-lang.org/"),
        ", and your system's web view: WebKitGTK, WebView2 or WKWebView"),
      el("dt", {}, "Typeface"), el("dd", {}, webLink("Space Grotesk", "https://fonts.google.com/specimen/Space+Grotesk"),
        " by Florian Karsten (SIL Open Font License), in the logo")),
    el("h3", {}, "Open-Source Components"),
    el("p", { class: "hint" }, "Penates is built from these projects, each under its own license. Thank you to everyone who makes them. ",
      el("a", { href: "#", textContent: "Full license texts", onclick: (e) => { e.preventDefault(); showLicenseTexts(); } })),
    list);
  $("about-main").hidden = true; page.hidden = false; back.focus();
  try {
    const items = await loadGenerated("credits.js", "PENATES_CREDITS");
    put(list, items.map(([name, ver, license, src]) =>
      el("div", { role: "listitem" }, src ? webLink(name, src) : name, el("span", { class: "hint" }, ` ${ver} · ${license}`))));
  } catch (e) { put(list, el("span", { class: "hint" }, String(e))); }
}
$("about-credits-open").onclick = showCredits;
$("ver").onclick = openAbout;
$("about-close").onclick = () => $("about").close();
$("about").addEventListener("click", (e) => { if (e.target === $("about")) $("about").close(); });
$("about-releases").onclick = (e) => { e.preventDefault(); invoke("open_url", { url: RELEASES_URL }).catch(fail); };
$("about-site").onclick = (e) => { e.preventDefault(); invoke("open_url", { url: SITE_URL }).catch(fail); };

function render() {
  const r = { apps: renderApps, catalog: renderCatalog, create: renderCreate, updates: renderUpdates, settings: renderSettings }[tab];
  Promise.resolve(r()).catch(fail);
}

(async () => {
  if (!window.__PENATES_APPEARANCE) {
    try { appearance = await invoke("get_appearance"); window.penatesApplyAppearance(appearance); } catch {}
  }
  version = await invoke("app_version");
  $("ver").textContent = "v" + version;
  $("about-ver").textContent = "Penates " + version;
  const start = window.__PENATES_START || location.hash.slice(1);
  if (start.startsWith("edit:")) {
    const found = (await invoke("list_apps")).apps.find((x) => x.id === start.slice(5));
    if (found) editing = structuredClone(found);
  }
  show(TABS.includes(start) ? start : "apps");
  if (start === "about") openAbout();
  if ((await invoke("get_update_settings")).check_on_start) refreshReport();
})();
