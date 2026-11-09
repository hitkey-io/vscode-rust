const vscode = require('vscode');

function activate(context) {
	const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
	item.text = '$(smiley) Hello';
	item.tooltip = 'Say hello';
	item.command = 'helloWorld.sayHello';
	item.show();
	context.subscriptions.push(item);

	context.subscriptions.push(vscode.commands.registerCommand('helloWorld.sayHello', () => {
		const greeting = vscode.workspace.getConfiguration('helloWorld').get('greeting');
		const folder = vscode.workspace.workspaceFolders ? vscode.workspace.workspaceFolders[0].name : 'no folder';
		vscode.window.showInformationMessage(`${greeting} from ${folder}!`);
	}));

	context.subscriptions.push(vscode.commands.registerCommand('helloWorld.askChoice', async () => {
		const choice = await vscode.window.showWarningMessage('Pick one', 'Yes', 'No');
		vscode.window.showInformationMessage(`You picked ${choice || 'nothing'}`);
	}));

	context.subscriptions.push(vscode.commands.registerCommand('helloWorld.fail', () => {
		throw new Error('boom');
	}));

	context.subscriptions.push(vscode.commands.registerCommand('helloWorld.internal', () => 42));

	// Highlight every TODO in the active editor, like TODO-highlighting extensions.
	const todo = vscode.window.createTextEditorDecorationType({ backgroundColor: '#ffbd2a', color: '#fff' });
	const decorate = editor => {
		if (!editor) { return; }
		const text = editor.document.getText();
		const ranges = [];
		const re = /TODO/g;
		let m;
		while ((m = re.exec(text))) {
			ranges.push(new vscode.Range(editor.document.positionAt(m.index), editor.document.positionAt(m.index + m[0].length)));
		}
		editor.setDecorations(todo, ranges);
	};
	decorate(vscode.window.activeTextEditor);
	context.subscriptions.push(vscode.window.onDidChangeActiveTextEditor(decorate));
	context.subscriptions.push(vscode.workspace.onDidChangeTextDocument(e => {
		if (vscode.window.activeTextEditor && e.document === vscode.window.activeTextEditor.document) {
			decorate(vscode.window.activeTextEditor);
		}
	}));

	context.subscriptions.push(vscode.commands.registerCommand('helloWorld.upperFirstLine', async () => {
		const editor = vscode.window.activeTextEditor;
		if (!editor) { return; }
		const line = editor.document.lineAt(0);
		await editor.edit(b => b.replace(line.range, line.text.toUpperCase()));
	}));

	context.subscriptions.push(vscode.commands.registerCommand('helloWorld.pick', async () => {
		const picked = await vscode.window.showQuickPick(['Apple', 'Banana', 'Cherry'], { placeHolder: 'Pick a fruit' });
		const name = await vscode.window.showInputBox({ prompt: 'Your name', value: 'Ada' });
		vscode.window.showInformationMessage(`Picked ${picked || 'nothing'} for ${name || 'nobody'}`);
	}));
	console.log('hello-world activated');
}

function deactivate() { }

module.exports = { activate, deactivate };
