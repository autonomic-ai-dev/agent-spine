<script lang="ts">
  import { onMount } from 'svelte';
  import { createPromiseClient } from '@connectrpc/connect';
  import { createGrpcWebTransport } from '@connectrpc/connect-web';
  import { DashboardService, SupervisorService } from './gen/supervisor_connect';

  const transport = createGrpcWebTransport({
    baseUrl: 'http://localhost:3000',
  });

  const dashboardClient = createPromiseClient(DashboardService, transport);
  const supervisorClient = createPromiseClient(SupervisorService, transport);

  let executions: string[] = [];
  let selectedExecutionId: string | null = null;
  let history: any[] = [];
  let pendingTasks: string[] = [];

  onMount(() => {
    fetchExecutions();
    fetchPending();
    setInterval(fetchPending, 2000); // Poll for pending tasks
  });

  async function fetchExecutions() {
    try {
      const res = await dashboardClient.listExecutions({});
      executions = res.executionIds;
    } catch (e) {
      console.error('Failed to fetch executions', e);
    }
  }

  async function selectExecution(id: string) {
    selectedExecutionId = id;
    try {
      const res = await dashboardClient.getExecutionHistory({ executionId: id });
      history = res.history.map(h => ({
        sequence: h.sequence,
        payload: JSON.parse(h.payloadJson || '{}')
      }));
    } catch (e) {
      console.error('Failed to fetch history', e);
    }
  }

  async function fetchPending() {
    try {
      const res = await supervisorClient.getPendingTasks({});
      pendingTasks = res.tasks.map(t => t.nodeName);
    } catch (e) {
      console.error('Failed to fetch pending tasks', e);
    }
  }

  async function resumeTask(nodeName: string, approved: boolean) {
    try {
      await supervisorClient.resumeExecution({
        nodeName,
        payloadJson: JSON.stringify({ approved })
      });
      fetchPending();
    } catch (e) {
      console.error('Failed to resume task', e);
    }
  }
</script>

<main class="app-container">
  <aside class="sidebar glass-panel">
    <h2>Executions</h2>
    <div class="execution-list">
      {#if executions.length === 0}
        <p class="muted">No executions found.</p>
      {/if}
      {#each executions as id}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div 
          class="execution-item {selectedExecutionId === id ? 'active' : ''}" 
          on:click={() => selectExecution(id)}
        >
          <div class="execution-id">{id.substring(0, 8)}...</div>
        </div>
      {/each}
    </div>
    
    <div class="spacer"></div>
    
    <h2>Action Center</h2>
    {#if pendingTasks.length === 0}
      <p class="muted">No pending tasks.</p>
    {/if}
    {#each pendingTasks as task}
      <div class="pending-task glass-panel">
        <div class="task-header">
          <span class="badge">HITL</span>
          <strong>{task}</strong>
        </div>
        <p class="muted">Awaiting human approval.</p>
        <div class="actions">
          <button class="primary" on:click={() => resumeTask(task, true)}>Approve</button>
          <button class="danger" on:click={() => resumeTask(task, false)}>Reject</button>
        </div>
      </div>
    {/each}
  </aside>

  <section class="main-content">
    {#if selectedExecutionId}
      <header class="glass-panel header-panel">
        <h1>Execution Details</h1>
        <p class="muted">ID: {selectedExecutionId}</p>
      </header>

      <div class="history-timeline">
        {#each history as snapshot, i}
          <div class="snapshot glass-panel">
            <div class="snapshot-header">
              <span class="seq-badge">Seq {snapshot.sequence}</span>
            </div>
            <pre class="json-viewer">{JSON.stringify(snapshot.payload, null, 2)}</pre>
          </div>
        {/each}
      </div>
    {:else}
      <div class="glass-panel empty-state">
        <h2 style="margin-bottom: 8px;">Agent Spine Dashboard</h2>
        <p class="muted">Select an execution from the sidebar to view its history.</p>
      </div>
    {/if}
  </section>
</main>

<style>
  .muted {
    color: var(--text-muted);
    font-size: 0.9rem;
  }
  
  .spacer {
    margin: 16px 0;
    height: 1px;
    background: var(--panel-border);
  }

  .execution-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 12px;
  }

  .execution-item {
    padding: 12px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.02);
    cursor: pointer;
    border: 1px solid transparent;
    transition: all 0.2s ease;
  }

  .execution-item:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .execution-item.active {
    background: rgba(59, 130, 246, 0.1);
    border-color: rgba(59, 130, 246, 0.3);
    color: var(--accent);
  }

  .execution-id {
    font-family: monospace;
    font-size: 1.1rem;
  }

  .pending-task {
    margin-top: 12px;
    padding: 16px;
    background: rgba(239, 68, 68, 0.05);
    border-color: rgba(239, 68, 68, 0.2);
  }

  .task-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  .badge {
    background: var(--danger);
    color: white;
    font-size: 0.7rem;
    padding: 2px 6px;
    border-radius: 4px;
    font-weight: 600;
  }

  .actions {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }

  .header-panel {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 24px;
  }

  .history-timeline {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .snapshot {
    animation: fadeIn 0.3s ease;
  }

  .snapshot-header {
    display: flex;
    align-items: center;
    margin-bottom: 12px;
  }

  .seq-badge {
    background: rgba(255, 255, 255, 0.1);
    padding: 4px 10px;
    border-radius: 12px;
    font-size: 0.8rem;
    font-weight: 500;
  }

  .json-viewer {
    background: rgba(0, 0, 0, 0.3);
    padding: 16px;
    border-radius: 8px;
    font-family: monospace;
    font-size: 0.85rem;
    color: #e2e8f0;
    overflow-x: auto;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    text-align: center;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(10px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
