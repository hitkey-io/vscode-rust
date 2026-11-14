// Extension host for vscode-rust.
//
// Runs VS Code extensions (their `main` entry point) in Node.js and gives
// them a `vscode` module that implements a subset of the extension API. The
// editor talks to this process with one JSON object per line: requests on
// stdin, events on stdout. Extensions' own console output is forwarded as
// `log` events so it never corrupts the protocol.
//
// Upstream analogues: src/vs/workbench/api/common/extHost.api.impl.ts (the
// API object), extHostCommands.ts, extHostMessageService.ts,
// extHostStatusBar.ts, extHostConfiguration.ts, extHostExtensionService.ts.

'use strict';

const Module = require('module');
const path = require('path');
const fs = require('fs');
const os = require('os');
const readline = require('readline');

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

const rawWrite = process.stdout.write.bind(process.stdout);
function send(msg) {
	rawWrite(JSON.stringify(msg) + '\n');
}

function formatArgs(args) {
	return args.map(a => {
		if (typeof a === 'string') { return a; }
		if (a instanceof Error) { return a.stack || String(a); }
		try { return JSON.stringify(a); } catch { return String(a); }
	}).join(' ');
}

for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
	console[level] = (...args) => send({ type: 'log', level, text: formatArgs(args) });
}
// Anything else written straight to stdout is wrapped as a log line too.
process.stdout.write = (chunk, ...rest) => {
	send({ type: 'log', level: 'log', text: String(chunk).replace(/\n$/, '') });
	const cb = rest.find(r => typeof r === 'function');
	if (cb) { cb(); }
	return true;
};

process.on('uncaughtException', err => send({ type: 'log', level: 'error', text: 'Uncaught: ' + (err && err.stack || err) }));
process.on('unhandledRejection', err => send({ type: 'log', level: 'error', text: 'Unhandled rejection: ' + (err && err.stack || err) }));

let nextRequestId = 1;
const pendingRequests = new Map();
function request(msg) {
	const requestId = nextRequestId++;
	return new Promise(resolve => {
		pendingRequests.set(requestId, resolve);
		send({ ...msg, requestId });
	});
}

// ---------------------------------------------------------------------------
// Basic types
// ---------------------------------------------------------------------------

class Disposable {
	static from(...items) {
		return new Disposable(() => items.forEach(i => i && typeof i.dispose === 'function' && i.dispose()));
	}
	constructor(fn) { this._fn = fn; }
	dispose() { if (this._fn) { const f = this._fn; this._fn = undefined; f(); } }
}

class EventEmitter {
	constructor() {
		this._listeners = new Set();
		this.event = (listener, thisArg, disposables) => {
			const entry = thisArg ? listener.bind(thisArg) : listener;
			this._listeners.add(entry);
			const d = new Disposable(() => this._listeners.delete(entry));
			if (Array.isArray(disposables)) { disposables.push(d); }
			return d;
		};
	}
	fire(data) { for (const l of [...this._listeners]) { try { l(data); } catch (e) { console.error(e); } } }
	dispose() { this._listeners.clear(); }
}

/** An event nothing ever fires (for API surface the editor doesn't drive yet). */
const silentEvent = (_listener, _thisArg, disposables) => {
	const d = new Disposable(() => { });
	if (Array.isArray(disposables)) { disposables.push(d); }
	return d;
};

class Uri {
	constructor(scheme, authority, p, query, fragment) {
		this.scheme = scheme || 'file';
		this.authority = authority || '';
		this.path = p || '';
		this.query = query || '';
		this.fragment = fragment || '';
	}
	get fsPath() { return this.scheme === 'file' ? path.normalize(this.path) : this.path; }
	static file(p) { return new Uri('file', '', path.resolve(p)); }
	static parse(value) {
		const m = /^([a-zA-Z][\w+.-]*):(?:\/\/([^/?#]*))?([^?#]*)(?:\?([^#]*))?(?:#(.*))?$/.exec(value);
		if (!m) { return new Uri('file', '', value); }
		return new Uri(m[1], m[2], decodeURIComponent(m[3] || ''), m[4], m[5]);
	}
	static joinPath(base, ...parts) { return base.with({ path: path.posix.join(base.path, ...parts) }); }
	static from(c) { return new Uri(c.scheme, c.authority, c.path, c.query, c.fragment); }
	with(change) {
		return new Uri(
			change.scheme ?? this.scheme, change.authority ?? this.authority, change.path ?? this.path,
			change.query ?? this.query, change.fragment ?? this.fragment);
	}
	toString() {
		const auth = this.scheme === 'file' || this.authority ? '//' + this.authority : '';
		return `${this.scheme}:${auth}${encodeURI(this.path)}${this.query ? '?' + this.query : ''}${this.fragment ? '#' + this.fragment : ''}`;
	}
	toJSON() { return { scheme: this.scheme, authority: this.authority, path: this.path, query: this.query, fragment: this.fragment, fsPath: this.fsPath }; }
}

class Position {
	constructor(line, character) { this.line = line; this.character = character; }
	isBefore(o) { return this.line < o.line || (this.line === o.line && this.character < o.character); }
	isEqual(o) { return this.line === o.line && this.character === o.character; }
	translate(dl = 0, dc = 0) { return new Position(this.line + dl, this.character + dc); }
	with(line = this.line, character = this.character) { return new Position(line, character); }
}

class Range {
	constructor(a, b, c, d) {
		if (typeof a === 'number') { this.start = new Position(a, b); this.end = new Position(c, d); }
		else { this.start = a; this.end = b; }
	}
	get isEmpty() { return this.start.isEqual(this.end); }
	contains(p) { const s = p.start || p; const e = p.end || p; return !s.isBefore(this.start) && !this.end.isBefore(e); }
}

class Selection extends Range {
	constructor(anchor, active, c, d) {
		super(anchor, active, c, d);
		this.anchor = this.start; this.active = this.end;
	}
}

class ThemeColor { constructor(id) { this.id = id; } }
class ThemeIcon {
	constructor(id, color) { this.id = id; this.color = color; }
}
ThemeIcon.File = new ThemeIcon('file');
ThemeIcon.Folder = new ThemeIcon('folder');

class MarkdownString {
	constructor(value = '', supportThemeIcons = false) { this.value = value; this.supportThemeIcons = supportThemeIcons; this.isTrusted = false; }
	appendText(v) { this.value += v; return this; }
	appendMarkdown(v) { this.value += v; return this; }
	appendCodeblock(code, lang = '') { this.value += '\n```' + lang + '\n' + code + '\n```\n'; return this; }
}

class CancellationTokenSource {
	constructor() {
		const emitter = new EventEmitter();
		this._emitter = emitter;
		this.token = { isCancellationRequested: false, onCancellationRequested: emitter.event };
	}
	cancel() { this.token.isCancellationRequested = true; this._emitter.fire(); }
	dispose() { this._emitter.dispose(); }
}

class TreeItem {
	constructor(labelOrUri, collapsibleState = 0) {
		if (typeof labelOrUri === 'string') { this.label = labelOrUri; } else { this.resourceUri = labelOrUri; }
		this.collapsibleState = collapsibleState;
	}
}

/**
 * `CodeActionKind`-style classes: dotted values with `append`, `contains`
 * and `intersects`, plus the named constants as static members.
 */
function hierarchicalKind(name, constants) {
	const Kind = ({
		[name]: class {
			constructor(value) { this.value = value; }
			append(part) { return new Kind(this.value ? this.value + '.' + part : part); }
			contains(other) { return this.value === other.value || other.value.startsWith(this.value + '.') || this.value === ''; }
			intersects(other) { return this.contains(other) || other.contains(this); }
		},
	})[name];
	for (const [k, v] of Object.entries(constants)) { Kind[k] = new Kind(v); }
	return Kind;
}

/** Fill-in for classes the editor doesn't model yet: constructible, keeps its args. */
function plainClass(name) {
	return ({ [name]: class { constructor(...args) { this.args = args; } } })[name];
}

// ---------------------------------------------------------------------------
// State shared with the editor
// ---------------------------------------------------------------------------

const state = {
	workspaceFolders: [],
	extensions: [],         // descriptions sent by the editor
	activated: new Map(),   // id -> { exports, context, module }
	commands: new Map(),    // id -> { callback, thisArg }
	configDefaults: {},     // dotted key -> default value
	configUser: {},         // dotted key -> value set at runtime
};

// ---------------------------------------------------------------------------
// vscode.commands
// ---------------------------------------------------------------------------

const commands = {
	registerCommand(id, callback, thisArg) {
		state.commands.set(id, { callback, thisArg, owner: currentExtensionId });
		send({ type: 'registerCommand', id });
		return new Disposable(() => {
			state.commands.delete(id);
			send({ type: 'unregisterCommand', id });
		});
	},
	registerTextEditorCommand(id, callback, thisArg) {
		return commands.registerCommand(id, (...args) => callback.call(thisArg, undefined, undefined, ...args));
	},
	async executeCommand(id, ...args) {
		// Context keys drive `when` clauses (which views show, menus…).
		if (id === 'vscode.diff') { return openDiff(args[0], args[1], args[2]); }
		if (id === 'vscode.open') { await showTextDocument(args[0]); return undefined; }
		if (id === 'setContext') {
			send({ type: 'setContext', key: String(args[0]), value: safeJson(args[1]) });
			return undefined;
		}
		if (state.commands.has(id) || await activateForCommand(id)) {
			const c = state.commands.get(id);
			if (c) {
				// Attribute what the command does synchronously (messages,
				// status bar items) to the extension that registered it.
				const previous = currentExtensionId;
				currentExtensionId = c.owner;
				try { return await c.callback.apply(c.thisArg, args); }
				finally { currentExtensionId = previous; }
			}
		}
		// Not an extension command: hand it to the editor.
		return request({ type: 'executeEditorCommand', id, args: safeJson(args) });
	},
	async getCommands() { return [...state.commands.keys()]; },
};

function safeJson(v) {
	try { return JSON.parse(JSON.stringify(v ?? null)); } catch { return null; }
}

// ---------------------------------------------------------------------------
// vscode.window
// ---------------------------------------------------------------------------

const StatusBarAlignment = { Left: 1, Right: 2 };
let nextStatusItem = 1;

function createStatusBarItem(idOrAlignment, alignmentOrPriority, maybePriority) {
	let id, alignment, priority;
	if (typeof idOrAlignment === 'string') { id = idOrAlignment; alignment = alignmentOrPriority; priority = maybePriority; }
	else { alignment = idOrAlignment; priority = alignmentOrPriority; }
	const owner = currentExtensionId;
	const handle = nextStatusItem++;
	const data = {
		text: '', tooltip: undefined, command: undefined, name: undefined,
		color: undefined, backgroundColor: undefined, visible: false,
	};
	let scheduled = false;
	const update = () => {
		if (scheduled) { return; }
		scheduled = true;
		queueMicrotask(() => {
			scheduled = false;
			const tooltip = data.tooltip instanceof MarkdownString ? data.tooltip.value : data.tooltip;
			const command = typeof data.command === 'string' ? data.command : data.command && data.command.command;
			const commandArgs = typeof data.command === 'object' && data.command ? safeJson(data.command.arguments) : null;
			send({
				type: 'statusBarItem', handle, extension: owner, id: id || null,
				alignment: alignment === StatusBarAlignment.Right ? 'right' : 'left',
				priority: typeof priority === 'number' ? priority : 0,
				text: data.text || '', tooltip: tooltip || null, command: command || null, commandArgs,
				visible: data.visible,
			});
		});
	};
	const item = {
		get id() { return id || `${owner}.${handle}`; },
		get alignment() { return alignment || StatusBarAlignment.Left; },
		get priority() { return priority; },
		show() { data.visible = true; update(); },
		hide() { data.visible = false; update(); },
		dispose() { data.visible = false; update(); },
	};
	for (const key of Object.keys(data)) {
		if (key === 'visible') { continue; }
		Object.defineProperty(item, key, {
			get() { return data[key]; },
			set(v) { data[key] = v; update(); },
			enumerable: true,
		});
	}
	return item;
}

function showMessage(level) {
	return async (message, ...rest) => {
		let options = {};
		if (rest.length && rest[0] && typeof rest[0] === 'object' && !('title' in rest[0])) { options = rest.shift(); }
		const items = rest.filter(i => i !== undefined);
		const titles = items.map(i => typeof i === 'string' ? i : i.title);
		const index = await request({
			type: 'showMessage', level, text: String(message), items: titles,
			modal: !!options.modal, detail: options.detail || null, extension: currentExtensionId,
		});
		return typeof index === 'number' && index >= 0 ? items[index] : undefined;
	};
}

const outputChannels = new Map();
function createOutputChannel(name, options) {
	const ch = {
		name,
		append(v) { send({ type: 'output', channel: name, text: String(v) }); },
		appendLine(v) { send({ type: 'output', channel: name, text: String(v) + '\n' }); },
		replace(v) { send({ type: 'output', channel: name, text: String(v), replace: true }); },
		clear() { send({ type: 'output', channel: name, text: '', replace: true }); },
		show() { send({ type: 'outputShow', channel: name }); }, hide() { }, dispose() { outputChannels.delete(name); send({ type: 'outputDispose', channel: name }); },
	};
	// LogOutputChannel (createOutputChannel(name, { log: true })).
	if (options && (options === true || options.log)) {
		for (const lvl of ['trace', 'debug', 'info', 'warn', 'error']) {
			ch[lvl] = (msg, ...args) => ch.appendLine(`[${lvl}] ${formatArgs([msg, ...args])}`);
		}
		ch.logLevel = 2;
		ch.onDidChangeLogLevel = silentEvent;
	}
	outputChannels.set(name, ch);
	send({ type: 'outputCreate', channel: name, log: !!(options && (options === true || options.log)) });
	return ch;
}

const ProgressLocation = { SourceControl: 1, Window: 10, Notification: 15 };

// ---------------------------------------------------------------------------
// Documents and editors (extHostDocuments.ts, extHostTextEditor.ts)
// ---------------------------------------------------------------------------

/** A `vscode.TextEdit`: replace `range` with `newText`. */
class TextEdit {
	static replace(range, newText) { return new TextEdit(range, newText); }
	static insert(position, newText) { return new TextEdit(new Range(position, position), newText); }
	static delete(range) { return new TextEdit(range, ''); }
	static setEndOfLine() { return new TextEdit(new Range(0, 0, 0, 0), ''); }
	constructor(range, newText) { this.range = range; this.newText = newText; }
}

/** A `vscode.WorkspaceEdit`: text edits per file, plus file operations. */
class WorkspaceEdit {
	constructor() { this._edits = new Map(); this._fileOps = []; }
	_list(uri) {
		const key = uri.fsPath;
		if (!this._edits.has(key)) { this._edits.set(key, { uri, edits: [] }); }
		return this._edits.get(key).edits;
	}
	replace(uri, range, newText) { this._list(uri).push(new TextEdit(range, newText)); }
	insert(uri, position, newText) { this._list(uri).push(TextEdit.insert(position, newText)); }
	delete(uri, range) { this._list(uri).push(TextEdit.delete(range)); }
	set(uri, edits) {
		const list = this._list(uri);
		list.length = 0;
		for (const e of edits || []) { list.push(Array.isArray(e) ? e[0] : e); }
	}
	has(uri) { return this._edits.has(uri.fsPath); }
	get(uri) { return this.has(uri) ? [...this._edits.get(uri.fsPath).edits] : []; }
	entries() { return [...this._edits.values()].map(v => [v.uri, [...v.edits]]); }
	get size() { return this._edits.size + this._fileOps.length; }
	createFile(uri, options) { this._fileOps.push({ kind: 'create', uri, options: options || {} }); }
	deleteFile(uri, options) { this._fileOps.push({ kind: 'delete', uri, options: options || {} }); }
	renameFile(oldUri, newUri, options) { this._fileOps.push({ kind: 'rename', oldUri, newUri, options: options || {} }); }
}

const EndOfLine = { LF: 1, CRLF: 2 };
const DEFAULT_WORD = /(-?\d*\.\d\w*)|([^\`\~\!\@\#\$\%\^\&\*\(\)\-\=\+\[\{\]\}\\\|\;\:\'\"\,\.\<\>\/\?\s]+)/g;

class TextLine {
	constructor(lineNumber, text, isLast) {
		this.lineNumber = lineNumber;
		this.text = text;
		this.range = new Range(lineNumber, 0, lineNumber, text.length);
		this.rangeIncludingLineBreak = isLast ? this.range : new Range(lineNumber, 0, lineNumber + 1, 0);
		this.firstNonWhitespaceCharacterIndex = /^(\s*)/.exec(text)[1].length;
		this.isEmptyOrWhitespace = this.firstNonWhitespaceCharacterIndex === text.length;
	}
}

class TextDocument {
	constructor(fsPath, languageId, version, text, untitled) {
		this.uri = untitled ? new Uri('untitled', '', fsPath) : Uri.file(fsPath);
		this.fileName = fsPath;
		this.isUntitled = !!untitled;
		this.languageId = languageId || 'plaintext';
		this.version = version || 1;
		this.isDirty = false;
		this.isClosed = false;
		this.encoding = 'utf8';
		this._setText(text || '');
	}
	_setText(text) {
		this._text = text;
		this._lines = text.split(/\r\n|\r|\n/);
		this.eol = text.includes('\r\n') ? EndOfLine.CRLF : EndOfLine.LF;
	}
	get lineCount() { return this._lines.length; }
	getText(range) {
		if (!range) { return this._text; }
		const r = this.validateRange(range);
		return this._text.slice(this.offsetAt(r.start), this.offsetAt(r.end));
	}
	lineAt(lineOrPosition) {
		const line = typeof lineOrPosition === 'number' ? lineOrPosition : lineOrPosition.line;
		if (line < 0 || line >= this._lines.length) { throw new Error('Illegal value for `line`'); }
		return new TextLine(line, this._lines[line], line === this._lines.length - 1);
	}
	offsetAt(position) {
		const p = this.validatePosition(position);
		const eolLen = this.eol === EndOfLine.CRLF ? 2 : 1;
		let offset = 0;
		for (let i = 0; i < p.line; i++) { offset += this._lines[i].length + eolLen; }
		return offset + p.character;
	}
	positionAt(offset) {
		offset = Math.max(0, Math.min(Math.floor(offset), this._text.length));
		const eolLen = this.eol === EndOfLine.CRLF ? 2 : 1;
		let line = 0;
		while (line < this._lines.length - 1 && offset > this._lines[line].length) {
			offset -= this._lines[line].length + eolLen;
			line++;
		}
		return new Position(line, Math.min(offset, this._lines[line].length));
	}
	validatePosition(p) {
		let line = Math.max(0, Math.min(p.line, this._lines.length - 1));
		let character = p.line < 0 ? 0 : p.line >= this._lines.length ? this._lines[line].length : p.character;
		character = Math.max(0, Math.min(character, this._lines[line].length));
		return new Position(line, character);
	}
	validateRange(r) { return new Range(this.validatePosition(r.start), this.validatePosition(r.end)); }
	getWordRangeAtPosition(position, regex) {
		const p = this.validatePosition(position);
		const text = this._lines[p.line];
		const re = new RegExp((regex || DEFAULT_WORD).source, 'g');
		let m;
		while ((m = re.exec(text))) {
			if (m.index <= p.character && p.character <= m.index + m[0].length && m[0].length) {
				return new Range(p.line, m.index, p.line, m.index + m[0].length);
			}
			if (!m[0].length) { re.lastIndex++; }
		}
		return undefined;
	}
	async save() { return !!(await request({ type: 'saveDocument', path: this.fileName })); }
}

const docs = new Map();             // fsPath -> TextDocument
const editors = new Map();          // fsPath -> TextEditor
let activeEditor;
const docOpened = new EventEmitter();
const docChanged = new EventEmitter();
const docClosed = new EventEmitter();
const docSaved = new EventEmitter();
const activeEditorChanged = new EventEmitter();
const visibleEditorsChanged = new EventEmitter();
const selectionChanged = new EventEmitter();
const visibleRangesChanged = new EventEmitter();

function toWire(range) {
	const s = range.start || range, e = range.end || range;
	return [s.line, s.character, e.line, e.character];
}
function fromWireSelection(a) { return new Selection(a[0], a[1], a[2], a[3]); }

/** Strip snippet syntax to its default text: `${1:name}` → `name`, `$0` → ``. */
function snippetText(value) {
	let prev;
	do {
		prev = value;
		value = value.replace(/\$\{\d+:([^{}$]*)\}/g, '$1').replace(/\$\{\d+\|([^,|]*)[^}]*\|\}/g, '$1');
	} while (value !== prev);
	return value.replace(/\$\{\d+\}|\$\d+/g, '').replace(/\\([$}\\])/g, '$1');
}

async function applyEditsTo(fsPath, edits) {
	const list = edits.map(e => ({ range: toWire(e.range), text: e.newText }));
	if (!list.length) { return true; }
	return !!(await request({ type: 'applyEdits', path: fsPath, edits: list }));
}

let nextDecorationType = 1;
function createTextEditorDecorationType(options) {
	const key = `TextEditorDecorationType${nextDecorationType++}`;
	// The editor is dark: `dark` overrides win over the base options.
	const merged = { ...(options || {}), ...((options && options.dark) || {}) };
	const plain = {};
	for (const [k, v] of Object.entries(merged)) {
		if (k === 'light' || k === 'dark') { continue; }
		plain[k] = v instanceof ThemeColor ? { themeColor: v.id } : (v && typeof v === 'object' && !Array.isArray(v) ? safeJson(v) : v);
	}
	send({ type: 'decorationType', key, options: plain });
	return { key, dispose() { send({ type: 'decorationTypeDisposed', key }); } };
}

class TextEditor {
	constructor(document) {
		this.document = document;
		this.selections = [new Selection(0, 0, 0, 0)];
		this.visibleRanges = [new Range(0, 0, Math.max(0, document.lineCount - 1), 0)];
		this.options = { tabSize: 2, indentSize: 2, insertSpaces: true, cursorStyle: 1, lineNumbers: 1 };
		this.viewColumn = 1;
	}
	get selection() { return this.selections[0]; }
	set selection(s) { this.selections = [s]; send({ type: 'setSelection', path: this.document.fileName, selection: [s.anchor ? s.anchor.line : s.start.line, s.anchor ? s.anchor.character : s.start.character, s.active ? s.active.line : s.end.line, s.active ? s.active.character : s.end.character] }); }
	setDecorations(decorationType, rangesOrOptions) {
		const ranges = (rangesOrOptions || []).map(r => {
			const range = r.range || r;
			const hover = r.hoverMessage instanceof MarkdownString ? r.hoverMessage.value : (typeof r.hoverMessage === 'string' ? r.hoverMessage : null);
			const after = r.renderOptions && r.renderOptions.after && r.renderOptions.after.contentText;
			const before = r.renderOptions && r.renderOptions.before && r.renderOptions.before.contentText;
			return { range: toWire(range), hover, after: after || null, before: before || null };
		});
		send({ type: 'setDecorations', path: this.document.fileName, key: decorationType.key, ranges });
	}
	async edit(callback) {
		const edits = [];
		const builder = {
			replace(location, value) { const r = location.start ? location : new Range(location, location); edits.push(new TextEdit(r, value)); },
			insert(position, value) { edits.push(TextEdit.insert(position, value)); },
			delete(location) { edits.push(TextEdit.delete(location)); },
			setEndOfLine() { },
		};
		callback(builder);
		return applyEditsTo(this.document.fileName, edits);
	}
	async insertSnippet(snippet, location) {
		const text = snippetText(typeof snippet === 'string' ? snippet : snippet.value);
		const targets = location ? (Array.isArray(location) ? location : [location]) : this.selections;
		const edits = targets.map(t => new TextEdit(t.start ? new Range(t.start, t.end) : new Range(t, t), text));
		return applyEditsTo(this.document.fileName, edits);
	}
	revealRange(range) { send({ type: 'reveal', path: this.document.fileName, line: (range.start || range).line }); }
	show() { } hide() { }
}

function editorFor(doc) {
	if (!editors.has(doc.fileName)) { editors.set(doc.fileName, new TextEditor(doc)); }
	return editors.get(doc.fileName);
}

/** `workspace.registerTextDocumentContentProvider`: scheme -> provider. */
const contentProviders = new Map();

/** The text behind `uri`: an open document, a content provider, or disk. */
async function contentOf(uri) {
	if (uri.scheme !== 'file' && uri.scheme !== 'untitled') {
		const provider = contentProviders.get(uri.scheme);
		if (!provider) { return ''; }
		return String((await provider.provideTextDocumentContent(uri, new CancellationTokenSource().token)) || '');
	}
	const open = docs.get(uri.fsPath);
	if (open) { return open.getText(); }
	try { return await fs.promises.readFile(uri.fsPath, 'utf8'); } catch { return ''; }
}

/** `vscode.diff`: show two documents side by side (here: an inline diff tab). */
async function openDiff(left, right, title) {
	const [base, text] = await Promise.all([contentOf(left), contentOf(right)]);
	const target = right.scheme === 'file' ? right.fsPath : (left.scheme === 'file' ? left.fsPath : right.path);
	await request({ type: 'openDiff', path: target, base, text, title: title || path.basename(target) });
}

async function openTextDocument(arg) {
	if (arg && typeof arg === 'object' && !(arg instanceof Uri) && !arg.scheme) {
		const d = new TextDocument(`Untitled-${docs.size + 1}`, arg.language || 'plaintext', 1, arg.content || '', true);
		return d;
	}
	if (arg instanceof Uri && arg.scheme !== 'file') {
		const d = new TextDocument(arg.path, languageIdFor(arg.path), 1, await contentOf(arg), false);
		d.uri = arg;
		return d;
	}
	const fsPath = typeof arg === 'string' ? path.resolve(arg) : arg.fsPath;
	if (docs.has(fsPath)) { return docs.get(fsPath); }
	const text = await fs.promises.readFile(fsPath, 'utf8');
	return new TextDocument(fsPath, languageIdFor(fsPath), 1, text);
}

function languageIdFor(fsPath) {
	const ext = path.extname(fsPath).slice(1).toLowerCase();
	const map = { ts: 'typescript', tsx: 'typescriptreact', js: 'javascript', jsx: 'javascriptreact', mjs: 'javascript', cjs: 'javascript', json: 'json', md: 'markdown', py: 'python', rs: 'rust', go: 'go', html: 'html', css: 'css', scss: 'scss', less: 'less', vue: 'vue', yaml: 'yaml', yml: 'yaml', toml: 'toml', sh: 'shellscript', c: 'c', h: 'c', cpp: 'cpp', java: 'java', rb: 'ruby', php: 'php', xml: 'xml', sql: 'sql' };
	return map[ext] || 'plaintext';
}

async function showTextDocument(arg, options) {
	const doc = arg instanceof TextDocument ? arg : await openTextDocument(arg);
	await request({ type: 'openEditor', path: doc.fileName, preserveFocus: !!(options && options.preserveFocus) });
	const open = docs.get(doc.fileName) || doc;
	return editorFor(open);
}

async function applyWorkspaceEdit(edit) {
	let ok = true;
	for (const op of edit._fileOps || []) {
		try {
			if (op.kind === 'create') {
				if (!(op.options.ignoreIfExists && fs.existsSync(op.uri.fsPath))) {
					await fs.promises.mkdir(path.dirname(op.uri.fsPath), { recursive: true });
					await fs.promises.writeFile(op.uri.fsPath, op.options.contents || '', { flag: op.options.overwrite ? 'w' : 'wx' });
				}
			} else if (op.kind === 'delete') {
				await fs.promises.rm(op.uri.fsPath, { recursive: !!op.options.recursive, force: !!op.options.ignoreIfNotExists });
			} else if (op.kind === 'rename') {
				await fs.promises.rename(op.oldUri.fsPath, op.newUri.fsPath);
			}
		} catch (err) { console.error(err); ok = false; }
	}
	for (const [uri, edits] of edit.entries()) {
		ok = (await applyEditsTo(uri.fsPath, edits)) && ok;
	}
	return ok;
}

/** Messages from the editor about its documents and editors. */
function handleDocumentMessage(msg) {
	switch (msg.type) {
		case 'documentOpened': {
			const doc = new TextDocument(msg.path, msg.languageId, msg.version, msg.text);
			docs.set(msg.path, doc);
			docOpened.fire(doc);
			return true;
		}
		case 'documentChanged': {
			const doc = docs.get(msg.path);
			if (!doc) { return true; }
			const oldRange = new Range(new Position(0, 0), doc.positionAt(doc._text.length));
			const oldLength = doc._text.length;
			doc._setText(msg.text);
			doc.version = msg.version;
			doc.isDirty = !!msg.dirty;
			docChanged.fire({
				document: doc,
				contentChanges: [{ range: oldRange, rangeOffset: 0, rangeLength: oldLength, text: msg.text }],
				reason: undefined,
			});
			return true;
		}
		case 'documentSaved': {
			const doc = docs.get(msg.path);
			if (doc) { doc.isDirty = false; docSaved.fire(doc); }
			return true;
		}
		case 'documentClosed': {
			const doc = docs.get(msg.path);
			if (doc) {
				docs.delete(msg.path);
				editors.delete(msg.path);
				doc.isClosed = true;
				docClosed.fire(doc);
			}
			return true;
		}
		case 'activeEditor': {
			const doc = msg.path && docs.get(msg.path);
			const editor = doc ? editorFor(doc) : undefined;
			if (editor && msg.selections) { editor.selections = msg.selections.map(fromWireSelection); }
			if (editor && msg.visibleRange) { editor.visibleRanges = [new Range(msg.visibleRange[0], 0, msg.visibleRange[1], 0)]; }
			if (editor !== activeEditor) {
				activeEditor = editor;
				activeEditorChanged.fire(editor);
				visibleEditorsChanged.fire(editor ? [editor] : []);
			}
			return true;
		}
		case 'selectionChanged': {
			const doc = docs.get(msg.path);
			if (doc) {
				const editor = editorFor(doc);
				editor.selections = msg.selections.map(fromWireSelection);
				selectionChanged.fire({ textEditor: editor, selections: editor.selections, kind: 1 });
			}
			return true;
		}
	}
	return false;
}

// ---------------------------------------------------------------------------
// Tabs (extHostEditorTabs.ts): one group mirroring the editor's tabs.
// ---------------------------------------------------------------------------

class TabInputText { constructor(uri) { this.uri = uri; } }
class TabInputTextDiff { constructor(original, modified) { this.original = original; this.modified = modified; } }
class TabInputWebview { constructor(viewType) { this.viewType = viewType; } }
class TabInputCustom { constructor(uri, viewType) { this.uri = uri; this.viewType = viewType; } }
class TabInputNotebook { constructor(uri, notebookType) { this.uri = uri; this.notebookType = notebookType; } }
class TabInputTerminal { constructor() { } }

const tabsChanged = new EventEmitter();
const tabGroupsChanged = new EventEmitter();
const tabGroup = { tabs: [], activeTab: undefined, isActive: true, viewColumn: 1 };
const tabGroups = {
	get all() { return [tabGroup]; },
	get activeTabGroup() { return tabGroup; },
	onDidChangeTabGroups: tabGroupsChanged.event,
	onDidChangeTabs: tabsChanged.event,
	async close(tabOrTabs) {
		const list = (Array.isArray(tabOrTabs) ? tabOrTabs : [tabOrTabs]).filter(t => t && t._path);
		if (!list.length) { return false; }
		return !!(await request({ type: 'closeTabs', paths: list.map(t => t._path) }));
	},
};

function setTabs(wire) {
	const before = tabGroup.tabs;
	tabGroup.tabs = wire.map(t => {
		let input;
		if (t.kind === 'webview') { input = new TabInputWebview(t.viewType || ''); }
		else if (t.kind === 'diff') { input = new TabInputTextDiff(Uri.file(t.path), Uri.file(t.path)); }
		else { input = new TabInputText(Uri.file(t.path)); }
		return { label: t.label, input, isActive: !!t.active, isDirty: !!t.dirty, isPinned: !!t.pinned, isPreview: false, group: tabGroup, _path: t.path };
	});
	tabGroup.activeTab = tabGroup.tabs.find(t => t.isActive);
	const opened = tabGroup.tabs.filter(t => !before.some(b => b._path === t._path));
	const closed = before.filter(b => !tabGroup.tabs.some(t => t._path === b._path));
	const changed = tabGroup.tabs.filter(t => before.some(b => b._path === t._path && (b.isActive !== t.isActive || b.isDirty !== t.isDirty)));
	tabsChanged.fire({ opened, closed, changed });
	tabGroupsChanged.fire({ opened: [], closed: [], changed: [tabGroup] });
}

// ---------------------------------------------------------------------------
// Webviews (extHostWebview.ts, extHostWebviewPanels.ts, extHostWebviewView.ts)
// ---------------------------------------------------------------------------

let nextWebview = 1;
const webviewsByHandle = new Map(); // handle -> { webview, panel?, view? }
const viewProviders = new Map();     // viewId -> { provider, options, owner }
const panelSerializers = new Map();

/** The scheme the editor serves webview resources under. */
const WEBVIEW_SCHEME = 'vscode-resource';

class Webview {
	constructor(handle, options, extensionPath) {
		this._handle = handle;
		this._html = '';
		this._options = options || {};
		this._messages = new EventEmitter();
		this.onDidReceiveMessage = this._messages.event;
		this._extensionPath = extensionPath;
	}
	get html() { return this._html; }
	set html(value) {
		this._html = String(value);
		send({ type: 'webviewHtml', handle: this._handle, html: this._html });
	}
	get options() { return this._options; }
	set options(value) {
		this._options = value || {};
		const roots = (this._options.localResourceRoots || []).map(u => u.fsPath);
		send({ type: 'webviewOptions', handle: this._handle, enableScripts: !!this._options.enableScripts, localResourceRoots: roots });
	}
	get cspSource() { return `${WEBVIEW_SCHEME}:`; }
	asWebviewUri(uri) { return new Uri(WEBVIEW_SCHEME, `w${this._handle}`, uri.fsPath || uri.path, uri.query, uri.fragment); }
	async postMessage(message) {
		send({ type: 'webviewPostMessage', handle: this._handle, message: safeJson(message) });
		return true;
	}
}

function newWebview(options) {
	const handle = nextWebview++;
	const desc = state.extensions.find(e => e.id === currentExtensionId);
	const webview = new Webview(handle, options, desc && desc.path);
	const roots = ((options && options.localResourceRoots) || []).map(u => u.fsPath);
	return { handle, webview, roots, extensionPath: desc ? desc.path : null };
}

function createWebviewPanel(viewType, title, showOptions, options) {
	const { handle, webview, roots, extensionPath } = newWebview(options);
	const disposed = new EventEmitter();
	const viewState = new EventEmitter();
	let currentTitle = title;
	let isDisposed = false;
	const panel = {
		viewType,
		webview,
		get title() { return currentTitle; },
		set title(t) { currentTitle = t; send({ type: 'webviewTitle', handle, title: String(t) }); },
		iconPath: undefined,
		options: options || {},
		viewColumn: 1,
		active: true,
		visible: true,
		onDidDispose: disposed.event,
		onDidChangeViewState: viewState.event,
		reveal() { send({ type: 'webviewReveal', handle }); },
		dispose() {
			if (isDisposed) { return; }
			isDisposed = true;
			send({ type: 'webviewDisposed', handle });
			webviewsByHandle.delete(handle);
			disposed.fire();
		},
		_fireDisposed() { if (!isDisposed) { isDisposed = true; webviewsByHandle.delete(handle); disposed.fire(); } },
		_viewState: viewState,
	};
	webviewsByHandle.set(handle, { webview, panel });
	send({
		type: 'webviewCreated', handle, kind: 'panel', viewType, title: String(title),
		extensionPath, localResourceRoots: roots, enableScripts: !!(options && options.enableScripts),
		preserveFocus: !!(showOptions && showOptions.preserveFocus),
	});
	return panel;
}

function registerWebviewViewProvider(viewId, provider, options) {
	viewProviders.set(viewId, { provider, options: options || {}, owner: currentExtensionId });
	send({ type: 'webviewViewProvider', viewId });
	return new Disposable(() => viewProviders.delete(viewId));
}

/** The editor shows a contributed webview view: let its provider fill it. */
async function resolveWebviewView(viewId, title) {
	if (!viewProviders.has(viewId)) { await activateByEvent(`onView:${viewId}`); }
	const entry = viewProviders.get(viewId);
	if (!entry) { send({ type: 'log', level: 'warn', text: `No provider for webview view ${viewId}` }); return; }
	const previous = currentExtensionId;
	currentExtensionId = entry.owner;
	const { handle, webview, roots, extensionPath } = newWebview(entry.options.webviewOptions || {});
	currentExtensionId = previous;
	const disposed = new EventEmitter();
	const visibility = new EventEmitter();
	let currentTitle = title;
	let description;
	const view = {
		viewType: viewId,
		webview,
		get title() { return currentTitle; },
		set title(t) { currentTitle = t; send({ type: 'webviewTitle', handle, title: String(t || '') }); },
		get description() { return description; },
		set description(d) { description = d; },
		badge: undefined,
		visible: true,
		onDidDispose: disposed.event,
		onDidChangeVisibility: visibility.event,
		show() { send({ type: 'webviewReveal', handle }); },
		_fireDisposed() { disposed.fire(); },
		_visibility: visibility,
	};
	webviewsByHandle.set(handle, { webview, view });
	send({ type: 'webviewCreated', handle, kind: 'view', viewId, title: String(title || ''), extensionPath, localResourceRoots: roots, enableScripts: true });
	try {
		const prev = currentExtensionId;
		currentExtensionId = entry.owner;
		try { await entry.provider.resolveWebviewView(view, { state: undefined }, new CancellationTokenSource().token); }
		finally { currentExtensionId = prev; }
	} catch (err) { console.error(err); }
}

/** Messages from the editor about webviews. */
function handleWebviewMessage(msg) {
	switch (msg.type) {
		case 'tabs':
			setTabs(msg.tabs || []);
			return true;
		case 'resolveWebviewView':
			resolveWebviewView(msg.viewId, msg.title).catch(err => console.error(err));
			return true;
		case 'webviewMessage': {
			const entry = webviewsByHandle.get(msg.handle);
			if (entry) { entry.webview._messages.fire(msg.message); }
			return true;
		}
		case 'webviewClosed': {
			const entry = webviewsByHandle.get(msg.handle);
			if (entry) {
				webviewsByHandle.delete(msg.handle);
				if (entry.panel) { entry.panel._fireDisposed(); }
				if (entry.view) { entry.view._fireDisposed(); }
			}
			return true;
		}
		case 'webviewVisible': {
			const entry = webviewsByHandle.get(msg.handle);
			if (entry && entry.view) { entry.view.visible = !!msg.visible; entry.view._visibility.fire(); }
			if (entry && entry.panel) {
				entry.panel.visible = !!msg.visible;
				entry.panel.active = !!msg.visible;
				entry.panel._viewState.fire({ webviewPanel: entry.panel });
			}
			return true;
		}
	}
	return false;
}

const window = {
	showInformationMessage: showMessage('info'),
	showWarningMessage: showMessage('warning'),
	showErrorMessage: showMessage('error'),
	createStatusBarItem,
	setStatusBarMessage(text) {
		send({ type: 'statusMessage', text: String(text) });
		return new Disposable(() => send({ type: 'statusMessage', text: '' }));
	},
	createOutputChannel,
	async showQuickPick(items, options) {
		const list = (await items) || [];
		const wire = list.map(i => typeof i === 'string'
			? { label: i, description: null, detail: null, separator: false }
			: { label: i.label, description: i.description || null, detail: i.detail || null, separator: i.kind === -1 });
		const picked = await request({
			type: 'showQuickPick', items: wire,
			placeholder: options && (options.placeHolder || options.title) || null,
			canPickMany: !!(options && options.canPickMany),
		});
		if (options && options.canPickMany) {
			return Array.isArray(picked) ? picked.map(i => list[i]) : undefined;
		}
		return typeof picked === 'number' && picked >= 0 ? list[picked] : undefined;
	},
	async showInputBox(options) {
		const value = await request({
			type: 'showInputBox', prompt: options && options.prompt || null,
			placeholder: options && options.placeHolder || null, value: options && options.value || '',
			password: !!(options && options.password),
		});
		return typeof value === 'string' ? value : undefined;
	},
	async withProgress(options, task) {
		const title = options && options.title;
		if (title) { send({ type: 'statusMessage', text: String(title) }); }
		try {
			return await task({ report() { } }, new CancellationTokenSource().token);
		} finally {
			if (title) { send({ type: 'statusMessage', text: '' }); }
		}
	},
	get activeTextEditor() { return activeEditor; },
	get visibleTextEditors() { return activeEditor ? [activeEditor] : []; },
	showTextDocument,
	tabGroups,
	terminals: [],
	state: { focused: true, active: true },
	onDidChangeActiveTextEditor: activeEditorChanged.event,
	onDidChangeVisibleTextEditors: visibleEditorsChanged.event,
	onDidChangeTextEditorSelection: selectionChanged.event,
	onDidChangeTextEditorVisibleRanges: visibleRangesChanged.event,
	onDidChangeWindowState: silentEvent,
	onDidOpenTerminal: silentEvent,
	onDidCloseTerminal: silentEvent,
	createTextEditorDecorationType,
	registerTreeDataProvider() { return new Disposable(() => { }); },
	createTreeView() {
		return { onDidChangeSelection: silentEvent, onDidChangeVisibility: silentEvent, onDidExpandElement: silentEvent, onDidCollapseElement: silentEvent, reveal: async () => { }, dispose() { } };
	},
	createWebviewPanel,
	registerWebviewViewProvider,
	registerWebviewPanelSerializer(viewType, serializer) {
		panelSerializers.set(viewType, serializer);
		return new Disposable(() => panelSerializers.delete(viewType));
	},
	registerUriHandler() { return new Disposable(() => { }); },
};

// ---------------------------------------------------------------------------
// vscode.workspace
// ---------------------------------------------------------------------------

const configurationEmitter = new EventEmitter();

function getConfiguration(section) {
	const prefix = section ? section + '.' : '';
	const lookup = key => {
		const full = prefix + key;
		if (full in state.configUser) { return state.configUser[full]; }
		if (full in state.configDefaults) { return state.configDefaults[full]; }
		// Nested object: collect `full.*` keys.
		const nested = {};
		let found = false;
		for (const source of [state.configDefaults, state.configUser]) {
			for (const [k, v] of Object.entries(source)) {
				if (k.startsWith(full + '.')) { setPath(nested, k.slice(full.length + 1), v); found = true; }
			}
		}
		return found ? nested : undefined;
	};
	return {
		get(key, defaultValue) { const v = lookup(key); return v === undefined ? defaultValue : v; },
		has(key) { return lookup(key) !== undefined; },
		inspect(key) { const k = prefix + key; return { key: k, defaultValue: state.configDefaults[k], globalValue: state.configUser[k] }; },
		async update(key, value) {
			state.configUser[prefix + key] = value;
			configurationEmitter.fire({ affectsConfiguration: s => (prefix + key).startsWith(s) });
		},
	};
}

function setPath(obj, dotted, value) {
	const parts = dotted.split('.');
	let o = obj;
	for (const p of parts.slice(0, -1)) { o = o[p] = o[p] || {}; }
	o[parts[parts.length - 1]] = value;
}

const FileType = { Unknown: 0, File: 1, Directory: 2, SymbolicLink: 64 };
const workspaceFs = {
	async readFile(uri) { return fs.promises.readFile(uri.fsPath); },
	async writeFile(uri, content) { await fs.promises.mkdir(path.dirname(uri.fsPath), { recursive: true }); return fs.promises.writeFile(uri.fsPath, content); },
	async stat(uri) {
		const s = await fs.promises.stat(uri.fsPath);
		return { type: s.isDirectory() ? FileType.Directory : FileType.File, ctime: s.ctimeMs, mtime: s.mtimeMs, size: s.size };
	},
	async readDirectory(uri) {
		const entries = await fs.promises.readdir(uri.fsPath, { withFileTypes: true });
		return entries.map(e => [e.name, e.isDirectory() ? FileType.Directory : FileType.File]);
	},
	async createDirectory(uri) { await fs.promises.mkdir(uri.fsPath, { recursive: true }); },
	async delete(uri, options) { await fs.promises.rm(uri.fsPath, { recursive: !!(options && options.recursive), force: true }); },
	async rename(source, target) { await fs.promises.rename(source.fsPath, target.fsPath); },
	async copy(source, target) { await fs.promises.cp(source.fsPath, target.fsPath, { recursive: true }); },
};

const workspace = {
	get workspaceFolders() { return state.workspaceFolders.length ? state.workspaceFolders : undefined; },
	get rootPath() { return state.workspaceFolders[0] && state.workspaceFolders[0].uri.fsPath; },
	get name() { return state.workspaceFolders[0] && state.workspaceFolders[0].name; },
	getWorkspaceFolder(uri) { return state.workspaceFolders.find(f => uri.fsPath.startsWith(f.uri.fsPath)); },
	asRelativePath(p) {
		const fsPath = typeof p === 'string' ? p : p.fsPath;
		const root = workspace.rootPath;
		return root && fsPath.startsWith(root) ? path.relative(root, fsPath) : fsPath;
	},
	getConfiguration,
	onDidChangeConfiguration: configurationEmitter.event,
	onDidOpenTextDocument: docOpened.event,
	onDidCloseTextDocument: docClosed.event,
	onDidChangeTextDocument: docChanged.event,
	onDidSaveTextDocument: docSaved.event,
	onWillSaveTextDocument: silentEvent,
	onDidChangeWorkspaceFolders: silentEvent,
	onDidCreateFiles: silentEvent,
	onDidDeleteFiles: silentEvent,
	onDidRenameFiles: silentEvent,
	get textDocuments() { return [...docs.values()]; },
	openTextDocument,
	applyEdit: applyWorkspaceEdit,
	fs: workspaceFs,
	isTrusted: true,
	onDidGrantWorkspaceTrust: silentEvent,
	async findFiles() { return []; },
	createFileSystemWatcher() {
		return { onDidCreate: silentEvent, onDidChange: silentEvent, onDidDelete: silentEvent, dispose() { } };
	},
	registerTextDocumentContentProvider(scheme, provider) {
		contentProviders.set(scheme, provider);
		return new Disposable(() => contentProviders.delete(scheme));
	},
	registerFileSystemProvider() { return new Disposable(() => { }); },
};

// ---------------------------------------------------------------------------
// vscode.languages (providers are accepted but not consulted yet)
// ---------------------------------------------------------------------------

// Language features (extHostLanguageFeatures.ts, extHostDiagnostics.ts).

class Diagnostic {
	constructor(range, message, severity = 0) {
		this.range = range; this.message = message; this.severity = severity;
		this.source = undefined; this.code = undefined; this.relatedInformation = undefined; this.tags = undefined;
	}
}
class DiagnosticRelatedInformation { constructor(location, message) { this.location = location; this.message = message; } }
class CompletionItem { constructor(label, kind) { this.label = label; this.kind = kind; } }
class CompletionList { constructor(items = [], isIncomplete = false) { this.items = items; this.isIncomplete = isIncomplete; } }
class Hover { constructor(contents, range) { this.contents = Array.isArray(contents) ? contents : [contents]; this.range = range; } }
class Location {
	constructor(uri, rangeOrPosition) {
		this.uri = uri;
		this.range = rangeOrPosition && rangeOrPosition.start ? rangeOrPosition : new Range(rangeOrPosition, rangeOrPosition);
	}
}

/** All diagnostics, per collection: owner -> fsPath -> Diagnostic[]. */
const allDiagnostics = new Map();
const diagnosticsChanged = new EventEmitter();

function sendDiagnostics(owner, uri, diags) {
	const fsPath = uri.fsPath || Uri.parse(String(uri)).fsPath;
	send({
		type: 'diagnostics', owner, path: fsPath,
		items: (diags || []).map(d => ({
			range: toWire(d.range),
			message: String(d.message),
			severity: typeof d.severity === 'number' ? d.severity : 0,
			source: d.source || null,
			code: d.code === undefined ? null : String(typeof d.code === 'object' && d.code ? d.code.value : d.code),
		})),
	});
	diagnosticsChanged.fire({ uris: [uri] });
}

let nextCollection = 1;
function diagnosticCollection(name) {
	const owner = `${name || 'diagnostics'}#${nextCollection++}`;
	const map = new Map(); // fsPath -> { uri, diags }
	allDiagnostics.set(owner, map);
	const put = (uri, diags) => {
		const key = uri.fsPath;
		if (diags && diags.length) { map.set(key, { uri, diags: [...diags] }); } else { map.delete(key); }
		sendDiagnostics(owner, uri, diags || []);
	};
	return {
		name,
		set(uri, diags) {
			if (Array.isArray(uri)) {
				const merged = new Map();
				for (const [u, d] of uri) {
					if (!merged.has(u.fsPath)) { merged.set(u.fsPath, { uri: u, diags: [] }); }
					if (d) { merged.get(u.fsPath).diags.push(...d); } else { merged.get(u.fsPath).diags = []; }
				}
				for (const { uri: u, diags } of merged.values()) { put(u, diags); }
			} else { put(uri, diags); }
		},
		delete(uri) { put(uri, []); },
		clear() { for (const { uri } of [...map.values()]) { put(uri, []); } },
		get(uri) { const e = map.get(uri.fsPath); return e ? [...e.diags] : []; },
		has(uri) { return map.has(uri.fsPath); },
		forEach(cb) { for (const { uri, diags } of map.values()) { cb(uri, diags, this); } },
		dispose() { this.clear(); allDiagnostics.delete(owner); },
		[Symbol.iterator]: function* () { for (const { uri, diags } of map.values()) { yield [uri, diags]; } },
	};
}

function getDiagnostics(uri) {
	if (uri) {
		const out = [];
		for (const map of allDiagnostics.values()) { const e = map.get(uri.fsPath); if (e) { out.push(...e.diags); } }
		return out;
	}
	const byPath = new Map();
	for (const map of allDiagnostics.values()) {
		for (const { uri: u, diags } of map.values()) {
			if (!byPath.has(u.fsPath)) { byPath.set(u.fsPath, [u, []]); }
			byPath.get(u.fsPath)[1].push(...diags);
		}
	}
	return [...byPath.values()];
}

/** `vscode.languages.match`: a selector's score for a document (0 = no). */
function globToRegExp(glob) {
	let re = '';
	for (let i = 0; i < glob.length; i++) {
		const c = glob[i];
		if (c === '*') {
			if (glob[i + 1] === '*') { re += '.*'; i++; if (glob[i + 1] === '/') { i++; re += '(?:/)?'; } } else { re += '[^/]*'; }
		} else if (c === '?') { re += '[^/]'; }
		else if (c === '{') { const end = glob.indexOf('}', i); re += '(?:' + glob.slice(i + 1, end).split(',').map(x => x.replace(/[.+^$()|[\]\\]/g, '\\$&')).join('|') + ')'; i = end; }
		else { re += c.replace(/[.+^$()|[\]\\]/g, '\\$&'); }
	}
	return new RegExp('^' + re + '$');
}
function match(selector, document) {
	if (Array.isArray(selector)) { return Math.max(0, ...selector.map(s => match(s, document))); }
	if (typeof selector === 'string') {
		return selector === '*' ? 5 : selector === document.languageId ? 10 : 0;
	}
	if (!selector) { return 0; }
	let score = 0;
	if (selector.language) {
		if (selector.language === document.languageId) { score = 10; } else if (selector.language === '*') { score = 5; } else { return 0; }
	}
	if (selector.scheme) {
		if (selector.scheme === document.uri.scheme) { score = Math.max(score, 10); } else if (selector.scheme === '*') { score = Math.max(score, 5); } else { return 0; }
	}
	if (selector.pattern) {
		const pattern = typeof selector.pattern === 'string' ? selector.pattern : selector.pattern.pattern;
		const base = typeof selector.pattern === 'object' && selector.pattern.baseUri ? selector.pattern.baseUri.fsPath : null;
		const target = base ? path.relative(base, document.fileName) : document.fileName;
		const re = globToRegExp(pattern);
		if (re.test(target) || re.test(document.fileName) || (!pattern.includes('/') && re.test(path.basename(document.fileName)))) { score = Math.max(score, 10); } else { return 0; }
	}
	return score;
}

/** Registered providers: kind -> [{ selector, provider, extra, owner }]. */
const providers = new Map();
function registerProvider(kind) {
	return (selector, provider, ...extra) => {
		const entry = { selector, provider, extra, owner: currentExtensionId };
		if (!providers.has(kind)) { providers.set(kind, []); }
		providers.get(kind).push(entry);
		send({ type: 'providerRegistered', kind, triggerCharacters: kind === 'completion' ? extra.flat().filter(c => typeof c === 'string') : [] });
		return new Disposable(() => {
			const list = providers.get(kind);
			const i = list.indexOf(entry);
			if (i >= 0) { list.splice(i, 1); }
		});
	};
}

function providersFor(kind, document) {
	return (providers.get(kind) || [])
		.map(p => ({ ...p, score: match(p.selector, document) }))
		.filter(p => p.score > 0)
		.sort((a, b) => b.score - a.score);
}

function markdownText(c) {
	if (c === undefined || c === null) { return ''; }
	if (typeof c === 'string') { return c; }
	if (c instanceof MarkdownString || 'value' in c && !('language' in c)) { return String(c.value); }
	if ('language' in c) { return '```' + c.language + '\n' + c.value + '\n```'; }
	return String(c);
}

const completionKindNames = ['Text', 'Method', 'Function', 'Constructor', 'Field', 'Variable', 'Class', 'Interface', 'Module', 'Property', 'Unit', 'Value', 'Enum', 'Keyword', 'Snippet', 'Color', 'File', 'Reference', 'Folder', 'EnumMember', 'Constant', 'Struct', 'Event', 'Operator', 'TypeParameter', 'User', 'Issue'];

function wireCompletion(item, document, position) {
	const label = typeof item.label === 'string' ? item.label : (item.label && item.label.label) || '';
	const labelDetail = typeof item.label === 'object' && item.label ? (item.label.detail || '') + (item.label.description ? ' ' + item.label.description : '') : '';
	let insert = item.insertText === undefined ? label : item.insertText;
	insert = typeof insert === 'string' ? insert : snippetText(insert.value || '');
	let range = item.range;
	if (range && range.inserting) { range = range.replacing || range.inserting; }
	if (!range) { range = document.getWordRangeAtPosition(position) || new Range(position, position); if (range.end.isBefore(position)) { range = new Range(range.start, position); } }
	return {
		label,
		kind: typeof item.kind === 'number' ? completionKindNames[item.kind] || 'Text' : 'Text',
		detail: item.detail || labelDetail || null,
		documentation: item.documentation ? markdownText(item.documentation) : null,
		insertText: insert,
		range: toWire(range),
		sortText: item.sortText || label,
		filterText: item.filterText || label,
		preselect: !!item.preselect,
	};
}

async function provideCompletion(msg) {
	const document = docs.get(msg.path);
	if (!document) { return { items: [], isIncomplete: false }; }
	const position = new Position(msg.position[0], msg.position[1]);
	const token = new CancellationTokenSource().token;
	const context = { triggerKind: msg.trigger ? 1 : 0, triggerCharacter: msg.trigger || undefined };
	const items = [];
	let isIncomplete = false;
	for (const p of providersFor('completion', document)) {
		if (msg.trigger && !(p.extra.flat().includes(msg.trigger))) { continue; }
		try {
			const r = await p.provider.provideCompletionItems(document, position, token, context);
			if (!r) { continue; }
			const list = Array.isArray(r) ? r : r.items || [];
			if (!Array.isArray(r) && r.isIncomplete) { isIncomplete = true; }
			for (const item of list) { items.push(wireCompletion(item, document, position)); }
		} catch (err) { console.error(err); }
	}
	return { items, isIncomplete };
}

async function provideHover(msg) {
	const document = docs.get(msg.path);
	if (!document) { return null; }
	const position = new Position(msg.position[0], msg.position[1]);
	const token = new CancellationTokenSource().token;
	const contents = [];
	let range = null;
	for (const p of providersFor('hover', document)) {
		try {
			const h = await p.provider.provideHover(document, position, token);
			if (!h) { continue; }
			for (const c of (Array.isArray(h.contents) ? h.contents : [h.contents])) {
				const t = markdownText(c);
				if (t.trim()) { contents.push(t); }
			}
			if (h.range && !range) { range = toWire(h.range); }
		} catch (err) { console.error(err); }
	}
	return contents.length ? { contents, range } : null;
}

async function provideLocations(kind, method, msg) {
	const document = docs.get(msg.path);
	if (!document) { return []; }
	const position = new Position(msg.position[0], msg.position[1]);
	const token = new CancellationTokenSource().token;
	const out = [];
	for (const p of providersFor(kind, document)) {
		try {
			let r = await p.provider[method](document, position, token);
			if (!r) { continue; }
			if (!Array.isArray(r)) { r = [r]; }
			for (const l of r) {
				const uri = l.targetUri || l.uri;
				const range = l.targetSelectionRange || l.targetRange || l.range;
				if (uri && range) { out.push({ path: uri.fsPath, range: toWire(range) }); }
			}
		} catch (err) { console.error(err); }
	}
	return out;
}

const languages = new Proxy({
	createLanguageStatusItem(id, selector) {
		return { id, selector, name: undefined, text: '', detail: undefined, severity: 0, busy: false, command: undefined, accessibilityInformation: undefined, dispose() { } };
	},
	createDiagnosticCollection: diagnosticCollection,
	async getLanguages() { return [...new Set([...docs.values()].map(d => d.languageId))]; },
	onDidChangeDiagnostics: diagnosticsChanged.event,
	getDiagnostics,
	setLanguageConfiguration() { return new Disposable(() => { }); },
	match,
	registerCompletionItemProvider: registerProvider('completion'),
	registerHoverProvider: registerProvider('hover'),
	registerDefinitionProvider: registerProvider('definition'),
	registerDeclarationProvider: registerProvider('declaration'),
	registerTypeDefinitionProvider: registerProvider('typeDefinition'),
	registerImplementationProvider: registerProvider('implementation'),
	async setTextDocumentLanguage(document, languageId) { document.languageId = languageId; return document; },
}, {
	get(target, prop) {
		if (prop in target) { return target[prop]; }
		if (typeof prop === 'string' && prop.startsWith('register')) { return () => new Disposable(() => { }); }
		return undefined;
	},
});

// ---------------------------------------------------------------------------
// vscode.extensions / vscode.env
// ---------------------------------------------------------------------------

function extensionObject(desc) {
	const active = state.activated.get(desc.id);
	return {
		id: desc.id,
		extensionPath: desc.path,
		extensionUri: Uri.file(desc.path),
		packageJSON: desc.manifest,
		extensionKind: 1,
		get isActive() { return state.activated.has(desc.id); },
		get exports() { const a = state.activated.get(desc.id); return a && a.exports; },
		activate: () => activate(desc.id, 'api').then(() => state.activated.get(desc.id) && state.activated.get(desc.id).exports),
		_active: active,
	};
}

const extensions = {
	getExtension(id) {
		const desc = state.extensions.find(e => e.id.toLowerCase() === String(id).toLowerCase());
		return desc ? extensionObject(desc) : undefined;
	},
	get all() { return state.extensions.map(extensionObject); },
	get allAcrossExtensionHosts() { return state.extensions.map(extensionObject); },
	onDidChange: silentEvent,
};

const env = {
	appName: 'Visual Studio Code',
	appHost: 'desktop',
	appRoot: path.dirname(__filename),
	uriScheme: 'vscode',
	language: 'en',
	machineId: 'someValue.machineId',
	sessionId: String(process.pid) + Date.now(),
	remoteName: undefined,
	shell: process.env.SHELL || '',
	uiKind: 1,
	isNewAppInstall: false,
	isTelemetryEnabled: false,
	onDidChangeTelemetryEnabled: silentEvent,
	clipboard: {
		readText: () => request({ type: 'clipboardRead' }).then(v => v || ''),
		writeText: async text => { send({ type: 'clipboardWrite', text: String(text) }); },
	},
	async openExternal(uri) { send({ type: 'openExternal', url: String(uri) }); return true; },
	async asExternalUri(uri) { return uri; },
	createTelemetryLogger() { return { logUsage() { }, logError() { }, dispose() { }, onDidChangeEnableStates: silentEvent }; },
};

// ---------------------------------------------------------------------------
// The `vscode` module
// ---------------------------------------------------------------------------

const vscodeApi = {
	version: '1.119.0',
	commands, window, workspace, languages, extensions, env,
	Disposable, EventEmitter, Uri, Position, Range, Selection,
	ThemeColor, ThemeIcon, MarkdownString, CancellationTokenSource, TreeItem,
	StatusBarAlignment, ProgressLocation, FileType,
	ViewColumn: { Active: -1, Beside: -2, One: 1, Two: 2, Three: 3 },
	ConfigurationTarget: { Global: 1, Workspace: 2, WorkspaceFolder: 3 },
	ExtensionMode: { Production: 1, Development: 2, Test: 3 },
	ExtensionKind: { UI: 1, Workspace: 2 },
	TreeItemCollapsibleState: { None: 0, Collapsed: 1, Expanded: 2 },
	DiagnosticSeverity: { Error: 0, Warning: 1, Information: 2, Hint: 3 },
	UIKind: { Desktop: 1, Web: 2 },
	LogLevel: { Off: 0, Trace: 1, Debug: 2, Info: 3, Warning: 4, Error: 5 },
	Diagnostic, DiagnosticRelatedInformation, CompletionItem, CompletionList, Hover, Location,
	NotebookCellOutputItem: class NotebookCellOutputItem {
		static text(value, mime = 'text/plain') { return new NotebookCellOutputItem(Buffer.from(String(value)), mime); }
		static json(value, mime = 'text/x-json') { return new NotebookCellOutputItem(Buffer.from(JSON.stringify(value, undefined, '\t')), mime); }
		static stdout(value) { return NotebookCellOutputItem.text(value, 'application/vnd.code.notebook.stdout'); }
		static stderr(value) { return NotebookCellOutputItem.text(value, 'application/vnd.code.notebook.stderr'); }
		static error(err) {
			const value = { name: err && err.name || 'Error', message: err && err.message || String(err), stack: err && err.stack };
			return NotebookCellOutputItem.json(value, 'application/vnd.code.notebook.error');
		}
		constructor(data, mime) { this.data = data; this.mime = mime; }
	},
	NotebookCellOutput: class NotebookCellOutput {
		constructor(items, idOrMetadata, metadata) { this.items = items; this.metadata = typeof idOrMetadata === 'object' ? idOrMetadata : metadata; }
	},
	TextEdit, WorkspaceEdit, TextDocument, TextLine, EndOfLine,
	TabInputText, TabInputTextDiff, TabInputWebview, TabInputCustom, TabInputNotebook, TabInputTerminal,
	SnippetString: class SnippetString {
		constructor(value = '') { this.value = value; this._tab = 1; }
		appendText(t) { this.value += t.replace(/[$}\\]/g, '\\$&'); return this; }
		appendTabstop(n = this._tab++) { this.value += '$' + n; return this; }
		appendPlaceholder(v, n = this._tab++) { this.value += '${' + n + ':' + (typeof v === 'string' ? v : '') + '}'; return this; }
		appendVariable(name, def) { this.value += '${' + name + (def ? ':' + def : '') + '}'; return this; }
	},
	CodeAction: plainClass('CodeAction'),
	CodeActionKind: hierarchicalKind('CodeActionKind', {
		Empty: '', QuickFix: 'quickfix', Refactor: 'refactor', RefactorExtract: 'refactor.extract',
		RefactorInline: 'refactor.inline', RefactorMove: 'refactor.move', RefactorRewrite: 'refactor.rewrite',
		Source: 'source', SourceOrganizeImports: 'source.organizeImports', SourceFixAll: 'source.fixAll',
		Notebook: 'notebook',
	}),
	DocumentDropOrPasteEditKind: hierarchicalKind('DocumentDropOrPasteEditKind', { Empty: '', Text: 'text', TextUpdateImports: 'text.updateImports' }),
	CodeLens: plainClass('CodeLens'),
	RelativePattern: plainClass('RelativePattern'),
	FileSystemError: class FileSystemError extends Error { },
};

// Every namespace, class and enum of the published API (generated from
// vscode.d.ts), so bundled extensions that copy the module's properties
// (`__toESM(require('vscode'))`) or import names from it find them all.
// Members the editor doesn't implement become harmless stubs (see below).
const API_NAMES = /*API_NAMES*/null;

function stubFor(key, member, kind) {
	switch (kind) {
		case 'event': return silentEvent;
		case 'array': return [];
		case 'value': return undefined;
		case 'object': return guarded(key, {});
		default:
			if (/^register[A-Z]/.test(member)) { return () => new Disposable(() => { }); }
			// `create*` returns an object the extension configures and
			// disposes (status items, collections, controllers…).
			if (/^create[A-Z]/.test(member)) {
				return () => guarded(`${key}()`, { dispose() { }, show() { }, hide() { } });
			}
			return (..._args) => { report(key); return undefined; };
	}
}

/** Members of the published API that are stubs here (for `apiCoverage`). */
const stubbed = { namespaces: {}, classes: [] };

function addPublishedApi() {
	if (!API_NAMES) { return; }
	for (const [name, members] of Object.entries(API_NAMES.enums)) {
		if (!(name in vscodeApi)) { vscodeApi[name] = Object.freeze({ ...members }); }
	}
	for (const name of API_NAMES.classes) {
		if (!(name in vscodeApi)) { vscodeApi[name] = placeholderClass(name); stubbed.classes.push(name); }
	}
	for (const [ns, members] of Object.entries(API_NAMES.namespaces)) {
		const target = vscodeApi[ns] || (vscodeApi[ns] = {});
		for (const [member, kind] of Object.entries(members)) {
			if (!(member in target)) {
				(stubbed.namespaces[ns] = stubbed.namespaces[ns] || []).push(member);
				Object.defineProperty(target, member, {
					get: () => { report(`vscode.${ns}.${member}`); return stubFor(`vscode.${ns}.${member}`, member, kind); },
					enumerable: true,
					configurable: true,
				});
			}
		}
	}
}

// vscode.l10n: messages come back in English with {0}/{name} filled in.
vscodeApi.l10n = {
	t(message, ...args) {
		let text = typeof message === 'object' && message ? message.message : String(message);
		const params = args.length === 1 && args[0] && typeof args[0] === 'object' && !Array.isArray(args[0]) ? args[0] : args;
		return text.replace(/\{([^}]+)\}/g, (m, key) => (key in params ? String(params[key]) : m));
	},
	bundle: undefined,
	uri: undefined,
};
vscodeApi.CancellationError = class CancellationError extends Error {
	constructor() { super('Canceled'); this.name = 'Canceled'; }
};

// Missing API surface degrades instead of throwing, and each gap is logged
// once so it shows up in the extension host log:
// - unknown `register*` functions return a disposable,
// - unknown `onDid*` / `onWill*` events never fire,
// - unknown namespaces (`tasks`, `debug`, `scm`…) are stubs of the same kind,
// - unknown classes (`DocumentLink`, `TimelineItem`…) are plain classes, so
//   `class X extends vscode.Y` works; their static members (enum values)
//   are distinct numbers.
const reported = new Set();
function report(key) {
	if (!reported.has(key)) {
		reported.add(key);
		send({ type: 'log', level: 'warn', text: `vscode API not available: ${key}` });
	}
}

function stubMember(name, prop) {
	if (/^register[A-Z]/.test(prop) || /^create[A-Z]/.test(prop) && prop.endsWith('Collection')) {
		return () => new Disposable(() => { });
	}
	if (/^on(Did|Will)[A-Z]/.test(prop)) { return silentEvent; }
	return undefined;
}

const plainClasses = new Map();
function placeholderClass(name) {
	if (!plainClasses.has(name)) {
		let next = 1;
		const values = new Map();
		const cls = new Proxy(plainClass(name), {
			get(t, prop, receiver) {
				if (prop in t || typeof prop !== 'string') { return Reflect.get(t, prop, receiver); }
				if (!values.has(prop)) { values.set(prop, next++); }
				return values.get(prop);
			},
		});
		plainClasses.set(name, cls);
	}
	return plainClasses.get(name);
}

const stubNamespaces = new Map();
function guarded(name, target) {
	return new Proxy(target, {
		get(t, prop) {
			if (prop in t || typeof prop !== 'string') { return t[prop]; }
			if (prop === 'then' || prop === '__esModule' || prop === 'default') { return undefined; }
			const key = `${name}.${prop}`;
			report(key);
			if (name === 'vscode' && /^[A-Z]/.test(prop)) { return placeholderClass(prop); }
			if (name === 'vscode' && /^[a-z]/.test(prop)) {
				if (!stubNamespaces.has(prop)) { stubNamespaces.set(prop, guarded(key, {})); }
				return stubNamespaces.get(prop);
			}
			return stubMember(name, prop);
		},
	});
}
addPublishedApi();
for (const ns of Object.keys(vscodeApi)) {
	const v = vscodeApi[ns];
	if (/^[a-z]/.test(ns) && v && typeof v === 'object' && !Array.isArray(v) && ns !== 'l10n') {
		vscodeApi[ns] = guarded(`vscode.${ns}`, v);
	}
}
const vscodeModule = guarded('vscode', vscodeApi);

// CommonJS extensions: `require('vscode')`.
const originalResolve = Module._resolveFilename;
Module._resolveFilename = function (request, ...rest) {
	if (request === 'vscode') { return 'vscode'; }
	return originalResolve.call(this, request, ...rest);
};
require.cache.vscode = { id: 'vscode', filename: 'vscode', loaded: true, exports: vscodeModule };

// ES-module extensions: `import * as vscode from 'vscode'`. A resolve hook
// maps the specifier to a generated module re-exporting the same object.
globalThis.__vscodeRustApi = vscodeModule;
try {
	const names = Object.keys(vscodeApi).filter(n => /^[A-Za-z_$][\w$]*$/.test(n));
	const shim = 'const api = globalThis.__vscodeRustApi;\nexport default api;\n' +
		names.map(n => `export const ${n} = api.${n};`).join('\n') + '\n';
	const shimUrl = 'data:text/javascript,' + encodeURIComponent(shim);
	const hooks = `export async function resolve(specifier, context, next) {
		if (specifier === 'vscode') { return { url: ${JSON.stringify(shimUrl)}, shortCircuit: true }; }
		return next(specifier, context);
	}`;
	if (typeof Module.register === 'function') {
		Module.register('data:text/javascript,' + encodeURIComponent(hooks));
	}
} catch (err) {
	send({ type: 'log', level: 'warn', text: 'ES-module extensions unavailable: ' + err });
}

// ---------------------------------------------------------------------------
// Activation
// ---------------------------------------------------------------------------

let currentExtensionId = null;

function memento(file) {
	let data = {};
	try { data = JSON.parse(fs.readFileSync(file, 'utf8')); } catch { }
	return {
		keys: () => Object.keys(data),
		get: (key, def) => key in data ? data[key] : def,
		async update(key, value) {
			if (value === undefined) { delete data[key]; } else { data[key] = value; }
			try { fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, JSON.stringify(data)); } catch { }
		},
		setKeysForSync() { },
	};
}

function storageDir(kind, id) {
	return path.join(state.storageRoot || path.join(os.tmpdir(), 'vscode-rust-ext'), kind, id);
}

async function activate(id, reason) {
	if (state.activated.has(id)) { return true; }
	const desc = state.extensions.find(e => e.id === id);
	if (!desc) { return false; }
	const record = { exports: undefined, context: null, module: null };
	state.activated.set(id, record);
	if (!desc.main) {
		send({ type: 'activated', id, reason, loadMs: 0, activateMs: 0 });
		return true;
	}
	const previous = currentExtensionId;
	currentExtensionId = id;
	// VS Code's activation times: code loading, then the activate() call.
	const loadStart = Date.now();
	let activateStart = loadStart;
	try {
		const mainPath = require.resolve(path.resolve(desc.path, desc.main));
		let mod;
		try {
			mod = require(mainPath);
		} catch (err) {
			// ES modules that `require` can't load (top-level await, older
			// Node): import them instead.
			if (!err || !/ERR_REQUIRE_(ESM|ASYNC_MODULE)/.test(err.code || '')) { throw err; }
			mod = await import(require('url').pathToFileURL(mainPath).href);
		}
		if (mod && mod.default && typeof mod.activate !== 'function' && typeof mod.default.activate === 'function') {
			mod = mod.default;
		}
		record.module = mod;
		const globalStorage = storageDir('global', id);
		const workspaceStorage = storageDir('workspace', id);
		const context = {
			subscriptions: [],
			extensionPath: desc.path,
			extensionUri: Uri.file(desc.path),
			extension: extensionObject(desc),
			extensionMode: 1,
			globalState: memento(path.join(globalStorage, 'state.json')),
			workspaceState: memento(path.join(workspaceStorage, 'state.json')),
			secrets: { get: async () => undefined, store: async () => { }, delete: async () => { }, onDidChange: silentEvent },
			storagePath: workspaceStorage,
			globalStoragePath: globalStorage,
			logPath: storageDir('logs', id),
			storageUri: Uri.file(workspaceStorage),
			globalStorageUri: Uri.file(globalStorage),
			logUri: Uri.file(storageDir('logs', id)),
			environmentVariableCollection: { replace() { }, append() { }, prepend() { }, get() { }, forEach() { }, delete() { }, clear() { }, persistent: false },
			asAbsolutePath: rel => path.join(desc.path, rel),
		};
		record.context = context;
		activateStart = Date.now();
		if (typeof mod.activate === 'function') {
			record.exports = await mod.activate(context);
		}
		send({ type: 'activated', id, reason, loadMs: activateStart - loadStart, activateMs: Date.now() - activateStart });
		return true;
	} catch (err) {
		send({ type: 'activationFailed', id, error: String(err && err.stack || err) });
		return false;
	} finally {
		currentExtensionId = previous;
	}
}

/** Activate whatever declares `onCommand:<id>` or contributes the command. */
async function activateForCommand(commandId) {
	let any = false;
	for (const e of state.extensions) {
		if (state.activated.has(e.id)) { continue; }
		const events = e.activationEvents || [];
		const contributes = (e.commands || []).includes(commandId);
		if (contributes || events.includes(`onCommand:${commandId}`)) {
			any = (await activate(e.id, `onCommand:${commandId}`)) || any;
		}
	}
	return any;
}

async function activateByEvent(event) {
	for (const e of state.extensions) {
		if ((e.activationEvents || []).includes(event)) { await activate(e.id, event); }
	}
}

// ---------------------------------------------------------------------------
// Requests from the editor
// ---------------------------------------------------------------------------

async function handle(msg) {
	if (handleDocumentMessage(msg)) { return; }
	if (handleWebviewMessage(msg)) { return; }
	switch (msg.type) {
		case 'init': {
			state.extensions = msg.extensions || [];
			state.storageRoot = msg.storageRoot || null;
			state.workspaceFolders = (msg.workspaceFolders || []).map((p, index) => ({ uri: Uri.file(p), name: path.basename(p), index }));
			state.configDefaults = msg.configDefaults || {};
			send({ type: 'ready' });
			for (const e of state.extensions) {
				const ev = e.activationEvents || [];
				if (ev.includes('*') || ev.includes('onStartupFinished')) { await activate(e.id, ev.includes('*') ? '*' : 'onStartupFinished'); }
			}
			for (const folder of state.workspaceFolders) {
				for (const e of state.extensions) {
					for (const ev of e.activationEvents || []) {
						if (!ev.startsWith('workspaceContains:')) { continue; }
						const pattern = ev.slice('workspaceContains:'.length);
						if (!/[*?{]/.test(pattern) && fs.existsSync(path.join(folder.uri.fsPath, pattern))) { await activate(e.id, ev); }
					}
				}
			}
			break;
		}
		case 'apiCoverage': {
			// How much of vscode.d.ts is implemented rather than stubbed.
			const namespaces = {};
			for (const [ns, members] of Object.entries((API_NAMES && API_NAMES.namespaces) || {})) {
				const missing = stubbed.namespaces[ns] || [];
				namespaces[ns] = { total: Object.keys(members).length, stubbed: missing };
			}
			send({
				type: 'apiCoverage', namespaces,
				classes: { total: ((API_NAMES && API_NAMES.classes) || []).length, stubbed: stubbed.classes },
				enums: { total: Object.keys((API_NAMES && API_NAMES.enums) || {}).length },
			});
			break;
		}
		case 'provideCompletion':
		case 'provideHover':
		case 'provideDefinition': {
			let result;
			try {
				result = msg.type === 'provideCompletion' ? await provideCompletion(msg)
					: msg.type === 'provideHover' ? await provideHover(msg)
						: [
							...await provideLocations('definition', 'provideDefinition', msg),
							...await provideLocations('declaration', 'provideDeclaration', msg),
						];
			} catch (err) { console.error(err); result = null; }
			send({ type: 'providerResult', requestId: msg.requestId, kind: msg.type, result });
			break;
		}
		case 'activateByEvent':
			await activateByEvent(msg.event);
			break;
		case 'executeCommand': {
			try {
				const result = await commands.executeCommand(msg.id, ...(msg.args || []));
				send({ type: 'commandResult', requestId: msg.requestId, id: msg.id, ok: true, result: safeJson(result) });
			} catch (err) {
				send({ type: 'commandResult', requestId: msg.requestId, id: msg.id, ok: false, error: String(err && err.message || err) });
			}
			break;
		}
		case 'response': {
			const resolve = pendingRequests.get(msg.requestId);
			if (resolve) { pendingRequests.delete(msg.requestId); resolve(msg.value); }
			break;
		}
		case 'shutdown': {
			for (const [, record] of state.activated) {
				try {
					if (record.module && typeof record.module.deactivate === 'function') { await record.module.deactivate(); }
					for (const d of (record.context && record.context.subscriptions) || []) { d && typeof d.dispose === 'function' && d.dispose(); }
				} catch (err) { console.error(err); }
			}
			process.exit(0);
		}
	}
}

const rl = readline.createInterface({ input: process.stdin });
rl.on('line', line => {
	let msg;
	try { msg = JSON.parse(line); } catch { return; }
	handle(msg).catch(err => console.error(err));
});
rl.on('close', () => process.exit(0));
