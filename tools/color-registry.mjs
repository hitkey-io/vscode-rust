// Extracts every `registerColor(...)` default from the VS Code sources into
// assets/themes/color-registry.json, which `theme::registry` evaluates at
// runtime for the active theme's type.
//
//   node tools/color-registry.mjs ../vscode-original/src
//
// Needs the `typescript` package (a global install is enough).
//
// Output: { "<color id>": { "dark": V, "light": V, "hcDark": V, "hcLight": V } }
// where V is null, "#rrggbb[aa]" (a colour), "id" (another colour) or
// { "f": "transparent" | "darken" | ..., "a": [V | number, ...] }.

import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { execSync } from 'node:child_process';

const require = createRequire(import.meta.url);
let ts;
try {
	ts = require('typescript');
} catch {
	const root = execSync('npm root -g').toString().trim();
	for (const r of [root, ...(process.env.NODE_PATH ?? "").split(":")].filter(Boolean)) { try { ts = require(path.join(r, "typescript")); break; } catch { } }
}

const srcRoot = path.resolve(process.argv[2] ?? '../vscode-original/src');
const out = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../assets/themes/color-registry.json');

function walk(dir, files = []) {
	for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
		const p = path.join(dir, entry.name);
		if (entry.isDirectory()) {
			if (entry.name === 'test' || entry.name === 'node_modules') { continue; }
			walk(p, files);
		} else if (entry.name.endsWith('.ts') && !entry.name.endsWith('.d.ts')) {
			files.push(p);
		}
	}
	return files;
}

const files = walk(path.join(srcRoot, 'vs')).filter(f => fs.readFileSync(f, 'utf8').includes('registerColor('));
const parsed = files.map(f => ts.createSourceFile(f, fs.readFileSync(f, 'utf8'), ts.ScriptTarget.Latest, true));

// Pass 1: variable name → colour id, and other module-level constants.
const varToId = new Map();
const localConsts = new Map(); // file → Map(name → initializer)
for (const sf of parsed) {
	const consts = new Map();
	localConsts.set(sf.fileName, consts);
	const visit = node => {
		if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer) {
			const init = unwrap(node.initializer);
			if (ts.isCallExpression(init) && calleeName(init) === 'registerColor' && ts.isStringLiteralLike(init.arguments[0])) {
				varToId.set(node.name.text, init.arguments[0].text);
			} else {
				consts.set(node.name.text, init);
			}
		}
		ts.forEachChild(node, visit);
	};
	visit(sf);
}

function unwrap(node) {
	while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression?.(node)) {
		node = node.expression;
	}
	return node;
}

function calleeName(call) {
	const e = call.expression;
	if (ts.isIdentifier(e)) { return e.text; }
	if (ts.isPropertyAccessExpression(e)) { return e.getText(); }
	return '';
}

const NAMED = {
	'Color.white': '#ffffff', 'Color.black': '#000000', 'Color.red': '#ff0000', 'Color.blue': '#0000ff',
	'Color.green': '#00ff00', 'Color.cyan': '#00ffff', 'Color.lightgrey': '#d3d3d3', 'Color.transparent': '#00000000',
};

const TRANSFORMS = new Set(['transparent', 'darken', 'lighten', 'withOpacity', 'opaque', 'oneOf', 'ifDefinedThenElse', 'lessProminent', 'mix']);

const unknown = [];

function hex2(n) { return Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, '0'); }

function value(node, sf) {
	node = unwrap(node);
	if (node.kind === ts.SyntaxKind.NullKeyword) { return null; }
	if (ts.isStringLiteralLike(node)) { return node.text; }
	if (ts.isNumericLiteral(node)) { return Number(node.text); }
	if (ts.isPrefixUnaryExpression(node) && node.operator === ts.SyntaxKind.MinusToken && ts.isNumericLiteral(node.operand)) {
		return -Number(node.operand.text);
	}
	if (ts.isPropertyAccessExpression(node) && NAMED[node.getText()]) { return NAMED[node.getText()]; }
	if (ts.isIdentifier(node)) {
		if (node.text === 'undefined') { return null; }
		const local = localConsts.get(sf.fileName)?.get(node.text);
		if (local) { return value(local, sf); }
		if (varToId.has(node.text)) { return varToId.get(node.text); }
	}
	if (ts.isPropertyAccessExpression(node)) {
		// e.g. `foo.id` on an imported colour, or an enum-like object member
		const text = node.getText();
		if (varToId.has(text)) { return varToId.get(text); }
	}
	if (ts.isCallExpression(node)) {
		const name = calleeName(node);
		if (name === 'Color.fromHex') { return value(node.arguments[0], sf); }
		if (name === 'registerColor') { return node.arguments[0].text; }
		if (TRANSFORMS.has(name)) {
			return { f: name, a: node.arguments.map(a => value(a, sf)) };
		}
		// `Color.white.transparent(0.1)`, `Color.fromHex(..).lighten(..)`
		const callee = node.expression;
		if (ts.isPropertyAccessExpression(callee) && ['transparent', 'darken', 'lighten'].includes(callee.name.text)) {
			return { f: callee.name.text, a: [value(callee.expression, sf), ...node.arguments.map(a => value(a, sf))] };
		}
	}
	if (ts.isObjectLiteralExpression(node)) {
		// A transform written out: `{ op: ColorTransformType.Mix, color, with, ratio }`
		const prop = k => node.properties.find(p => p.name?.getText() === k);
		const op = prop('op')?.initializer?.getText().replace('ColorTransformType.', '');
		const arg = k => { const p = prop(k); return p ? value(ts.isShorthandPropertyAssignment(p) ? p.name : p.initializer, sf) : null; };
		if (op === 'Mix') { return { f: 'mix', a: [arg('color'), arg('with'), arg('ratio')] }; }
	}
	if (ts.isNewExpression(node) && node.expression.getText() === 'Color') {
		const inner = unwrap(node.arguments[0]);
		if (ts.isNewExpression(inner) && inner.expression.getText() === 'RGBA') {
			const [r, g, b, a = 1] = inner.arguments.map(x => value(x, sf));
			return '#' + hex2(r) + hex2(g) + hex2(b) + (a === 1 ? '' : hex2(a * 255));
		}
	}
	unknown.push(`${path.relative(srcRoot, sf.fileName)}: ${node.getText().slice(0, 80)}`);
	return null;
}

function defaults(node, sf) {
	node = unwrap(node);
	if (ts.isObjectLiteralExpression(node)) {
		const keys = node.properties.filter(p => ts.isPropertyAssignment(p) || ts.isShorthandPropertyAssignment(p));
		const names = keys.map(p => p.name.getText());
		if (names.includes('dark') || names.includes('light')) {
			const result = { dark: null, light: null, hcDark: null, hcLight: null };
			for (const p of keys) {
				const k = p.name.getText();
				result[k] = ts.isShorthandPropertyAssignment(p) ? value(p.name, sf) : value(p.initializer, sf);
			}
			return result;
		}
	}
	if (ts.isIdentifier(node)) {
		const local = localConsts.get(sf.fileName)?.get(node.text);
		if (local && ts.isObjectLiteralExpression(unwrap(local))) { return defaults(local, sf); }
	}
	const v = value(node, sf);
	return { dark: v, light: v, hcDark: v, hcLight: v };
}

const registry = {};
const dynamic = [];
for (const sf of parsed) {
	const visit = node => {
		if (ts.isCallExpression(node) && calleeName(node) === 'registerColor') {
			const idArg = node.arguments[0];
			if (ts.isStringLiteralLike(idArg)) {
				registry[idArg.text] = node.arguments.length > 1 ? defaults(node.arguments[1], sf) : { dark: null, light: null, hcDark: null, hcLight: null };
			} else {
				dynamic.push(`${path.relative(srcRoot, sf.fileName)}: ${node.getText().slice(0, 100)}`);
			}
		}
		ts.forEachChild(node, visit);
	};
	visit(sf);
}

// The terminal's ANSI colours are registered in a loop over this table.
const term = parsed.find(sf => sf.fileName.endsWith('terminalColorRegistry.ts'));
if (term) {
	const map = localConsts.get(term.fileName).get('ansiColorMap');
	if (map && ts.isObjectLiteralExpression(map)) {
		for (const p of map.properties) {
			const id = p.name.text ?? p.name.getText().replace(/['"]/g, '');
			const def = p.initializer.properties.find(q => q.name.getText() === 'defaults');
			if (def) { registry[id] = defaults(def.initializer, term); }
		}
	}
}

// Colours contributed by built-in extensions (`contributes.colors`), e.g.
// the git extension's gitDecoration.* (colorExtensionPoint.ts).
const extRoot = path.join(srcRoot, '..', 'extensions');
for (const dir of fs.existsSync(extRoot) ? fs.readdirSync(extRoot) : []) {
	const pkg = path.join(extRoot, dir, 'package.json');
	if (!fs.existsSync(pkg)) { continue; }
	const colors = JSON.parse(fs.readFileSync(pkg, 'utf8')).contributes?.colors ?? [];
	for (const c of colors) {
		const d = c.defaults ?? {};
		registry[c.id] = {
			dark: d.dark ?? null,
			light: d.light ?? null,
			hcDark: d.highContrast ?? d.dark ?? null,
			hcLight: d.highContrastLight ?? d.light ?? null,
		};
	}
}

const sorted = Object.fromEntries(Object.keys(registry).sort().map(k => [k, registry[k]]));
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, JSON.stringify(sorted, null, '\t') + '\n');
console.log(`${Object.keys(sorted).length} colours from ${files.length} files → ${path.relative(process.cwd(), out)}`);
if (dynamic.length) { console.log(`dynamic ids (${dynamic.length}):\n  ` + dynamic.join('\n  ')); }
if (unknown.length) { console.log(`unresolved values (${unknown.length}):\n  ` + unknown.join('\n  ')); }
