// Run in a clean VS Code with no folder open:
//   Code --user-data-dir=/tmp/dump-profile --extensions-dir=<dir holding this folder> -n
// Output goes to $VSCODE_DUMP_DIR (default /tmp/vscode-settings-dump); `done`
// appears when everything is written.
const vscode = require('vscode');
const fs = require('fs');

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

exports.activate = async function () {
  const out = process.env.VSCODE_DUMP_DIR || '/tmp/vscode-settings-dump';
  fs.mkdirSync(out, { recursive: true });
  const log = (m) => fs.appendFileSync(`${out}/log.txt`, m + '\n');
  await wait(4000);
  const dumpActive = async (command, file) => {
    try {
      await vscode.commands.executeCommand(command);
      await wait(1500);
      const ed = vscode.window.activeTextEditor;
      log(`${command}: ${ed && ed.document.uri.toString()}`);
      if (ed) fs.writeFileSync(`${out}/${file}`, ed.document.getText());
    } catch (e) {
      log(`${command} failed: ${e.stack || e}`);
    }
  };
  await dumpActive('workbench.action.openRawDefaultSettings', 'defaultSettings.json');
  await dumpActive('workbench.action.openDefaultKeybindingsFile', 'keybindings.json');
  for (const uri of ['vscode://schemas/settings/default', 'vscode://schemas/settings/user']) {
    try {
      const doc = await vscode.workspace.openTextDocument(vscode.Uri.parse(uri));
      fs.writeFileSync(`${out}/schema-${uri.split('/').pop()}.json`, doc.getText());
    } catch (e) {
      log(`schema ${uri} failed: ${e.message}`);
    }
  }
  fs.writeFileSync(`${out}/done`, 'ok');
};
