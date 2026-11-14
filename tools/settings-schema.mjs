// Turns the settings schema dumped from VS Code (tools/dump-settings) into
// assets/settings/settings.json: one entry per setting the Settings editor
// shows, in registration order, with what it renders (preferencesModels.ts
// `parseSetting`, settingsTreeModels.ts `initSettingValueType`).
//
//   node tools/settings-schema.mjs /tmp/vscode-settings-dump assets/settings/settings.json

import fs from 'node:fs';
import path from 'node:path';

const [dumpDir, outFile] = process.argv.slice(2);
if (!dumpDir || !outFile) {
	console.error('usage: node tools/settings-schema.mjs <dump dir> <out.json>');
	process.exit(2);
}

const schema = JSON.parse(fs.readFileSync(path.join(dumpDir, 'schema-default.json'), 'utf8'));
const defs = schema.$defs ?? {};

// `$ref: "#/$defs/_12"` → the definition (merged with sibling keys).
function resolve(node, depth = 0) {
	if (!node || typeof node !== 'object' || depth > 8) {
		return node;
	}
	if (Array.isArray(node)) {
		return node.map((n) => resolve(n, depth + 1));
	}
	let out = node;
	if (typeof node.$ref === 'string' && node.$ref.startsWith('#/$defs/')) {
		const { $ref, ...rest } = node;
		out = { ...resolve(defs[$ref.slice('#/$defs/'.length)], depth + 1), ...rest };
	}
	const result = {};
	for (const [k, v] of Object.entries(out)) {
		result[k] = v && typeof v === 'object' ? resolve(v, depth + 1) : v;
	}
	return result;
}

const SIMPLE = new Set(['string', 'boolean', 'integer', 'number']);
const EXCLUDE = new Set(['files.exclude', 'search.exclude', 'workbench.localHistory.exclude', 'explorer.autoRevealExclude', 'files.readonlyExclude', 'files.watcherExclude']);
const OVERRIDE = /^\[.*\]$/;

function renderableType(s, key) {
	const { type } = s;
	if (Array.isArray(type)) {
		if (key === 'workbench.editor.customLabels.patterns' && type.length === 2 && type.includes('null') && type.some((t) => SIMPLE.has(t))) {
			return 'simple';
		}
		return type.every((t) => SIMPLE.has(t)) ? 'complex' : false;
	}
	if (SIMPLE.has(type)) {
		return 'simple';
	}
	if (type === 'array' && s.items) {
		for (const item of Array.isArray(s.items) ? s.items : [s.items]) {
			const types = Array.isArray(item.type) ? item.type : [item.type];
			if (!types.every((t) => SIMPLE.has(t))) {
				return false;
			}
		}
		return 'complex';
	}
	return false;
}

function objectSchemaType(key, s) {
	if (s.type !== 'object') {
		return false;
	}
	const { properties, patternProperties, additionalProperties } = s;
	if (properties == null && patternProperties == null && additionalProperties == null) {
		return false;
	}
	if ((additionalProperties === true || additionalProperties === undefined) && !Object.keys(patternProperties ?? {}).includes('.*')) {
		return false;
	}
	const schemas = [...Object.values(properties ?? {}), ...Object.values(patternProperties ?? {})];
	if (additionalProperties && typeof additionalProperties === 'object') {
		schemas.push(additionalProperties);
	}
	let result = 'simple';
	for (const sub of schemas) {
		for (const part of Array.isArray(sub.anyOf) ? sub.anyOf : [sub]) {
			const t = renderableType(part, key);
			if (t === false) {
				return false;
			}
			if (t === 'complex') {
				result = 'complex';
			}
		}
	}
	return result;
}

function valueType(key, s, arrayItemType, enumValues, allKeysAreBoolean) {
	const enumRenderable = (t) => (Array.isArray(t) ? t : [t]).every((x) => ['string', 'boolean', 'null', 'integer', 'number'].includes(x));
	if (enumValues && (!s.type || enumRenderable(s.type))) {
		return 'enum';
	}
	if (s.type === 'string') {
		return s.editPresentation === 'multilineText' ? 'multilineString' : 'string';
	}
	if (EXCLUDE.has(key)) {
		return 'exclude';
	}
	if (key === 'files.readonlyInclude') {
		return 'include';
	}
	if (s.type === 'integer') {
		return 'integer';
	}
	if (s.type === 'number') {
		return 'number';
	}
	if (s.type === 'boolean') {
		return 'boolean';
	}
	if (s.type === 'array' && arrayItemType && ['string', 'enum', 'number', 'integer'].includes(arrayItemType)) {
		return 'array';
	}
	if (Array.isArray(s.type) && s.type.includes('null') && s.type.length === 2) {
		if (s.type.includes('integer')) {
			return 'nullableInteger';
		}
		if (s.type.includes('number')) {
			return 'nullableNumber';
		}
		return 'complex';
	}
	const objectType = objectSchemaType(key, s);
	if (objectType) {
		if (allKeysAreBoolean) {
			return 'booleanObject';
		}
		return objectType === 'simple' ? 'object' : 'complexObject';
	}
	return OVERRIDE.test(key) ? 'languageTag' : 'complex';
}

const settings = [];
for (const [key, raw] of Object.entries(schema.properties)) {
	const s = resolve(raw);
	if (!s.section) {
		continue; // preferencesModels.ts skips settings without a section
	}
	const description = s.markdownDescription || s.description || '';
	let arrayItemType;
	if (s.type === 'array' && s.items && !Array.isArray(s.items) && s.items.type) {
		arrayItemType = s.items.enum ? 'enum' : Array.isArray(s.items.type) ? undefined : s.items.type;
	}
	let enumValues = s.enum;
	let enumDescriptions = s.markdownEnumDescriptions ?? s.enumDescriptions;
	let enumMarkdown = !!s.markdownEnumDescriptions;
	if (arrayItemType === 'enum') {
		enumValues = s.items.enum;
		enumDescriptions = s.items.markdownEnumDescriptions ?? s.items.enumDescriptions;
		enumMarkdown = !!s.items.markdownEnumDescriptions;
	}
	const allKeysAreBoolean = s.type === 'object' && !s.additionalProperties && s.properties && Object.keys(s.properties).length > 0
		&& Object.values(s.properties).every((p) => p.type === 'boolean');
	const entry = {
		key,
		type: s.type,
		valueType: valueType(key, s, arrayItemType, enumValues, allKeysAreBoolean),
		default: s.default,
		description,
	};
	const put = (name, value) => {
		if (value !== undefined && value !== null && value !== false && !(Array.isArray(value) && value.length === 0)) {
			entry[name] = value;
		}
	};
	put('markdown', !!s.markdownDescription);
	put('enum', enumValues);
	put('enumDescriptions', enumDescriptions?.some((d) => d) ? enumDescriptions : undefined);
	put('enumMarkdown', enumMarkdown && !!enumDescriptions);
	put('enumItemLabels', s.enumItemLabels);
	put('arrayItemType', arrayItemType);
	put('minimum', s.minimum);
	put('maximum', s.maximum);
	put('tags', s.tags);
	put('keywords', s.keywords);
	put('scope', s.scope);
	put('order', s.order);
	put('title', s.title);
	put('restricted', s.restricted);
	put('deprecation', s.markdownDeprecationMessage || s.deprecationMessage);
	put('deprecationMarkdown', !!s.markdownDeprecationMessage);
	put('deprecationShowInSettings', s.deprecationMessageShowInSettings);
	if (allKeysAreBoolean) {
		entry.properties = Object.fromEntries(Object.entries(s.properties).map(([k, p]) => [k, p.markdownDescription || p.description || '']));
	}
	if (entry.valueType === 'object') {
		// What the object widget needs (settingsTreeModels.ts: objectProperties,
		// objectPatternProperties, objectAdditionalProperties).
		const valueSchema = (v) => {
			if (!v || typeof v !== 'object') { return undefined; }
			const o = { type: 'string' };
			const branches = v.anyOf ? v.anyOf : [v];
			if (branches.some((b) => b.type === 'string' && b.enum?.length > 0)) {
				const e = branches.find((b) => b.type === 'string' && b.enum?.length > 0);
				o.type = 'enum';
				o.enum = e.enum;
				o.enumDescriptions = e.markdownEnumDescriptions ?? e.enumDescriptions;
			} else if (!v.anyOf && v.type === 'boolean') {
				o.type = 'boolean';
			}
			o.description = v.markdownDescription || v.description || undefined;
			return o;
		};
		if (s.properties) {
			entry.objectProperties = Object.fromEntries(Object.entries(s.properties).map(([k, p]) => [k, valueSchema(p) ?? { type: 'string' }]));
		}
		if (s.patternProperties) {
			entry.objectPatternProperties = Object.entries(s.patternProperties).map(([k, p]) => [k, valueSchema(p) ?? { type: 'string' }]);
		}
		if (s.additionalProperties && typeof s.additionalProperties === 'object') {
			entry.objectAdditional = valueSchema(s.additionalProperties);
		}
		if (s.additionalProperties === false || s.additionalProperties === undefined && (s.properties || s.patternProperties)) {
			entry.objectClosed = true;
		}
	}
	entry.section = { id: s.section.id || '', title: s.section.title || '', order: s.section.order };
	if (s.source && typeof s.source === 'object') {
		entry.source = { id: s.source.id, displayName: s.source.displayName };
	}
	settings.push(entry);
}

fs.mkdirSync(path.dirname(outFile), { recursive: true });
fs.writeFileSync(outFile, JSON.stringify(settings) + '\n');
const types = {};
for (const s of settings) {
	types[s.valueType] = (types[s.valueType] ?? 0) + 1;
}
console.log(`${settings.length} settings →`, outFile, types);
