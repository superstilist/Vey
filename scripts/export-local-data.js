// Run in the original PlayerVF DevTools console. This exports local app data only.
// It exports only allowlisted values and never reads Google sessions.
(() => {
  const keys = ['pvf:liked','pvf:library-sources','pvf:folders','pvf:listen-history:v1','pvf:lyrics-provider','pvf:lyrics:theme','pvf:lyrics:custom-themes','pvf:ui-font','pvf:ui-theme','pvf:auto-download-ytm','pvf:auto-stream-ytm','playervf.artMode'];
  const prefixes = ['pvf:lyrics:offset:','pvf:lyrics:video:','pvf:lyrics:v1:'];
  for (let i=0;i<localStorage.length;i++) {const k=localStorage.key(i);if(prefixes.some(p=>k?.startsWith(p)))keys.push(k);}
  const data=Object.fromEntries(keys.filter(k=>localStorage.getItem(k)!==null).map(k=>[k,localStorage.getItem(k)]));
  const url=URL.createObjectURL(new Blob([JSON.stringify(data,null,2)],{type:'application/json'}));
  const link=document.createElement('a');link.href=url;link.download='playervf-local-migration.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
})();
