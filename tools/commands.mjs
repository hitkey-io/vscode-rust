// Builds assets/keybindings/commands.json and default.json for the Keyboard
// Shortcuts editor and the command palette:
//
//   NODE_PATH=<node_modules with typescript> node tools/commands.mjs \
//       ../vscode-original /tmp/vscode-settings-dump/keybindings.json [rows.json]
//
// rows.json (optional, preferred): the Keyboard Shortcuts editor's table read
// out of a running VS Code (tools/keybindings-table.mjs): its labels and
// sources replace the ones found in the sources.
//
// commands.json: { "<command id>": { "title": "...", "category": "...",
//   "source": "<extension display name>" } } from every titled command the
//   sources register — Action2 / EditorAction descriptors (`super({ id,
//   title | label, category })`), `MenuRegistry.appendMenuItem(…, { command:
//   { id, title, category } })` — plus the built-in extensions'
//   `contributes.commands` (package.nls.json resolved).
// default.json: [{ "key", "command", "when" }] in the order of VS Code's
//   default keybindings file (dumped by tools/dump-settings), and
//   "unbound": the command ids it lists as having no keybinding.

import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
let ts;
for (const r of [null, ...(process.env.NODE_PATH ?? '').split(':').filter(Boolean)]) {
	try {
		ts = require(r ? path.join(r, 'typescript') : 'typescript');
		break;
	} catch { }
}
if (!ts) {
	console.error('needs the typescript package (set NODE_PATH)');
	process.exit(2);
}

const repo = path.resolve(process.argv[2] ?? '../vscode-original');
const keybindingsFile = process.argv[3] ?? '/tmp/vscode-settings-dump/keybindings.json';
const outDir = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../assets/keybindings');

function walk(dir, files = []) {
	for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
		const p = path.join(dir, e.name);
		if (e.isDirectory()) {
			if (e.name === 'test' || e.name === 'node_modules' || e.name === 'fixtures') { continue; }
			walk(p, files);
		} else if (e.name.endsWith('.ts') && !e.name.endsWith('.d.ts') && !e.name.includes('.test.')) {
			files.push(p);
		}
	}
	return files;
}

const files = walk(path.join(repo, 'src', 'vs'));
const sources = files.map(f => ts.createSourceFile(f, fs.readFileSync(f, 'utf8'), ts.ScriptTarget.Latest, true));

// ── Constants: `const X = 'a.b'`, `const X = localize2(…)`, enum members.
const consts = new Map(); // name → string (ids or titles)
function nodeText(node, scope) {
	if (!node) { return undefined; }
	if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) { return node.text; }
	if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression?.(node)) { return nodeText(node.expression, scope); }
	if (ts.isCallExpression(node)) {
		const callee = node.expression.getText();
		if (/(^|\.)localize2?$/.test(callee) && node.arguments.length >= 2) {
			return nodeText(node.arguments[1], scope);
		}
		return undefined;
	}
	if (ts.isObjectLiteralExpression(node)) {
		// { value: localize(…), original: '…' }
		for (const p of node.properties) {
			if (ts.isPropertyAssignment(p) && p.name.getText() === 'value') { return nodeText(p.initializer, scope); }
		}
		for (const p of node.properties) {
			if (ts.isPropertyAssignment(p) && p.name.getText() === 'original') { return nodeText(p.initializer, scope); }
		}
		// { ...localize2(…), mnemonicTitle: … }
		for (const p of node.properties) {
			if (ts.isSpreadAssignment(p)) {
				const v = nodeText(p.expression, scope);
				if (v !== undefined) { return v; }
			}
		}
		return undefined;
	}
	if (ts.isIdentifier(node) || ts.isPropertyAccessExpression(node)) {
		const key = node.getText();
		if (scope?.has(key)) { return scope.get(key); }
		if (consts.has(key)) { return consts.get(key); }
		if (ts.isPropertyAccessExpression(node) && consts.has(node.name.text)) { return consts.get(node.name.text); }
		return undefined;
	}
	if (ts.isTemplateExpression(node)) {
		let s = node.head.text;
		for (const span of node.templateSpans) {
			const v = nodeText(span.expression, scope);
			if (v === undefined) { return undefined; }
			s += v + span.literal.text;
		}
		return s;
	}
	if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.PlusToken) {
		const a = nodeText(node.left, scope), b = nodeText(node.right, scope);
		return a !== undefined && b !== undefined ? a + b : undefined;
	}
	return undefined;
}

// The variable an object literal initialises, through `as const`,
// `satisfies` and `Object.freeze(…)`.
function objectOwner(obj) {
	let n = obj.parent;
	while (n && (ts.isAsExpression(n) || ts.isParenthesizedExpression(n) || (ts.isSatisfiesExpression?.(n)) || (ts.isCallExpression(n) && /freeze$/.test(n.expression.getText())))) {
		n = n.parent;
	}
	return n && ts.isVariableDeclaration(n) && ts.isIdentifier(n.name) ? n.name.text : undefined;
}

// Several passes so constants built from other constants resolve. Each
// file's own names win over same-named ones elsewhere.
const local = new Map(sources.map(sf => [sf, new Map()]));
for (let pass = 0; pass < 3; pass++) {
	for (const sf of sources) {
		const scope = local.get(sf);
		const set = (k, v) => { consts.set(k, v); scope.set(k, v); };
		const nodeTextIn = (n) => nodeText(n, scope);
		const visit = (node) => {
			if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer) {
				const v = nodeTextIn(node.initializer);
				if (v !== undefined) { set(node.name.text, v); }
			} else if (ts.isEnumDeclaration(node)) {
				for (const m of node.members) {
					const v = m.initializer && nodeTextIn(m.initializer);
					if (v !== undefined) { set(`${node.name.text}.${m.name.getText()}`, v); }
				}
			} else if (ts.isPropertyAssignment(node) && ts.isObjectLiteralExpression(node.parent) && objectOwner(node.parent)) {
				// const Categories = Object.freeze({ View: localize2(…) })
				const v = nodeTextIn(node.initializer);
				if (v !== undefined) { set(`${objectOwner(node.parent)}.${node.name.getText()}`, v); }
			} else if (ts.isPropertyDeclaration(node) && node.initializer && (ts.isClassDeclaration(node.parent) || ts.isClassExpression(node.parent)) && node.parent.name) {
				// static readonly ID = '…'
				const v = nodeTextIn(node.initializer);
				if (v !== undefined) { set(`${node.parent.name.text}.${node.name.getText()}`, v); }
			}
			ts.forEachChild(node, visit);
		};
		visit(sf);
	}
}

// ── Command descriptors.
const commands = {};
function record(id, title, category) {
	if (!id || !title) { return; }
	const prev = commands[id];
	if (prev && prev.title && (!category || prev.category)) { return; }
	commands[id] = { title, ...(category ? { category } : {}) };
}
function prop(obj, name) {
	for (const p of obj.properties) {
		if ((ts.isPropertyAssignment(p) || ts.isShorthandPropertyAssignment(p)) && p.name.getText() === name) {
			return ts.isPropertyAssignment(p) ? p.initializer : p.name;
		}
	}
	return undefined;
}
for (const sf of sources) {
	const scope = local.get(sf);
	const visit = (node) => {
		if (ts.isObjectLiteralExpression(node)) {
			const idNode = prop(node, 'id');
			const titleNode = prop(node, 'title') ?? prop(node, 'label');
			if (idNode && titleNode) {
				const id = nodeText(idNode, scope);
				const title = nodeText(titleNode, scope);
				const catNode = prop(node, 'category');
				const category = catNode ? nodeText(catNode, scope) : undefined;
				if (id && title && /^[\w.~-]+$/.test(id) && id.includes('.')) {
					record(id, title, category);
				}
			}
		}
		ts.forEachChild(node, visit);
	};
	visit(sf);
}

// ── Built-in extensions.
const extRoot = path.join(repo, 'extensions');
for (const name of fs.readdirSync(extRoot)) {
	const pkgFile = path.join(extRoot, name, 'package.json');
	if (!fs.existsSync(pkgFile)) { continue; }
	let pkg;
	try { pkg = JSON.parse(fs.readFileSync(pkgFile, 'utf8')); } catch { continue; }
	let nls = {};
	try { nls = JSON.parse(fs.readFileSync(path.join(extRoot, name, 'package.nls.json'), 'utf8')); } catch { }
	const tr = (s) => typeof s === 'string' ? s.replace(/^%(.+)%$/, (_, k) => typeof nls[k] === 'string' ? nls[k] : nls[k]?.message ?? k) : s?.value;
	const display = tr(pkg.displayName) ?? pkg.name;
	for (const c of pkg.contributes?.commands ?? []) {
		const title = tr(c.title);
		if (!c.command || !title) { continue; }
		commands[c.command] = { title, ...(c.category ? { category: tr(c.category) } : {}), source: display };
	}
	for (const k of pkg.contributes?.keybindings ?? []) {
		if (k.command && commands[k.command]) { commands[k.command].source = display; }
	}
}

// ── Default keybindings (VS Code's own file, JSONC with a trailing list).
const raw = fs.readFileSync(keybindingsFile, 'utf8');
const unbound = [];
let inUnbound = false;
for (const line of raw.split('\n')) {
	if (/Here are other available commands/.test(line)) { inUnbound = true; continue; }
	const m = inUnbound && line.match(/^\/\/ - (.+)$/);
	if (m) { unbound.push(m[1].trim()); }
}
const json = raw.replace(/^\s*\/\/.*$/gm, '');
const bindings = JSON.parse(json).map(b => ({ key: b.key, command: b.command, ...(b.when ? { when: b.when } : {}) }));

// ── The live table: labels and sources as VS Code shows them.
const rowsFile = process.argv[4];
if (rowsFile) {
	const rows = JSON.parse(fs.readFileSync(rowsFile, 'utf8'));
	const keyLabel = (k, ui) => ({ left: '←', right: '→', up: '↑', down: '↓' })[k] ?? ({
		enter: 'Enter', escape: 'Escape', tab: 'Tab', space: 'Space', backspace: 'Backspace', delete: 'Del', insert: 'Insert',
		home: 'Home', end: 'End', pageup: 'PageUp', pagedown: 'PageDown', browserback: 'BrowserBack', browserforward: 'BrowserForward',
		numpad_add: 'NumPad_Add', numpad_subtract: 'NumPad_Subtract',
	})[k] ?? (k.startsWith('numpad') ? 'NumPad' + k.slice(6) : k.toUpperCase());
	const ui = (raw) => raw.split(' ').map(chord => {
		const parts = chord.split('+');
		const key = parts.pop();
		const has = (m) => parts.includes(m);
		const out = [];
		if (has('ctrl')) { out.push('⌃'); }
		if (has('shift')) { out.push('⇧'); }
		if (has('alt')) { out.push('⌥'); }
		if (has('cmd')) { out.push('⌘'); }
		out.push(keyLabel(key));
		return out.join('|');
	}).join(' ');
	const live = {};
	const seen = new Set();
	const sourceOf = new Map();
	for (const r of rows) {
		const m = r.title.match(/^(.*) \(([^()]+)\)$/);
		const id = r.label ? (m ? m[2] : r.id) : r.title;
		if (!id) { continue; }
		seen.add(id);
		if (r.label) { live[id] = r.label; }
		const when = r.when === '-' ? '' : r.when;
		const source = r.source !== 'System' && r.source !== 'User' ? r.source : undefined;
		sourceOf.set(`${id}\u0000${r.keys.replaceAll("| |", " ")}\u0000${when}`, source);
		if (!r.keys) { sourceOf.set(`${id}\u0000unbound`, source); }
	}
	for (const [id, label] of Object.entries(live)) {
		commands[id] = { label };
	}
	for (const [id, c] of Object.entries(commands)) {
		if (seen.has(id) && !live[id]) {
			// VS Code lists it by its id: no title.
			delete commands[id];
		} else if (!live[id] && c.title) {
			commands[id] = { label: c.category ? `${c.category}: ${c.title}` : c.title };
		}
	}
	// The running editor decides what is bound and what is listed: a
	// keybinding it doesn't show (registered by a contribution that hadn't
	// loaded) is dropped, unbound commands it lists are kept.
	const shown = new Set(rows.filter(r => r.keys).map(r => {
		const m = r.title.match(/^(.*) \(([^()]+)\)$/);
		const id = r.label ? (m ? m[2] : r.id) : r.title;
		return `${id}\u0000${r.keys.replaceAll('| |', ' ')}\u0000${r.when === '-' ? '' : r.when}`;
	}));
	const keep = (b) => shown.has(`${b.command}\u0000${ui(b.key)}\u0000${b.when ?? ''}`) || /vscode-userdata:/.test(b.when ?? '');
	const dropped = bindings.filter(b => !keep(b));
	bindings.splice(0, bindings.length, ...bindings.filter(keep));
	const listedUnbound = rows.filter(r => !r.keys).map(r => {
		const m = r.title.match(/^(.*) \(([^()]+)\)$/);
		return r.label ? (m ? m[2] : r.id) : r.title;
	});
	const boundIds = new Set(bindings.map(b => b.command));
	unbound.splice(0, unbound.length, ...listedUnbound.filter(id => id && !boundIds.has(id)));
	console.log(`${dropped.length} keybindings not shown by the editor dropped`);
	for (const b of bindings) {
		const s = sourceOf.get(`${b.command}\u0000${ui(b.key)}\u0000${b.when ?? ''}`);
		if (s) { b.source = s; }
	}
	for (let i = 0; i < unbound.length; i++) {
		const s = sourceOf.get(`${unbound[i]}\u0000unbound`);
		if (s) { unbound[i] = [unbound[i], s]; }
	}
	// Commands VS Code lists that the default keybindings file doesn't
	// (registered later than the dump ran) aren't added.
	console.log(`${rows.length} rows from the running editor`);
}

fs.mkdirSync(outDir, { recursive: true });
fs.writeFileSync(path.join(outDir, 'commands.json'), JSON.stringify(commands) + '\n');
fs.writeFileSync(path.join(outDir, 'default.json'), JSON.stringify({ bindings, unbound }) + '\n');
const titled = bindings.filter(b => commands[b.command]).length;
console.log(`${Object.keys(commands).length} titled commands; ${bindings.length} keybindings (${titled} titled); ${unbound.length} unbound`);
