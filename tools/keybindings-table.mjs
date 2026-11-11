// Reads the Keyboard Shortcuts editor's table out of a running VS Code, for
// tools/commands.mjs (labels and sources exactly as VS Code shows them):
//
//   Code --user-data-dir=<fresh profile with "chat.disableAIFeatures": true> \
//        --extensions-dir=<empty> -n --remote-debugging-port=9335
//   node tools/keybindings-table.mjs open          # presses ⌘K ⌘S
//   node tools/keybindings-table.mjs > rows.json   # scrolls through the table
//
// Each row: { i, label, id, title (the command column's aria-label "label
// (id)"), keys ("⌘|K ⌘|C"), when ("-" when empty), source, h }.
const list = await (await fetch(`http://127.0.0.1:${process.env.CDP_PORT ?? 9335}/json/list`)).json();
const page = list.find(t => t.type === 'page');
const ws = new WebSocket(page.webSocketDebuggerUrl);
let id = 0; const pending = new Map();
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); } };
await new Promise(r => ws.onopen = r);
const send = (method, params = {}) => new Promise(r => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const sleep = ms => new Promise(r => setTimeout(r, ms));
const evalJs = async (expr) => (await send('Runtime.evaluate', { expression: expr, returnByValue: true })).result?.result?.value;
const key = async (k, code, vk, modifiers) => {
  await send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: k, code, windowsVirtualKeyCode: vk, modifiers });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: k, code, windowsVirtualKeyCode: vk, modifiers });
};
const mode = process.argv[2] ?? 'dump';
if (mode === 'open') {
  await key('k', 'KeyK', 75, 4); await sleep(300); await key('s', 'KeyS', 83, 4); await sleep(2000);
  console.log(await evalJs(`!!document.querySelector('.keybindings-editor')`));
  process.exit(0);
}
const rows = new Map();
let total = await evalJs(`document.querySelector('.keybindings-editor .monaco-list').getAttribute('aria-rowcount') || ''`);
console.error('rowcount', total);
const box = await evalJs(`(() => { const r = document.querySelector('.keybindings-editor .monaco-list-rows').parentElement.getBoundingClientRect(); return [r.x + r.width/2, r.y + r.height/2]; })()`);
for (let k = 0; k < 3000; k++) { if (await evalJs(`!!document.querySelector('.keybindings-editor .monaco-list-row[data-index="0"]')`)) break; await send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: box[0], y: box[1], deltaX: 0, deltaY: -2000 }); await sleep(10); }
await sleep(300);
let stale = 0;
for (let step = 0; step < 5000 && stale < 8; step++) {
  const got = await evalJs(`[...document.querySelectorAll('.keybindings-editor .monaco-list-row')].map(r => {
    const tds = r.querySelectorAll('.monaco-table-td');
    const cmd = tds[1];
    return {
      i: +r.getAttribute('data-index'),
      label: cmd.querySelector('.command-label:not(.hide)')?.textContent ?? '',
      id: cmd.querySelector('.command-id:not(.hide)')?.textContent ?? '',
      title: cmd.querySelector('.command')?.getAttribute('aria-label') ?? '',
      keys: [...tds[2].querySelectorAll('.monaco-keybinding')].map(k => [...k.children].map(c => c.classList.contains('monaco-keybinding-key-chord-separator') ? ' ' : c.textContent).join('|')).join(''),
      when: tds[3].innerText,
      source: tds[4].innerText,
      h: r.offsetHeight,
    };
  })`);
  const before = rows.size;
  for (const r of got) rows.set(r.i, r);
  stale = rows.size === before ? stale + 1 : 0;
  await send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: box[0], y: box[1], deltaX: 0, deltaY: 400 });
  await sleep(40);
}
const out = [...rows.values()].sort((a, b) => a.i - b.i);
console.error('rows', out.length);
process.stdout.write(JSON.stringify(out));
process.exit(0);
