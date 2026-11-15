// In-memory stand-in for the Tauri backend, so ui/index.html runs in a plain browser (preview only).
// CATALOG is the real preset catalog, injected by build.py from crates/penates-core/data/presets.json.
const COLORS=['#5865f2','#2f5bea','#c0392b','#6d4aff','#d9822b','#0f8b9d'];
function mkIcon(letter,color){const c=document.createElement('canvas');c.width=c.height=128;const x=c.getContext('2d');
  x.fillStyle=color;x.beginPath();x.roundRect(8,8,112,112,26);x.fill();x.fillStyle='#fff';x.font='600 64px system-ui,sans-serif';
  x.textAlign='center';x.textBaseline='middle';x.fillText(letter,64,68);return c.toDataURL('image/png');}
const DEF={width:1200,height:800,zoom:1,user_agent:null,inject_css:null,inject_js:null,internal_hosts:[],single_instance:true,hide_menu_bar:true,
  start_maximized:true,start_fullscreen:false,resizable:true,always_on_top:false,decorations:true,external_links_in_browser:true,devtools:false,allow_notifications:true,allow_camera_microphone:false,tray:false,tray_grayscale:false,close_to_tray:false,theme:'system',custom_icon:false,catalog_id:null,catalog_version:null};
const icons={};
const db=[['discord','Discord','https://discord.com/app',null],['github','GitHub','https://github.com/','1.0.0'],['notion','Notion','https://www.notion.so/',null],['proton-mail','Proton Mail','https://mail.proton.me/',null],['youtube-music','YouTube Music','https://music.youtube.com/',null]]
 .map(([id,name,url,cv],i)=>{icons[id]=mkIcon(name[0],COLORS[i]);return {...DEF,id,name,url,icon:'mock:'+id,catalog_version:cv,catalog_id:cv?id:null}});
const CAT=Object.fromEntries(CATALOG.catalog.map(e=>[e.id,{v:e.version,hosts:e.internal_hosts||[],ua:e.user_agent,css:e.inject_css,notes:e.notes,name:e.name,url:e.url}]));
let mockAppearance={theme:'system',skin:'default',accent:'violet'};
let settings={manifest_url:'',check_on_start:true};
const wait=(ms)=>new Promise(r=>setTimeout(r,ms));
const find=(id)=>{const a=db.find(a=>a.id===id);if(!a)throw 'no app '+id;return a};
const slug=s=>s.toLowerCase().replace(/[^a-z0-9]+/g,'-').replace(/^-|-$/g,'');
function normUrl(u){u=u.trim();if(!u.includes('://'))u='https://'+u;try{const p=new URL(u);if(!/^https?:$/.test(p.protocol))throw 0;return p.href}catch{throw 'That is not a valid http(s) address. Try https://example.com.'}}
function applyPreset(a){const e=CAT[a.id];if(!e)return null;const have=a.catalog_version||'0';
  if(e.v.localeCompare(have,undefined,{numeric:true})<=0)return null;
  for(const h of e.hosts)if(!a.internal_hosts.includes(h))a.internal_hosts.push(h);if(e.ua)a.user_agent=e.ua;if(e.css)a.inject_css=e.css;
  a.catalog_id=a.id;a.catalog_version=e.v;return e.v}
const H={
  list_apps:()=>JSON.parse(JSON.stringify(db)),
  app_version:()=>'0.1.0',
  async check_self_update(){await wait(500);return {current:'0.1.0',version:'0.1.1',notes:'Sample update, shown so the preview has something to install.',can_install:true,package_manager:false,releases_page:'https://github.com/Natetronn/penates/releases/latest'}},
  async install_self_update(){await wait(800);say('Would download Penates 0.1.1, verify its signature, install it and restart (preview only).')},
  cache_info:({id})=>({bytes:(find(id).name.length*7340032)%94371840+3145728,running:false}),
  async clear_cache({id}){await wait(400);find(id);return 12582912},
  export_apps({ids}){return '~/Downloads/'+(ids.length===1?ids[0]:'penates-backup')+'.penates'},
  async inspect_import({source,isUrl}){await wait(300);
    if(isUrl&&!/^https?:\/\//.test(source.trim()))throw 'fetching '+source+': not a web address';
    const s=[{id:'linear',name:'Linear',url:'https://linear.app/',custom_js:false,custom_css:true,user_agent:false,has_icon:true,extra_hosts:0,all_links_in_app:false,exists:false},
      {id:'cloudflare',name:'Cloudflare',url:'https://dash.cloudflare.com/',custom_js:true,custom_css:false,user_agent:false,has_icon:true,extra_hosts:4,all_links_in_app:false,exists:false},
      {id:'github',name:'GitHub',url:'https://github.com/',custom_js:false,custom_css:false,user_agent:false,has_icon:true,extra_hosts:2,all_links_in_app:false,exists:false}];
    s.forEach(x=>x.exists=db.some(a=>a.id===x.id));
    return {json:'sample',summary:s,has_settings:true}},
  async import_apps({onConflict,allowJs,settings}){await wait(600);const r={imported:[],skipped:[],renamed:[],js_removed:[],settings_applied:!!settings,errors:[]};
    for(const [id,name,url,js] of [['linear','Linear','https://linear.app/',false],['cloudflare','Cloudflare','https://dash.cloudflare.com/',true],['github','GitHub','https://github.com/',false]]){
      const ex=db.some(a=>a.id===id);if(ex&&onConflict==='skip'){r.skipped.push(id);continue}
      let nid=id;if(ex&&onConflict==='rename'){let n=2;while(db.some(a=>a.id===id+'-'+n))n++;nid=id+'-'+n}
      if(ex&&onConflict==='overwrite')db.splice(db.findIndex(a=>a.id===id),1);
      icons[nid]=mkIcon(name[0],COLORS[db.length%COLORS.length]);db.push({...DEF,id:nid,name,url,icon:'mock:'+nid,custom_js:js});
      if(js&&!allowJs)r.js_removed.push(nid);r.imported.push(nid)}
    return r},
  get_appearance:()=>({...mockAppearance}),
  set_appearance({appearance:a}){mockAppearance=a},
  async suggest_name({url}){await wait(400);const u=normUrl(url);const known=Object.fromEntries(CATALOG.catalog.map(e=>[new URL(e.url).hostname,e.name]));
    const h=new URL(u).hostname;if(known[h])return known[h];const p=h.replace(/^www\./,'').split('.');const b=p.length>1?p[p.length-2]:p[0];return b[0].toUpperCase()+b.slice(1)},
  async create_app({url,name,openAfter,iconUrl,iconBytes}){await wait(700);url=normUrl(url);
    if(iconUrl&&!/^https?:\/\//.test(iconUrl))throw 'icon URL: only http(s) icons';
    if(!name){const p=new URL(url).hostname.replace(/^www\./,'').split('.');const b=p.length>1?p[p.length-2]:p[0];name=b[0].toUpperCase()+b.slice(1)}
    let id=slug(name),n=2;while(db.some(a=>a.id===id))id=slug(name)+'-'+n++;
    icons[id]=mkIcon(name[0].toUpperCase(),COLORS[db.length%COLORS.length]);
    const a={...DEF,id,name,url,icon:'mock:'+id};const host=new URL(url).hostname;const ce=Object.entries(CAT).find(([,e])=>new URL(e.url).hostname===host);if(ce&&ce[0]!==id)CAT[id]=ce[1];
    if(iconBytes&&iconBytes.length){icons[id]=await new Promise(r=>{const fr=new FileReader();fr.onload=()=>r(fr.result);fr.readAsDataURL(new Blob([new Uint8Array(iconBytes)]))});a.custom_icon=true}
    else if(iconUrl){icons[id]=mkIcon('↗','#0f8b9d');a.custom_icon=true}
    applyPreset(a);db.push(a);if(openAfter)say('Launching '+name+' (preview only)…');return a},
  save_app({app}){const a=find(app.id);if(!app.name.trim())throw 'name cannot be empty';app.url=normUrl(app.url);
    app.width=Math.min(Math.max(app.width,200),10000);app.height=Math.min(Math.max(app.height,200),10000);app.zoom=Math.min(Math.max(app.zoom,.25),5);
    app.internal_hosts=app.internal_hosts.map(h=>h.trim()).filter(Boolean);
    for(const k of['user_agent','inject_css','inject_js'])if(!(app[k]||'').trim())app[k]=null;
    Object.assign(a,app,{icon:a.icon,custom_icon:a.custom_icon});return JSON.parse(JSON.stringify(a))},
  async update_app({id}){await wait(800);const a=find(id);const preset=applyPreset(a);
    if(!a.custom_icon)icons[id]=icons[id];return {app:JSON.parse(JSON.stringify(a)),icon_refreshed:!a.custom_icon,preset}},
  async update_all(){await wait(900);return db.map(a=>({app:a,icon_refreshed:!a.custom_icon,preset:applyPreset(a)}))},
  async check_updates(){await wait(500);const apps=db.filter(a=>CAT[a.id]&&CAT[a.id].v.localeCompare(a.catalog_version||'0',undefined,{numeric:true})>0)
      .map(a=>({id:a.id,name:a.name,from:a.catalog_version,to:CAT[a.id].v,notes:CAT[a.id].notes}));
    return {current:'0.1.0',source:settings.manifest_url||'Penates app catalog',penates:null,penates_error:null,apps}},
  get_update_settings:()=>({...settings}),
  set_update_settings({settings:s}){settings=s},
  open_url({url}){say('Would open '+url+' in your browser (preview only).')},
  remove_app({id}){const i=db.findIndex(a=>a.id===id);if(i<0)throw 'no app '+id;db.splice(i,1)},
  clear_data({id}){find(id)},
  run_app({id}){say('Launching '+find(id).name+' (preview only)…')},
  icon_data({path}){const id=path.replace('mock:','');if(!icons[id])throw 'missing';return icons[id]},
  async icon_candidates(){await wait(600);return [['#2f5bea','A'],['#d9822b','B'],['#0f8b9d','C']].map(([c,l],i)=>({src:'mock-candidate-'+i,data:mkIcon(l,c)}))},
  set_icon_from_candidate({id,src}){const a=find(id);const i=+src.split('-').pop();icons[id]=mkIcon(['A','B','C'][i],['#2f5bea','#d9822b','#0f8b9d'][i]);a.custom_icon=true;return JSON.parse(JSON.stringify(a))},
  async set_icon_from_url({id,url}){await wait(500);if(!/^https?:\/\//.test(url.trim()))throw 'only http(s) icons';const a=find(id);icons[id]=mkIcon('↗','#0f8b9d');a.custom_icon=true;return JSON.parse(JSON.stringify(a))},
  async set_icon_from_bytes({id,bytes}){const a=find(id);const blob=new Blob([new Uint8Array(bytes)]);
    icons[id]=await new Promise((res,rej)=>{const r=new FileReader();r.onload=()=>res(r.result);r.onerror=rej;r.readAsDataURL(blob)});a.custom_icon=true;return JSON.parse(JSON.stringify(a))},
  async reset_icon({id}){await wait(400);const a=find(id);icons[id]=mkIcon(a.name[0],COLORS[db.indexOf(a)%COLORS.length]);a.custom_icon=false;return JSON.parse(JSON.stringify(a))},
};
window.__TAURI__={core:{invoke:async(cmd,args={})=>{const f=H[cmd];if(!f)throw 'unknown command '+cmd;return await f(args)}}};
