const vscode = acquireVsCodeApi();
document.getElementById('send').addEventListener('click', () => vscode.postMessage({ text: 'hi' }));
window.addEventListener('message', e => { document.getElementById('reply').textContent = e.data.reply; });
vscode.postMessage({ text: 'loaded' });
