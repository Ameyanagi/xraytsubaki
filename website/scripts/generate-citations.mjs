/** Preserve citation links authored in API documentation, including where they occur. */
import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { resolve, relative } from 'node:path';
const root=resolve(import.meta.dirname,'../..');
function files(dir){return readdirSync(dir,{withFileTypes:true}).sort((a,b)=>a.name<b.name?-1:a.name>b.name?1:0).flatMap(e=>e.isDirectory()?files(resolve(dir,e.name)):[resolve(dir,e.name)]);}
const sources=[
 'py-rexafs/python/rexafs/__init__.pyi','py-rexafs/python/rexafs/io.pyi',
 'py-rexafs/src/lib.rs','crates/rexafs-wasm/src/lib.rs',
 ...readdirSync(resolve(root,'js-rexafs')).sort().filter(p=>p.endsWith('.d.ts')).map(p=>'js-rexafs/'+p),
 ...files(resolve(root,'crates/rexafs/src')).filter(p=>p.endsWith('.rs')).map(p=>relative(root,p)),
];
const found=new Map();
for(const source of sources){
 const text=readFileSync(resolve(root,source),'utf8');
 const documented=source.endsWith('.rs')?text.split('\n').filter(line=>/^\s*\/\/[/!]/.test(line)).join('\n'):text;
 for(const raw of documented.matchAll(/https?:\/\/[^\s<>`"'\])]+/g)){
   const url=raw[0].replace(/[.,;]+$/,'');
   // Retain every authored link, including specifications and cited implementations.
   new URL(url);
   if(!found.has(url))found.set(url,new Set());found.get(url).add(source);
 }
}
writeFileSync(resolve(root,'website/src/data/api-citations.json'),JSON.stringify([...found].sort(([a],[b])=>a.localeCompare(b)).map(([url,sources])=>({url,sources:[...sources]})),null,2)+'\n');
console.log(`Generated ${found.size} citation links from API documentation.`);
