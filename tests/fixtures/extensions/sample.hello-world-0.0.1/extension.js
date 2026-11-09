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
	// Language features for Markdown files.
	const selector = { language: 'markdown' };
	context.subscriptions.push(vscode.languages.registerCompletionItemProvider(selector, {
		provideCompletionItems(document, position) {
			const fn = new vscode.CompletionItem('helloWorld', vscode.CompletionItemKind.Function);
			fn.detail = 'Say hello';
			fn.documentation = new vscode.MarkdownString('Inserts a **greeting**.');
			fn.insertText = new vscode.SnippetString('helloWorld(${1:name})');
			const kw = new vscode.CompletionItem('heading', vscode.CompletionItemKind.Keyword);
			return [fn, kw];
		},
	}, '.'));
	context.subscriptions.push(vscode.languages.registerHoverProvider(selector, {
		provideHover(document, position) {
			const range = document.getWordRangeAtPosition(position);
			if (!range || document.getText(range) !== 'TODO') { return undefined; }
			return new vscode.Hover(new vscode.MarkdownString('**TODO** marks unfinished work'), range);
		},
	}));
	context.subscriptions.push(vscode.languages.registerDefinitionProvider(selector, {
		provideDefinition(document, position) {
			const range = document.getWordRangeAtPosition(position);
			if (!range || document.getText(range) !== 'package') { return undefined; }
			const target = vscode.Uri.joinPath(vscode.workspace.workspaceFolders[0].uri, 'package.json');
			return new vscode.Location(target, new vscode.Position(1, 2));
		},
	}));
	const problems = vscode.languages.createDiagnosticCollection('hello');
	context.subscriptions.push(problems);
	const lint = document => {
		if (document.languageId !== 'markdown') { return; }
		const out = [];
		document.getText().split('\n').forEach((line, i) => {
			for (const [word, severity] of [['FIXME', vscode.DiagnosticSeverity.Warning], ['BUG', vscode.DiagnosticSeverity.Error]]) {
				const at = line.indexOf(word);
				if (at >= 0) {
					const d = new vscode.Diagnostic(new vscode.Range(i, at, i, at + word.length), `${word} found`, severity);
					d.source = 'hello';
					out.push(d);
				}
			}
		});
		problems.set(document.uri, out);
	};
	vscode.workspace.textDocuments.forEach(lint);
	context.subscriptions.push(vscode.workspace.onDidOpenTextDocument(lint));
	context.subscriptions.push(vscode.workspace.onDidChangeTextDocument(e => lint(e.document)));

	console.log('hello-world activated');
}

function deactivate() { }

module.exports = { activate, deactivate };
