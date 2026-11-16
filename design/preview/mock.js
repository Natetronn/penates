// In-memory stand-in for the Tauri backend, so ui/app.js runs in a plain browser (preview only).
// Mirrors src-tauri/src/commands.rs: same command names, arguments and result shapes.
// CATALOG is the real preset catalog, injected by build.py from crates/penates-core/data/presets.json;
// VERSION is the version in Cargo.toml. The sample self-update is the next patch version.
const NEXT = VERSION.replace(/\d+$/, (n) => String(Number(n) + 1));
const COLORS = ['#5865f2', '#2f5bea', '#c0392b', '#6d4aff', '#d9822b', '#0f8b9d'];
function mkIcon(letter, color) {
  const c = document.createElement('canvas'); c.width = c.height = 128; const x = c.getContext('2d');
  x.fillStyle = color; x.beginPath(); x.roundRect(8, 8, 112, 112, 26); x.fill(); x.fillStyle = '#fff'; x.font = '600 64px system-ui,sans-serif';
  x.textAlign = 'center'; x.textBaseline = 'middle'; x.fillText(letter, 64, 68); return c.toDataURL('image/png');
}
const DEF = { width: 1200, height: 800, zoom: 1, user_agent: null, inject_css: null, inject_js: null, internal_hosts: [], single_instance: true,
  start_maximized: true, start_fullscreen: false, start_as: 'window', resizable: true, always_on_top: false, decorations: true, toolbar: true, toolbar_color: 'default', external_links_in_browser: true, devtools: false,
  allow_notifications: true, allow_camera_microphone: false, tray: false, tray_grayscale: false, close_to_tray: false, theme: 'system', custom_icon: false,
  catalog_id: null, catalog_version: null, launcher: true };
const icons = {};
const db = [['discord', 'Discord', 'https://discord.com/app', null], ['github', 'GitHub', 'https://github.com/', '1.0.0'], ['notion', 'Notion', 'https://www.notion.so/', null],
  ['proton-mail', 'Proton Mail', 'https://mail.proton.me/', null], ['youtube-music', 'YouTube Music', 'https://music.youtube.com/', null]]
  .map(([id, name, url, cv], i) => { icons[id] = mkIcon(name[0], COLORS[i]); return { ...DEF, id, name, url, icon: 'mock:' + id, catalog_version: cv, catalog_id: cv ? id : null }; });
const CAT = Object.fromEntries(CATALOG.catalog.map((e) => [e.id, { v: e.version, hosts: e.internal_hosts || [], ua: e.user_agent, css: e.inject_css, notes: e.notes, name: e.name, url: e.url }]));
let mockAppearance = window.__PENATES_APPEARANCE;
let settings = { penates_catalog: true, catalogs: [], check_on_start: true };
// The demo reads no catalogs of your own: one you add counts as read, with a few sample apps.
const catalogSource = () => {
  const n = settings.catalogs.length;
  if (settings.penates_catalog) return n ? `the Penates app catalog and ${n} of your own` : 'the Penates app catalog';
  return n === 1 ? `your catalog ${settings.catalogs[0]}` : n ? `${n} catalogs of your own` : 'no catalog (the Penates app catalog is off and none of your own is set)';
};
const catalogEntries = () => settings.penates_catalog ? CATALOG.catalog : [];
let exportDir = '';
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const clone = (x) => JSON.parse(JSON.stringify(x));
const find = (id) => { const a = db.find((a) => a.id === id); if (!a) throw 'no app with id \'' + id + '\''; return a; };
const slug = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
const bare = (h) => h.replace(/^www\./, '');
function normUrl(u) { u = u.trim(); if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(u)) u = 'https://' + u; try { const p = new URL(u); if (!/^https?:$/.test(p.protocol)) throw 0; return p.href; } catch { throw 'That is not a valid http(s) address. Try https://example.com.'; } }
function entryFor(a) { return a.catalog_id ? CAT[a.catalog_id] && [a.catalog_id, CAT[a.catalog_id]] : Object.entries(CAT).find(([, e]) => bare(new URL(e.url).hostname) === bare(new URL(a.url).hostname)); }
const newer = (v, have) => v.localeCompare(have || '0', undefined, { numeric: true }) > 0;
function applyPreset(a) {
  const f = entryFor(a); if (!f) return null; const [cid, e] = f;
  if (!newer(e.v, a.catalog_version)) return null;
  for (const h of e.hosts) if (!a.internal_hosts.includes(h)) a.internal_hosts.push(h);
  if (e.ua) a.user_agent = e.ua; if (e.css) a.inject_css = e.css;
  a.catalog_id = cid; a.catalog_version = e.v; return e.v;
}
const nameFor = (url) => { const h = bare(new URL(url).hostname); const known = Object.values(CAT).find((e) => bare(new URL(e.url).hostname) === h); if (known) return known.name; const p = h.split('.'); const b = p.length > 1 ? p[p.length - 2] : p[0]; return b[0].toUpperCase() + b.slice(1); };
const sample = [['linear', 'Linear', 'https://linear.app/', false], ['cloudflare', 'Cloudflare', 'https://dash.cloudflare.com/', true], ['github', 'GitHub', 'https://github.com/', false]];
const H = {
  list_apps: () => ({ apps: clone(db).sort((a, b) => a.name.localeCompare(b.name)), broken: [] }),
  app_version: () => VERSION,
  async check_self_update() { await wait(500); return { current: VERSION, version: NEXT, notes: 'Sample update, shown so the preview has something to install.', can_install: true, package_manager: false, releases_page: 'https://github.com/Natetronn/penates/releases/latest' }; },
  async install_self_update() { await wait(800); say('Would download Penates ' + NEXT + ', verify its signature, install it and restart (preview only).'); },
  cache_info: ({ id }) => ({ bytes: (find(id).name.length * 7340032) % 94371840 + 3145728, running: false }),
  async clear_cache({ id }) { await wait(400); find(id); return 12582912; },
  clear_data({ id }) { find(id); },
  export_apps({ ids }) { return (exportDir || '~/Downloads') + '/' + (ids.length === 1 ? ids[0] : 'penates-backup') + '.penates'; },
  async inspect_import({ source, isUrl }) {
    await wait(300);
    if (isUrl && !/^https?:\/\//.test(source.trim())) throw source + ': not a web address';
    const summary = sample.map(([id, name, url, js]) => ({ id, name, url, custom_js: js, custom_css: id === 'linear', user_agent: false, has_icon: true,
      extra_hosts: CAT[id] ? CAT[id].hosts : [], all_links_in_app: false, camera_microphone: id === 'cloudflare', devtools: false, exists: db.some((a) => a.id === id) }));
    return { json: 'sample', summary, has_settings: true };
  },
  async import_apps({ onConflict, allowJs, settings: withSettings }) {
    await wait(600);
    const r = { imported: [], skipped: [], renamed: [], js_removed: [], permissions_reset: [], settings_applied: !!withSettings, errors: [] };
    for (const [id, name, url, js] of sample) {
      const ex = db.some((a) => a.id === id); if (ex && onConflict === 'skip') { r.skipped.push(id); continue; }
      let nid = id; if (ex && onConflict === 'rename') { let n = 2; while (db.some((a) => a.id === id + '-' + n)) n++; nid = id + '-' + n; r.renamed.push([id, nid]); }
      if (ex && onConflict === 'overwrite') db.splice(db.findIndex((a) => a.id === id), 1);
      icons[nid] = mkIcon(name[0], COLORS[db.length % COLORS.length]); db.push({ ...DEF, id: nid, name, url, icon: 'mock:' + nid, inject_js: js && allowJs ? '/* custom */' : null });
      if (js && !allowJs) r.js_removed.push(nid); if (id === 'cloudflare') r.permissions_reset.push(nid); r.imported.push(nid);
    }
    return r;
  },
  get_appearance: () => ({ ...mockAppearance }),
  set_appearance({ appearance: a }) { mockAppearance = a; },
  async suggest_name({ url }) { await wait(400); return nameFor(normUrl(url)); },
  async create_app({ url, name, openAfter, iconUrl, iconBytes, toolbar }) {
    await wait(700); url = normUrl(url);
    if (iconUrl && !/^https?:\/\//.test(iconUrl)) throw 'The icon address must start with https:// or http://';
    name = (name || '').trim() || nameFor(url);
    const base = slug(name) || slug(nameFor(url)) || 'app'; let id = base, n = 2; while (db.some((a) => a.id === id)) id = base + '-' + n++;
    icons[id] = mkIcon(name[0].toUpperCase(), COLORS[db.length % COLORS.length]);
    const a = { ...DEF, id, name, url, icon: 'mock:' + id, toolbar: toolbar !== false };
    if (iconBytes && iconBytes.length) { icons[id] = await new Promise((r) => { const fr = new FileReader(); fr.onload = () => r(fr.result); fr.readAsDataURL(new Blob([new Uint8Array(iconBytes)])); }); a.custom_icon = true; }
    else if (iconUrl) { icons[id] = mkIcon('↗', '#0f8b9d'); a.custom_icon = true; }
    const preset = applyPreset(a); db.push(a); if (openAfter) say('Launching ' + name + ' (preview only)…');
    return { app: clone(a), preset, launcher: '~/.local/share/applications/penates-' + id + '.desktop', warnings: [] };
  },
  save_app({ app }) {
    const a = find(app.id); if (!app.name.trim()) throw 'name cannot be empty'; app.url = normUrl(app.url);
    app.width = Math.min(Math.max(app.width, 200), 10000); app.height = Math.min(Math.max(app.height, 200), 10000); app.zoom = Math.min(Math.max(app.zoom, 0.25), 5);
    app.internal_hosts = app.internal_hosts.map((h) => h.trim().toLowerCase()).filter(Boolean);
    const bad = app.internal_hosts.find((h) => !/^(\*\.)?[a-z0-9-]+(\.[a-z0-9-]+)+$/.test(h)); if (bad) throw `'${bad}' is not a host name (use example.com or *.example.com)`;
    for (const k of ['user_agent', 'inject_css', 'inject_js']) if (!(app[k] || '').trim()) app[k] = null;
    Object.assign(a, app, { icon: a.icon, custom_icon: a.custom_icon }); return { app: clone(a), warning: null };
  },
  async update_app({ id }) { await wait(800); const a = find(id); return { app: clone(a), icon_refreshed: false, preset: applyPreset(a), warning: null }; },
  async update_all() { await wait(900); return { updated: db.map((a) => ({ app: clone(a), icon_refreshed: false, preset: applyPreset(a), warning: null })), failed: [] }; },
  async check_apps() {
    await wait(500);
    const apps = db.map((a) => [a, entryFor(a)]).filter(([a, f]) => f && newer(f[1].v, a.catalog_version))
      .map(([a, [, e]]) => ({ id: a.id, name: a.name, from: a.catalog_version, to: e.v, notes: e.notes }));
    return { current: VERSION, source: catalogSource(), problems: [], catalog_size: catalogEntries().length, app_count: db.length, apps };
  },
  async catalog_apps() {
    await wait(300);
    const apps = catalogEntries().map((e) => { const mine = db.find((a) => { const f = entryFor(a); return f && f[0] === e.id; });
      return { id: e.id, name: e.name || e.id, url: e.url, notes: e.notes, icon_url: e.icon_url || null, category: e.category || '', tags: e.tags || [], popular: !!e.popular, added: mine ? mine.id : null }; })
      .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    return { source: catalogSource(), problems: [], apps };
  },
  async catalog_icon({ id }) {
    await wait(200 + Math.random() * 600);
    const e = CAT[id]; if (!e) throw 'unknown catalog app';
    return mkIcon((e.name || id)[0], COLORS[Object.keys(CAT).indexOf(id) % COLORS.length]);
  },
  async catalog_submission({ id, notes }) {
    await wait(150); const a = find(id); notes = notes.split(/\s+/).filter(Boolean).join(' ');
    if (!notes) throw 'say in one line what the preset fixes';
    if (!a.url.startsWith('https://')) throw 'only https sites can go in the app catalog';
    const f = entryFor(a), change = !!f;
    const v = change ? f[1].v.replace(/(\d+)$/, (n) => +n + 1) : '1.0.0';
    const e = { id: change ? f[0] : a.id.replace(/_/g, '-'), version: v, name: a.name, url: a.url };
    if (a.internal_hosts.length) e.internal_hosts = a.internal_hosts; if (a.user_agent) e.user_agent = a.user_agent; if (a.inject_css) e.inject_css = a.inject_css;
    e.notes = notes;
    return { kind: change ? 'change' : 'new', json: JSON.stringify(e, null, 2) + '\n', url: 'https://github.com/Natetronn/penates-apps/' + (change ? 'issues/new' : 'new/main/apps') };
  },
  get_update_settings: () => ({ ...settings }),
  get_export_folder: () => ({ chosen: exportDir, folder: exportDir || '/home/you/Downloads' }),
  set_export_folder({ dir }) { exportDir = dir; return { chosen: exportDir, folder: exportDir || '/home/you/Downloads' }; },
  async pick_export_folder() { await wait(300); return '/home/you/Backups/Penates'; },
  // The demo has no file dialog: it "chooses" the Penates logo as an icon, or a sample export to import.
  async pick_file({ kind }) {
    await wait(300);
    if (kind === 'image') {
      const svg = document.querySelector('.hlogo')?.src || '';
      const png = await new Promise((r) => { const i = new Image(); i.onload = () => { const c = document.createElement('canvas'); c.width = c.height = 128; c.getContext('2d').drawImage(i, 0, 0, 128, 128); c.toBlob((b) => r(b)); }; i.onerror = () => r(null); i.src = svg; });
      return png ? { name: 'logo.png', bytes: Array.from(new Uint8Array(await png.arrayBuffer())) } : null;
    }
    return { name: 'sample.penates', bytes: Array.from(new TextEncoder().encode('sample')) };
  },
  async pick_catalog_file() { await wait(300); return '/home/you/penates/my-catalog.json'; },
  async check_catalog({ source }) {
    await wait(400);
    if (!/^(https:\/\/|file:\/\/)|^[^:]+$/.test(source)) throw 'a catalog must be an https:// or file:// address, or a path';
    return 3;
  },
  set_update_settings({ updates }) { settings = updates; },
  open_url({ url }) { say('Would open ' + url + ' in your browser (preview only).'); },
  async problem_report({ what }) {
    await wait(200);
    const text = 'Penates 0.1.4 (Arch package penates-bin)\nSystem: CachyOS, KDE, wayland, x86_64\nWeb view: WebKitGTK 2.48.3\nApps: ' + db.length
      + '\n\nRecent log (oldest first, 2 lines):\n2026-11-16T07:12:03Z WARN 0.1.4 app discord: popup window: the window could not be created'
      + '\n2026-11-16T07:14:40Z ERROR 0.1.4 manager: could not reach https://example.org/… (timed out)\n';
    return { text, left_out: 0, log: '/home/you/.local/share/penates/logs/penates.log',
      link: 'https://github.com/Natetronn/penates/issues/new?template=bug.yml&what=' + encodeURIComponent(what || '') + '&report=' + encodeURIComponent(text) };
  },
  log_ui_error() {},
  clear_log() {},
  open_log_folder() { say('Would open the log folder (preview only).'); },
  remove_app({ id }) { const i = db.findIndex((a) => a.id === id); if (i < 0) throw 'no app with id \'' + id + '\''; db.splice(i, 1); },
  run_app({ id }) { say('Launching ' + find(id).name + ' (preview only)…'); },
  icon_data({ path }) { const id = path.replace('mock:', ''); if (!icons[id]) throw 'missing'; return icons[id]; },
  async icon_candidates() { await wait(600); return [['#2f5bea', 'A'], ['#d9822b', 'B'], ['#0f8b9d', 'C']].map(([c, l], i) => ({ src: 'https://example.com/icon-' + i + '.png', data: mkIcon(l, c) })); },
  async set_icon_from_url({ id, url }) {
    await wait(400); if (!/^https?:\/\//.test(url.trim())) throw 'only http(s) icons';
    const a = find(id); const m = url.match(/icon-(\d)\.png$/);
    icons[id] = m ? mkIcon('ABC'[m[1]], ['#2f5bea', '#d9822b', '#0f8b9d'][m[1]]) : mkIcon('↗', '#0f8b9d'); a.custom_icon = true; return { app: clone(a), warning: null };
  },
  async set_icon_from_bytes({ id, bytes }) {
    const a = find(id); const blob = new Blob([new Uint8Array(bytes)]);
    icons[id] = await new Promise((res, rej) => { const r = new FileReader(); r.onload = () => res(r.result); r.onerror = rej; r.readAsDataURL(blob); }); a.custom_icon = true; return { app: clone(a), warning: null };
  },
  async reset_icon({ id }) { await wait(400); const a = find(id); icons[id] = mkIcon(a.name[0], COLORS[db.indexOf(a) % COLORS.length]); a.custom_icon = false; return { app: clone(a), warning: null }; },
};
window.__TAURI_INTERNALS__ = { invoke: async (cmd, args = {}) => { const f = H[cmd]; if (!f) throw 'unknown command ' + cmd; return await f(args); } };
