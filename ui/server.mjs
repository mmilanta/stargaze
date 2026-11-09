import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { watch } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve, extname, sep } from 'node:path';

const root = resolve(fileURLToPath(new URL('.', import.meta.url)));
const args = process.argv.slice(2);
const port = Number(args[args.indexOf('--port') + 1]) || Number(process.env.PORT) || 5188;
const explore = args.includes('--explore');
const clients = new Set();
const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.png': 'image/png', '.svg': 'image/svg+xml', '.ttf': 'font/ttf', '.json': 'application/json', '.txt': 'text/plain; charset=utf-8' };
const server = createServer(async (req, res) => {
  try {
    const url = new URL(req.url, 'http://localhost');
    if (url.pathname === '/__events') {
      res.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-cache', Connection: 'keep-alive' });
      res.write(': connected\n\n'); clients.add(res); req.on('close', () => clients.delete(res)); return;
    }
    if (url.pathname === '/runtime.json') {
      res.writeHead(200, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
      res.end(JSON.stringify({ explore })); return;
    }
    const path = resolve(root, '.' + decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname));
    if (!path.startsWith(root + sep) || path.includes(`${sep}.git${sep}`) || !['.html', '.js', '.css', '.png', '.svg', '.ttf', '.json', '.txt'].includes(extname(path))) {
      res.writeHead(404); res.end('Not found'); return;
    }
    if (!(await stat(path)).isFile()) throw new Error('Not a file');
    res.writeHead(200, { 'Content-Type': types[extname(path)], 'Cache-Control': 'no-store' });
    res.end(await readFile(path));
  } catch { res.writeHead(404); res.end('Not found'); }
});
let reload;
for (const folder of ['.', 'src']) {
  watch(resolve(root, folder), (_, name) => {
    if (!name || !/\.(html|js|css)$/.test(name)) return;
    clearTimeout(reload); reload = setTimeout(() => { for (const client of clients) client.write('data: reload\n\n'); }, 100);
  });
}
const heartbeat = setInterval(() => { for (const client of clients) client.write(': heartbeat\n\n'); }, 15000);
server.on('error', error => { console.error(error.message); process.exitCode = 1; clearInterval(heartbeat); });
server.listen(port, '127.0.0.1', () => console.log(`Stargaze UI → http://localhost:${port}${explore ? '/?explore=1' : '/'}\nHTML/CSS/JS changes reload automatically. Ctrl+C to stop.`));
