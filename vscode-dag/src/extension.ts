import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

const executionsRoot = () =>
  path.join(os.homedir(), ".autonomic", "logs", "spine", "executions");

interface DagNode {
  sequence: number;
  transition?: string;
  payload_keys?: string[];
}

interface DagFile {
  execution_id: string;
  nodes: DagNode[];
}

class DagTreeItem extends vscode.TreeItem {
  constructor(
    public readonly label: string,
    public readonly dagPath: string,
    collapsibleState: vscode.TreeItemCollapsibleState
  ) {
    super(label, collapsibleState);
    this.command = {
      command: "autonomic.openDagFile",
      title: "Open DAG",
      arguments: [dagPath],
    };
  }
}

class DagProvider implements vscode.TreeDataProvider<DagTreeItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<DagTreeItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  refresh(): void {
    this._onDidChangeTreeData.fire(undefined);
  }

  getTreeItem(element: DagTreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(): Thenable<DagTreeItem[]> {
    const root = executionsRoot();
    if (!fs.existsSync(root)) {
      return Promise.resolve([
        new DagTreeItem("(no executions yet)", "", vscode.TreeItemCollapsibleState.None),
      ]);
    }

    const files = fs
      .readdirSync(root)
      .filter((f) => f.endsWith(".dag.json"))
      .sort()
      .reverse()
      .slice(0, 50);

    return Promise.resolve(
      files.map(
        (f) =>
          new DagTreeItem(
            f.replace(".dag.json", ""),
            path.join(root, f),
            vscode.TreeItemCollapsibleState.None
          )
      )
    );
  }
}

function renderDagPanel(context: vscode.ExtensionContext, dagPath: string): void {
  const raw = fs.readFileSync(dagPath, "utf8");
  const dag = JSON.parse(raw) as DagFile;
  const panel = vscode.window.createWebviewPanel(
    "autonomicDag",
    `DAG: ${dag.execution_id}`,
    vscode.ViewColumn.Beside,
    { enableScripts: false }
  );

  const rows = dag.nodes
    .map(
      (n) =>
        `<tr><td>${n.sequence}</td><td>${n.transition ?? "—"}</td><td>${(n.payload_keys ?? []).join(", ")}</td></tr>`
    )
    .join("");

  panel.webview.html = `<!DOCTYPE html>
<html>
<head><meta charset="UTF-8"><style>
body { font-family: var(--vscode-font-family); color: var(--vscode-foreground); background: var(--vscode-editor-background); padding: 12px; }
table { border-collapse: collapse; width: 100%; }
th, td { border: 1px solid var(--vscode-panel-border); padding: 6px 8px; text-align: left; }
th { background: var(--vscode-editor-selectionBackground); }
</style></head>
<body>
<h2>Execution ${dag.execution_id}</h2>
<table>
<thead><tr><th>#</th><th>Transition</th><th>Payload keys</th></tr></thead>
<tbody>${rows}</tbody>
</table>
</body></html>`;
}

export function activate(context: vscode.ExtensionContext): void {
  const provider = new DagProvider();
  vscode.window.registerTreeDataProvider("autonomic.dagExplorer", provider);

  const root = executionsRoot();
  if (fs.existsSync(root)) {
    fs.watch(root, { persistent: false }, () => provider.refresh());
  }

  context.subscriptions.push(
    vscode.commands.registerCommand("autonomic.refreshDag", () => provider.refresh()),
    vscode.commands.registerCommand("autonomic.openDagFile", (dagPath: string) => {
      if (!dagPath || !fs.existsSync(dagPath)) {
        vscode.window.showWarningMessage("DAG file not found.");
        return;
      }
      renderDagPanel(context, dagPath);
    })
  );
}

export function deactivate(): void {}
