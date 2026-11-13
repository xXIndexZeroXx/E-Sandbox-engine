// The simulator's source, as the engine is built from it: ngspice-rs at the revision pinned in
// Cargo.toml, with the patches in patches/ applied, in vendor/ngspice-rs (not committed;
// Cargo's [patch] section points there). Run before a build (`npm run wasm` in the app); does nothing
// when the folder is already there. Delete the folder to fetch and patch it afresh.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)));
const dir = join(root, 'vendor', 'ngspice-rs');
const patches = join(root, 'patches');
const stamp = join(dir, '.patched');
const dep = /ngspice-rs\s*=\s*\{\s*git\s*=\s*"([^"]+)",\s*rev\s*=\s*"([0-9a-f]+)"/.exec(readFileSync(join(root, 'Cargo.toml'), 'utf8'));
if (!dep) throw new Error('Cargo.toml: no pinned ngspice-rs dependency found');
const [, url, rev] = dep;
const list = readdirSync(patches).filter((f) => f.endsWith('.patch')).sort();
const want = `${rev}\n${list.join('\n')}\n`;

if (existsSync(stamp) && readFileSync(stamp, 'utf8') === want) {
	console.log(`engine source: ${rev.slice(0, 7)} with ${list.length} patch(es), already in place`);
} else {
	const git = (args, cwd) => execFileSync('git', args, { cwd, stdio: 'inherit' });
	rmSync(dir, { recursive: true, force: true });
	mkdirSync(dir, { recursive: true });
	git(['init', '-q'], dir);
	git(['remote', 'add', 'origin', url], dir);
	git(['fetch', '-q', '--depth', '1', 'origin', rev], dir);
	git(['checkout', '-q', 'FETCH_HEAD'], dir);
	rmSync(join(dir, '.git'), { recursive: true, force: true });
	for (const f of list) {
		git(['apply', '--ignore-whitespace', join(patches, f)], dir);
		console.log(`engine source: applied ${f}`);
	}
	writeFileSync(stamp, want);
}
