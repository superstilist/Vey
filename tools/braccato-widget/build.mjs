import { build } from 'esbuild';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { mkdir, writeFile as writeRawFile, readFile, readdir, rename, rm } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const packages = resolve(root, 'third-party/braccato/packages');
const out = resolve(root, 'web/braccato');
await mkdir(out, { recursive: true });
// Publish changed assets by replacement: Windows can keep embedded inputs mapped.
async function writeFile(path, content) {
  if (dirname(path) !== out) throw new Error('Asset output escaped the widget directory');
  const bytes = Buffer.from(content);
  try { if ((await readFile(path)).equals(bytes)) return; } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  const temporary = path + '.tmp';
  try {
    await writeRawFile(temporary, bytes);
    await rename(temporary, path);
  } finally { await rm(temporary, { force: true }); }
}
const result = await build({
  entryPoints: [resolve(here, 'widget.ts')], outfile: resolve(out, 'widget.js'),
  write: false, bundle: true, platform: 'browser', format: 'iife', globalName: 'PlayerVFBraccato',
  target: ['chrome110'], minify: true, treeShaking: false, legalComments: 'linked',
  loader: { '.ttf': 'dataurl', '.tsv': 'text' }, metafile: true,
  nodePaths: [resolve(here, 'node_modules')],
  // Adapt the upstream view at build time, keeping the vendored source intact.
  // The host knows whether a zero-origin cue is timed; upstream's numeric
  // heuristic cannot distinguish it from plain text or start-only word cues.
  plugins: [{
    name: 'host-sync-mode',
    setup(build) {
      build.onLoad({ filter: /[\\/]core[\\/]src[\\/](view|renderer)\.ts$/ }, async ({ path }) => {
        let contents = await readFile(path, 'utf8');
        const replacements = path.endsWith('renderer.ts') ? [
          ['songwriters: options?.songwriters,', 'songwriters: options?.songwriters, syncType: (options as any)?.syncType,'],
        ] : [
          ['language?: string | null;', 'language?: string | null; syncType?: "none" | "synced" | "richsync";'],
          ['const allZero = lyrics.every(item => item.startTimeMs === 0);', 'const allZero = options.syncType ? options.syncType === "none" : lyrics.every(item => item.startTimeMs === 0);'],
          ['const syncType = deriveSyncType(lyrics);', 'const syncType = options.syncType === "richsync" && disableRichsync.getBooleanValue() ? "synced" : (options.syncType ?? deriveSyncType(lyrics));'],
        ];
        for (const [before, after] of replacements) {
          if (!contents.includes(before)) throw Error(`Upstream sync adapter needs updating: ${before}`);
          contents = contents.replace(before, after);
        }
        return { contents, loader: 'ts' };
      });
    },
  }],
  alias: {
    '@braccato/core': resolve(packages, 'core/src/index.ts'),
    '@braccato/parsers/format': resolve(packages, 'parsers/src/format.ts'),
    '@braccato/parsers': resolve(packages, 'parsers/src/index.ts'),
    '@braccato/types': resolve(packages, 'types/src/index.ts'),
  },
});
for (const file of result.outputFiles) await writeFile(file.path, file.contents);
const licenses = [await readFile(resolve(root,'third-party/braccato/LICENSE'),'utf8'),await readFile(resolve(root,'assets/fonts/OFL-Roboto.txt'),'utf8'),await readFile(resolve(root,'assets/fonts/OFL-NotoSansJP.txt'),'utf8')];
const licensed = new Set();
for (const input of Object.keys(result.metafile.inputs)) {
  let folder=dirname(resolve(process.cwd(),input));
  while(folder.startsWith(resolve(here,'node_modules'))) {
    try {
      const pkg=JSON.parse(await readFile(resolve(folder,'package.json'),'utf8'));
      if(!licensed.has(pkg.name)) {
        licensed.add(pkg.name);
        licenses.push(`\n\n${pkg.name} ${pkg.version} (${pkg.license || 'see license'})\n`);
        for(const name of await readdir(folder)) {
          if(/^(licen[sc]e|copying)(\.|$)/i.test(name)) licenses.push(await readFile(resolve(folder,name),'utf8'));
        }
      }
      break;
    }catch{const parent=dirname(folder);if(parent===folder)break;folder=parent;}
  }
}
await writeFile(resolve(out,'LICENSES.txt'),licenses.join('\n'));
const assets = {};
for (const name of ['widget.js', 'widget.css','LICENSES.txt']) {
  const bytes = await readFile(resolve(out, name));
  assets[name] = { bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
}
await writeFile(resolve(out, 'manifest.json'), JSON.stringify({
  repository: 'https://github.com/better-lyrics/braccato',
  commit: '1e6b6954b1c18136274681a9bf4985efb2332b01',
  packages: ['core', 'parsers', 'highlight', 'provider-blyrics', 'types'],
  bundledModules: Object.keys(result.metafile.inputs).length, assets,
}, null, 2) + '\n');
console.log(JSON.stringify(assets, null, 2));
