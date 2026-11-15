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
const TABS = ["apps", "create", "updates", "settings"];
let tab = "apps", editing = null, importing = null;
function show(t) {
  tab = t;
  if (t !== "apps") importing = null; // an unfinished edit is kept; it is still there on Apps
  document.querySelectorAll("nav button").forEach((b) => (b.dataset.tab === t ? b.setAttribute("aria-current", "page") : b.removeAttribute("aria-current")));
  for (const n of TABS) $("tab-" + n).hidden = n !== t;
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

// ---- update all (Apps tab, Updates tab and Settings share this) ----
async function updateAllApps() {
  const r = await invoke("update_all");
  const changed = r.updated.filter((o) => o.preset || o.icon_refreshed).length;
  const warnings = r.updated.map((o) => o.warning && `${o.app.name}: ${o.warning}`).concat(r.failed.map((f) => `${f.id}: ${f.error}`));
  sayWith(`Updated ${plural(r.updated.length, "app")} (${changed} changed).`, warnings);
  await refreshReport();
  return r;
}

// ---- apps list ----
async function renderApps() {
  const root = $("tab-apps");
  if (editing) return renderEdit(root);
  if (importing) return renderImport(root);
  const { apps, broken } = await invoke("list_apps");
  const pending = pendingUpdates();
  const updateAll = el("button", { textContent: "Update all", onclick: (ev) => busy(ev.target, "Updating…", async () => { await updateAllApps(); renderApps(); }) });
  put(root,
    el("div", { class: "toolbar" },
      el("span", { class: "hint" }, plural(apps.length, "app") + " installed"),
      el("button", { textContent: "Import…", onclick: () => { importing = { stage: "pick" }; renderApps(); } }),
      el("button", { textContent: "Export all", title: "Save every app to a .penates file in Downloads", onclick: async () => {
        try { say("Saved to " + (await invoke("export_apps", { ids: [], withSettings: false }))); } catch (e) { fail(e); } } }),
      updateAll),
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
  refreshReport(); renderApps();
}
/** Apps with a newer catalog preset waiting, from the last update check. */
const pendingUpdates = () => new Set((checks.apps.report?.apps || []).map((u) => u.id));

function appMenu(a, actions, menuBtn) {
  const item = (label, fn) => el("button", { textContent: label, onclick: async () => { closeMenus(); try { await fn(); } catch (e) { fail(e); } } });
  const cache = item("Clear cache", async () => { const n = await invoke("clear_cache", { id: a.id }); say(`Cleared ${fmtBytes(n)} of cache for ${a.name}. You stay signed in.`); });
  invoke("cache_info", { id: a.id }).then((i) => { cache.textContent = `Clear cache (${fmtBytes(i.bytes)})`; cache.disabled = i.bytes === 0; }).catch(() => {});
  const menu = el("div", { class: "menu", role: "menu", "aria-label": "More actions for " + a.name },
    item("Update (preset and icon)", () => updateApp(a)),
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
    el("button", { class: "primary", textContent: "Open", onclick: open }),
    // Only when this app has a newer preset waiting; otherwise Update lives in the ⋮ menu.
    hasUpdate ? el("button", { textContent: "Update", onclick: (ev) => busy(ev.target, "Updating…", () => updateApp(a)) }) : null,
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
    hasUpdate ? el("span", { class: "chip new", title: "A newer preset is waiting in the app catalog" }, "update available")
      : a.catalog_version ? el("span", { class: "chip", title: "Catalog preset version" }, "preset " + a.catalog_version) : null,
    actions);
  return row;
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
  // No `accept` filter: WebKitGTK turns it into a file-dialog filter of MIME types only, and .penates has
  // none, so exported files would be hidden. The contents are checked on load instead.
  const file = el("input", { type: "file", id: "imp-file" });
  const url = el("input", { type: "text", id: "imp-url", class: "grow", placeholder: "https://example.com/github.penates" });
  const load = async (source, isUrl) => {
    try { importing = { stage: "review", data: await invoke("inspect_import", { source, isUrl }) }; renderApps(); } catch (e) { fail(e); }
  };
  file.onchange = async () => { const f = file.files[0]; if (f) load(await f.text(), false); };
  put(root, el("h2", {}, "Import apps"),
    el("fieldset", {}, el("legend", {}, "From a file"),
      el("label", { class: "f" }, "Choose a .penates file exported from Penates. You review it before anything is added.", file)),
    el("fieldset", {}, el("legend", {}, "From a link"),
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
  put(root, el("h2", {}, `Import ${plural(d.summary.length, "app")}`), rows,
    anyJs ? el("div", { class: "banner" }, el("span", {}, "Some of these apps include custom JavaScript. It runs inside the site and could read what you type. It is removed unless you allow it below. Only allow it if you trust where this file came from.")) : null,
    el("fieldset", {}, el("legend", {}, "Options"),
      anyExists ? el("label", { class: "f" }, "If an app already exists", conflict) : null,
      anyJs ? el("label", { class: "chk" }, allowJs, el("span", {}, "Keep custom JavaScript", el("small", {}, "Leave off unless you trust the source"))) : null,
      d.has_settings ? el("label", { class: "chk" }, withSettings, el("span", {}, "Also apply the settings in this file", el("small", {}, "Look, accent and the app catalog address. A different catalog decides which presets your apps get."))) : null,
      el("p", { class: "hint" }, "Files contain configs and icons only, never logins or cookies. Camera, microphone and developer tools are always switched off on import.")),
    el("div", { class: "formbar" },
      el("button", { class: "primary", textContent: "Import", onclick: (ev) => busy(ev.target, "Importing…", async () => {
        importResult(await invoke("import_apps", { json: d.json, onConflict: conflict.value, allowJs: allowJs.checked, settings: withSettings.checked }));
        importing = null; refreshReport(); renderApps();
      }) }),
      el("button", { textContent: "Cancel", onclick: cancel })));
}

// ---- edit ----
const FIELDS = {
  General: [["name", "Name", "text"], ["url", "URL", "text"]],
  Window: [["width", "Width (px)", "number"], ["height", "Height (px)", "number"], ["zoom", "Zoom", "number", { step: 0.05 }],
    ["theme", "Window theme", "select", { options: [["system", "Follow system"], ["light", "Light"], ["dark", "Dark"]], help: "Windows and macOS. On Linux, sites follow your desktop's light/dark setting." }],
    ["start_as", "When it starts", "select", { options: [["window", "Open the window"], ["minimized", "Minimized"], ["tray", "Only in the tray"]], help: "\"Only in the tray\" needs the tray icon (under Behavior); without it the app starts minimized. Launching it again shows the window." }],
    ["start_maximized", "Start maximized", "bool", "Open full-frame; width and height apply when restored"],
    ["start_fullscreen", "Start fullscreen", "bool"], ["resizable", "Resizable", "bool"],
    ["always_on_top", "Always on top", "bool"], ["decorations", "Show title bar and borders", "bool", "Turn off for a frameless window"]],
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

function fieldInput(a, k, label, type, extra) {
  if (type === "select") {
    const input = el("select", { id: "f-" + k }, extra.options.map(([v, t]) => el("option", { value: v, textContent: t, selected: (a[k] || extra.options[0][0]) === v })));
    return { input, node: el("label", { class: "f" }, label, input, extra.help ? el("small", { class: "hint" }, extra.help) : null), check: false };
  }
  if (type === "bool") {
    const input = el("input", { type: "checkbox", checked: !!a[k], id: "f-" + k });
    return { input, node: el("label", { class: "chk" }, input, el("span", {}, label, extra ? el("small", {}, extra) : null)), check: true };
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
  const file = el("input", { type: "file", accept: "image/png,image/jpeg,image/webp,image/gif,image/x-icon,image/vnd.microsoft.icon,.ico", hidden: true,
    onchange: () => { const f = file.files[0]; if (f) pick(async () => invoke("set_icon_from_bytes", { id: a.id, bytes: Array.from(new Uint8Array(await f.arrayBuffer())) }))(); } });
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
      el("button", { type: "button", textContent: "Upload image…", onclick: () => file.click() }),
      el("button", { type: "button", textContent: "Use site default", onclick: pick(() => invoke("reset_icon", { id: a.id })) })),
    el("div", { class: "row" }, urlIn, el("button", { type: "button", textContent: "Use address", onclick: () => {
      if (!urlIn.value.trim()) return say("Paste an image address first.", "err");
      pick(() => invoke("set_icon_from_url", { id: a.id, url: urlIn.value }))();
    } }))), file];
}

async function renderEdit(root) {
  const a = editing, inputs = {};
  const collect = () => {
    const out = { ...a };
    for (const [k, [inp, type]] of Object.entries(inputs)) {
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
  const iconUrl = el("input", { type: "text", id: "c-icon-url", placeholder: "https://example.com/logo.png" });
  const iconFile = el("input", { type: "file", id: "c-icon-file", accept: "image/png,image/jpeg,image/webp,image/gif,image/x-icon,image/vnd.microsoft.icon,.ico" });
  put($("tab-create"),
    el("h2", {}, "Create an app"),
    el("p", { class: "hint" }, "Paste a site address. The name and icon are read from the site automatically. You can edit either one here or later under Edit."),
    el("form", { onsubmit: async (e) => {
      e.preventDefault();
      await busy(e.target.querySelector("button.primary"), "Creating…", async () => {
        const f = iconFile.files[0];
        const iconBytes = f ? Array.from(new Uint8Array(await f.arrayBuffer())) : null;
        const c = await invoke("create_app", { url: url.value, name: name.value || null, openAfter: open.checked, iconUrl: iconUrl.value.trim() || null, iconBytes });
        url.value = name.value = iconUrl.value = iconFile.value = "";
        refreshReport(); show("apps");
        sayWith(`Created ${c.app.name}.` + (c.preset ? ` Preset ${c.preset} from the app catalog applied.` : ""), c.warnings);
      });
    } },
      el("div", { class: "grid" }, el("label", { class: "f" }, "Website address", url), el("label", { class: "f" }, "Name", name)),
      el("fieldset", {}, el("legend", {}, "Icon (optional)"),
        el("p", { class: "hint" }, "Leave both empty to use the site's own icon."),
        el("div", { class: "grid" }, el("label", { class: "f" }, "Image address", iconUrl), el("label", { class: "f" }, "Or choose a file", iconFile))),
      el("label", { class: "chk" }, open, el("span", {}, "Open after creating")),
      el("div", { class: "row" }, el("button", { class: "primary", textContent: "Create app" }))),
    popularApps());
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
function popularApps() {
  const box = el("div", {}, el("p", { class: "hint" }, "Loading the app catalog…"));
  const fill = async () => {
    try {
      const l = await invoke("catalog_apps");
      put(box, el("p", { class: "hint" }, `Ready-made apps from the ${l.source}. Adding one also applies its fixes, such as keeping sign-in inside the window.`),
        l.apps.map((c) => el("div", { class: "app" }, catalogIcon(c),
          el("div", { class: "meta" }, el("b", {}, c.name), el("span", { class: "url" }, hostOf(c.url)), el("span", { class: "wrap" }, c.notes)),
          c.added
            ? [el("span", { class: "chip" }, "added"), el("button", { textContent: "Open", onclick: () => invoke("run_app", { id: c.added }).catch(fail) })]
            : el("button", { class: "primary", textContent: "Add", "aria-label": "Add " + c.name, onclick: (ev) => busy(ev.target, "Adding…", async () => {
                const r = await invoke("create_app", { url: c.url, name: c.name, openAfter: false, iconUrl: null, iconBytes: null });
                sayWith(`Added ${r.app.name}. Open it from the Apps tab or your app menu.`, r.warnings);
                refreshReport(); fill();
              }) }))));
    } catch (e) { put(box, el("p", { class: "hint" }, "Could not load the app catalog: " + e)); }
  };
  fill();
  return [el("h2", {}, "Popular apps"), box];
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
  const how = u.can_install ? "" : u.package_manager ? " Update it with your package manager (for example paru -Syu)." : " This install can't update itself; download the new version.";
  return el("div", { class: "banner" },
    el("span", {}, `Penates ${u.version} is available (you have ${u.current}).` + (u.notes ? " " + u.notes.split("\n")[0] : "") + how),
    u.can_install ? install : null, u.package_manager ? null : download);
}

async function refreshReport() {
  checks.penates = { state: "checking" }; checks.apps = { state: "checking" };
  if (tab === "updates") renderUpdates();
  const [apps, self] = await Promise.allSettled([invoke("check_apps"), invoke("check_self_update")]);
  checks.apps = apps.status === "fulfilled" ? { state: "done", report: apps.value } : { state: "error", error: String(apps.reason) };
  checks.penates = self.status === "fulfilled" ? { state: "done", update: self.value } : { state: "error", error: String(self.reason) };
  const n = (checks.penates.update ? 1 : 0) + (checks.apps.report?.apps.length || 0);
  $("ubadge").hidden = !n; $("ubadge").textContent = n;
  put($("banner"), checks.penates.update ? selfUpdateBlock(checks.penates.update) : null);
  if (tab === "updates") renderUpdates();
  else if (tab === "apps" && !editing && !importing) renderApps().catch(fail); // show or hide Update buttons
}

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
  if (!r.app_count) return el("p", {}, `No apps yet. The ${r.source} has ${plural(r.catalog_size, "ready-made app")}: see `, link("Popular apps", () => show("create")), " under Create.");
  if (!r.apps.length) return el("p", {}, `All apps are up to date (${r.source}, ${plural(r.catalog_size, "app")}).`);
  return [el("p", { class: "hint" }, `From the ${r.source}.`), r.apps.map((u) => el("div", { class: "app" },
    el("div", { class: "meta" }, el("b", {}, u.name), el("span", { class: "wrap" }, `Preset ${u.from ?? "none"} → ${u.to}: ${u.notes}`)),
    el("button", { textContent: "Update", onclick: (ev) => busy(ev.target, "Updating…", async () => {
      const o = await invoke("update_app", { id: u.id }); sayWith(u.name + " updated.", o.warning); refreshReport();
    }) })))];
}

function renderUpdates() {
  const checking = checks.penates.state === "checking" || checks.apps.state === "checking";
  put($("tab-updates"),
    el("div", { class: "toolbar" }, el("h2", { style: "margin:0" }, "Updates"),
      el("div", { class: "row", style: "margin:0" },
        el("button", { textContent: checking ? "Checking…" : "Check now", disabled: checking, onclick: () => refreshReport() }),
        el("button", { class: "primary", textContent: "Update all apps", onclick: (ev) => busy(ev.target, "Updating…", updateAllApps) }))),
    el("h2", {}, "Penates"), penatesStatus(),
    el("h2", {}, "Apps"), el("p", { class: "hint" }, "Presets add fixes such as sign-in hosts for known sites. Your icon choice is never overwritten."), appsStatus(),
    el("p", { class: "hint" }, "When Penates checks, and which app catalog it uses, is under ", link("Settings", () => show("settings")), "."));
}

// ---- appearance (tokens live in tokens.css; boot.js sets the attributes) ----
const ACCENTS = ["violet", "blue", "green", "orange", "rose", "slate"];
const SKINS = [["default", "Penates"], ["nord", "Nord"], ["solarized", "Solarized"], ["mono", "High contrast"]];
let appearance = window.__PENATES_APPEARANCE || { theme: "system", skin: "default", accent: "violet" };

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
    el("fieldset", {}, el("legend", {}, "Light or dark"),
      el("div", { class: "seg", role: "group", "aria-label": "Light or dark" },
        [["system", "System"], ["light", "Light"], ["dark", "Dark"]].map(([v, t]) => lookButton("theme", v, { textContent: t })))),
    el("fieldset", {}, el("legend", {}, "Accent color"),
      el("div", { class: "swatches", role: "group", "aria-label": "Accent color" },
        ACCENTS.map((n) => lookButton("accent", n, { class: "swatch", title: n, "aria-label": n, "data-accent": n }))))];
}

function updatesSection(us) {
  const save = (patch) => invoke("set_update_settings", { updates: { ...us, ...patch } }).then(() => Object.assign(us, patch));
  const onStart = el("input", { type: "checkbox", id: "s-check-start", checked: us.check_on_start,
    onchange: () => save({ check_on_start: onStart.checked }).then(() => say("Saved.")).catch(fail) });
  const seeUpdates = () => link("See Updates", () => show("updates"));
  const coreStatus = el("span", { class: "hint", role: "status" });
  const coreBtn = el("button", { type: "button", textContent: "Check for a new Penates", onclick: (ev) => busy(ev.target, null, async () => {
    put(coreStatus, "Checking…"); await refreshReport();
    const c = checks.penates;
    put(coreStatus, c.state === "error" ? "Could not check: " + c.error : c.update ? [`Penates ${c.update.version} is available. `, seeUpdates()] : "Penates is up to date.");
  }) });
  const appsStatusEl = el("span", { class: "hint", role: "status" });
  const appsBtn = el("button", { type: "button", textContent: "Check your apps", onclick: (ev) => busy(ev.target, null, async () => {
    put(appsStatusEl, "Checking…"); await refreshReport();
    const c = checks.apps, n = c.report?.apps.length || 0;
    put(appsStatusEl, c.state === "error" ? "Could not check: " + c.error : n ? [`${plural(n, "app has an update", "apps have updates")}. `, seeUpdates()] : "All your apps are up to date.");
  }) });
  const updateAll = el("button", { type: "button", textContent: "Update all apps", onclick: (ev) => busy(ev.target, "Updating…", async () => {
    const r = await updateAllApps();
    put(appsStatusEl, `Updated ${plural(r.updated.length, "app")}: icons refreshed and newer presets applied.`);
  }) });
  const url = el("input", { type: "text", id: "s-catalog-url", value: us.catalog_url, placeholder: "Empty = the Penates app catalog" });
  const catalogStatus = el("p", { class: "hint", role: "status" }, "Checking the app catalog…");
  const showCatalog = () => invoke("catalog_apps")
    .then((l) => put(catalogStatus, `Using the ${l.source}: ${plural(l.apps.length, "app")}. `, link("Browse them", () => show("create"))))
    .catch((e) => put(catalogStatus, "Could not load the app catalog: " + e));
  showCatalog();
  return [el("h2", {}, "Updates"),
    el("label", { class: "chk", style: "margin-bottom:10px" }, onStart, el("span", {}, "Check for updates when Penates starts", el("small", {}, "Both kinds below: a new Penates and updates for your apps"))),
    el("fieldset", {}, el("legend", {}, "Penates"),
      el("p", { class: "hint" }, "New versions of Penates itself. Installs in place and restarts, where your install type allows it."),
      el("div", { class: "row update-check" }, coreBtn, coreStatus)),
    el("fieldset", {}, el("legend", {}, "Your apps"),
      el("p", { class: "hint" }, "Refreshes each app's site icon (unless you picked one) and adds fixes for known sites, such as sign-in hosts, from the app catalog."),
      el("div", { class: "row update-check" }, appsBtn, updateAll, appsStatusEl),
      el("label", { class: "f" }, "App catalog address (https://, file:// or a path; leave empty for the Penates app catalog)", url),
      el("div", { class: "row" }, el("button", { type: "button", textContent: "Save address", onclick: () =>
        save({ catalog_url: url.value.trim() }).then(() => { say("Saved."); refreshReport(); put(catalogStatus, "Checking the app catalog…"); showCatalog(); }).catch(fail) })),
      catalogStatus)];
}

async function renderSettings() {
  const us = await invoke("get_update_settings");
  put($("tab-settings"), appearanceSection(), updatesSection(us),
    el("h2", {}, "Backup"),
    el("fieldset", {}, el("legend", {}, "Back up everything"),
      el("p", { class: "hint" }, "Saves every app plus these settings to one .penates file in your Downloads folder. Restore it with Import on the Apps tab. It never includes logins or cookies."),
      el("div", { class: "row" }, el("button", { type: "button", textContent: "Back up apps and settings", onclick: async () => {
        try { say("Saved to " + (await invoke("export_apps", { ids: [], withSettings: true }))); } catch (e) { fail(e); } } }))),
    el("p", { class: "hint" }, "Each app's own window theme is under Edit → Window."),
    el("p", {}, el("button", { type: "button", textContent: "About Penates", onclick: openAbout })));
}

// ---- about ----
const RELEASES_URL = "https://github.com/Natetronn/penates/releases/latest";
function openAbout() { $("about").showModal(); }
$("ver").onclick = openAbout;
$("about-close").onclick = () => $("about").close();
$("about").addEventListener("click", (e) => { if (e.target === $("about")) $("about").close(); });
$("about-releases").onclick = (e) => { e.preventDefault(); invoke("open_url", { url: RELEASES_URL }).catch(fail); };

function render() {
  const r = { apps: renderApps, create: renderCreate, updates: renderUpdates, settings: renderSettings }[tab];
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
