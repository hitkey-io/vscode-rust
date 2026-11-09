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
		// Context keys drive `when` clauses, which the editor doesn't evaluate yet.
		if (id === 'setContext') { return undefined; }
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
		show() { }, hide() { }, dispose() { outputChannels.delete(name); },
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
	return ch;
}

const ProgressLocation = { SourceControl: 1, Window: 10, Notification: 15 };

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
		const list = await items;
		const labels = (list || []).map(i => typeof i === 'string' ? i : i.label);
		const index = await request({ type: 'showQuickPick', items: labels, placeholder: options && options.placeHolder || null });
		return typeof index === 'number' && index >= 0 ? list[index] : undefined;
	},
	async showInputBox(options) {
		const value = await request({
			type: 'showInputBox', prompt: options && options.prompt || null,
			placeholder: options && options.placeHolder || null, value: options && options.value || '',
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
	activeTextEditor: undefined,
	visibleTextEditors: [],
	tabGroups: {
		all: [],
		activeTabGroup: { tabs: [], activeTab: undefined, isActive: true, viewColumn: 1 },
		onDidChangeTabGroups: silentEvent,
		onDidChangeTabs: silentEvent,
		async close() { return false; },
	},
	terminals: [],
	state: { focused: true, active: true },
	onDidChangeActiveTextEditor: silentEvent,
	onDidChangeVisibleTextEditors: silentEvent,
	onDidChangeTextEditorSelection: silentEvent,
	onDidChangeWindowState: silentEvent,
	onDidOpenTerminal: silentEvent,
	onDidCloseTerminal: silentEvent,
	createTextEditorDecorationType() { return { key: 'decoration', dispose() { } }; },
	registerTreeDataProvider() { return new Disposable(() => { }); },
	createTreeView() {
		return { onDidChangeSelection: silentEvent, onDidChangeVisibility: silentEvent, onDidExpandElement: silentEvent, onDidCollapseElement: silentEvent, reveal: async () => { }, dispose() { } };
	},
	registerWebviewViewProvider() { return new Disposable(() => { }); },
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
	onDidOpenTextDocument: silentEvent,
	onDidCloseTextDocument: silentEvent,
	onDidChangeTextDocument: silentEvent,
	onDidSaveTextDocument: silentEvent,
	onWillSaveTextDocument: silentEvent,
	onDidChangeWorkspaceFolders: silentEvent,
	onDidCreateFiles: silentEvent,
	onDidDeleteFiles: silentEvent,
	onDidRenameFiles: silentEvent,
	textDocuments: [],
	fs: workspaceFs,
	isTrusted: true,
	onDidGrantWorkspaceTrust: silentEvent,
	async findFiles() { return []; },
	createFileSystemWatcher() {
		return { onDidCreate: silentEvent, onDidChange: silentEvent, onDidDelete: silentEvent, dispose() { } };
	},
	registerTextDocumentContentProvider() { return new Disposable(() => { }); },
	registerFileSystemProvider() { return new Disposable(() => { }); },
};

// ---------------------------------------------------------------------------
// vscode.languages (providers are accepted but not consulted yet)
// ---------------------------------------------------------------------------

function diagnosticCollection(name) {
	const map = new Map();
	return {
		name,
		set(uri, diags) { if (Array.isArray(uri)) { for (const [u, d] of uri) { map.set(String(u), d); } } else { map.set(String(uri), diags); } },
		delete(uri) { map.delete(String(uri)); },
		clear() { map.clear(); },
		get(uri) { return map.get(String(uri)); },
		has(uri) { return map.has(String(uri)); },
		forEach(cb) { map.forEach((v, k) => cb(Uri.parse(k), v, this)); },
		dispose() { map.clear(); },
	};
}

const languages = new Proxy({
	createLanguageStatusItem(id, selector) {
		return { id, selector, name: undefined, text: '', detail: undefined, severity: 0, busy: false, command: undefined, accessibilityInformation: undefined, dispose() { } };
	},
	createDiagnosticCollection: diagnosticCollection,
	async getLanguages() { return []; },
	onDidChangeDiagnostics: silentEvent,
	getDiagnostics() { return []; },
	setLanguageConfiguration() { return new Disposable(() => { }); },
	match() { return 0; },
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
	EndOfLine: { LF: 1, CRLF: 2 },
	UIKind: { Desktop: 1, Web: 2 },
	LogLevel: { Off: 0, Trace: 1, Debug: 2, Info: 3, Warning: 4, Error: 5 },
	CompletionItemKind: new Proxy({}, { get: (_t, p) => typeof p === 'string' ? 0 : undefined }),
	SymbolKind: new Proxy({}, { get: (_t, p) => typeof p === 'string' ? 0 : undefined }),
	Diagnostic: plainClass('Diagnostic'),
	CompletionItem: plainClass('CompletionItem'),
	CompletionList: plainClass('CompletionList'),
	Hover: plainClass('Hover'),
	Location: plainClass('Location'),
	TextEdit: plainClass('TextEdit'),
	WorkspaceEdit: plainClass('WorkspaceEdit'),
	CodeAction: plainClass('CodeAction'),
	CodeActionKind: hierarchicalKind('CodeActionKind', {
		Empty: '', QuickFix: 'quickfix', Refactor: 'refactor', RefactorExtract: 'refactor.extract',
		RefactorInline: 'refactor.inline', RefactorMove: 'refactor.move', RefactorRewrite: 'refactor.rewrite',
		Source: 'source', SourceOrganizeImports: 'source.organizeImports', SourceFixAll: 'source.fixAll',
		Notebook: 'notebook',
	}),
	DocumentDropOrPasteEditKind: hierarchicalKind('DocumentDropOrPasteEditKind', { Empty: '', Text: 'text', TextUpdateImports: 'text.updateImports' }),
	CodeLens: plainClass('CodeLens'),
	SnippetString: plainClass('SnippetString'),
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

function addPublishedApi() {
	if (!API_NAMES) { return; }
	for (const [name, members] of Object.entries(API_NAMES.enums)) {
		if (!(name in vscodeApi)) { vscodeApi[name] = Object.freeze({ ...members }); }
	}
	for (const name of API_NAMES.classes) {
		if (!(name in vscodeApi)) { vscodeApi[name] = placeholderClass(name); }
	}
	for (const [ns, members] of Object.entries(API_NAMES.namespaces)) {
		const target = vscodeApi[ns] || (vscodeApi[ns] = {});
		for (const [member, kind] of Object.entries(members)) {
			if (!(member in target)) {
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
		send({ type: 'activated', id, reason });
		return true;
	}
	const previous = currentExtensionId;
	currentExtensionId = id;
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
		if (typeof mod.activate === 'function') {
			record.exports = await mod.activate(context);
		}
		send({ type: 'activated', id, reason });
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
