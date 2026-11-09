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
	console.log('hello-world activated');
}

function deactivate() { }

module.exports = { activate, deactivate };
