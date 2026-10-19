// Static fixture host with explicit MIME types (Windows registry associations
// can otherwise label ES modules text/plain). No application API or user data.
import {createServer} from 'node:http';
import {stat} from 'node:fs/promises';
import {createReadStream} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {resolve,sep,extname} from 'node:path';
const root=fileURLToPath(new URL('../../web',import.meta.url));
const mime={'.js':'text/javascript','.css':'text/css','.wasm':'application/wasm','.html':'text/html','.json':'application/json','.txt':'text/plain','.png':'image/png','.svg':'image/svg+xml'};
createServer(async(req,res)=>{
  try {
    const path=resolve(root,'.'+decodeURIComponent(new URL(req.url,'http://localhost').pathname));
    if(path!==root&&!path.startsWith(root+sep)){res.writeHead(403).end();return;}
    const file=(await stat(path)).isDirectory()?resolve(path,'index.html'):path;
    const info=await stat(file);
    res.writeHead(200,{'Content-Type':mime[extname(file)]||'application/octet-stream','Content-Length':info.size,'Cache-Control':'no-store'});
    if(req.method==='HEAD')res.end();else createReadStream(file).pipe(res);
  }catch{res.writeHead(404).end();}
}).listen(4181,'127.0.0.1',()=>console.log('Braccato fixture host: http://127.0.0.1:4181'));
