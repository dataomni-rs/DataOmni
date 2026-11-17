import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  Channel: class {}
}));

const { useTaskStore } = await import('./taskStore');

async function settled(id: string) {
  await vi.waitFor(() => {
    const task = useTaskStore.getState().tasks.find((candidate) => candidate.id === id);
    expect(task?.finishedAt).not.toBeNull();
  });
  return useTaskStore.getState().tasks.find((candidate) => candidate.id === id);
}

describe('任务失败', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useTaskStore.setState({ tasks: [] });
  });

  it('原因写在那一行上，不必展开日志才看得到', async () => {
    // 此前一行只写「失败」，原因只在日志里
    invokeMock.mockRejectedValue('pg_dump: error: connection refused');
    const backup = useTaskStore.getState().start({
      kind: 'backup',
      title: 'b',
      payload: { connectionId: 'c', path: '/tmp/x.dump' }
    });
    expect((await settled(backup))?.detail).toContain('connection refused');

    invokeMock.mockRejectedValue('permission denied');
    const exported = useTaskStore.getState().start({
      kind: 'export',
      title: 'e',
      payload: { connectionId: 'c', sql: 'SELECT 1', path: '/x.csv', options: {} as never }
    });
    expect((await settled(exported))?.detail).toContain('permission denied');
  });

  it('导入失败也一样，进度那句换成原因', async () => {
    invokeMock.mockRejectedValue('relation "t" does not exist');
    const id = useTaskStore.getState().start({
      kind: 'import',
      title: 'i',
      payload: {
        connectionId: 'c',
        schema: null,
        table: 't',
        path: '/x.csv',
        csv: { delimiter: ',', hasHeader: true, nullText: '' },
        columns: [],
        batchSize: 100,
        strategy: 'single-transaction',
        onError: 'abort'
      }
    });
    expect((await settled(id))?.detail).toContain('does not exist');
  });

  it('取消的导出不算失败，也不写原因', async () => {
    invokeMock.mockRejectedValue({ code: 'EXPORT_CANCELLED', message: 'cancelled' });
    const id = useTaskStore.getState().start({
      kind: 'export',
      title: 'e',
      payload: { connectionId: 'c', sql: 'SELECT 1', path: '/x.csv', options: {} as never }
    });
    const task = await settled(id);
    expect(task?.status).toBe('cancelled');
    expect(task?.detail).toBeNull();
  });

  it('当前活动连接上的导出或导入遇到 CONNECTION_LOST 时把连接标成已断开，非当前连接或磁盘错误不误标', async () => {
    const { useQueryStore } = await import('./queryStore');
    useQueryStore.setState({
      connectionId: 'active-conn',
      database: {} as never,
      connectionLost: false
    });

    // 1. 磁盘权限错不是断线
    invokeMock.mockRejectedValueOnce('permission denied');
    const diskFail = useTaskStore.getState().start({
      kind: 'export',
      title: 'e1',
      payload: { connectionId: 'active-conn', sql: 'SELECT 1', path: '/x.csv', options: {} as never }
    });
    await settled(diskFail);
    expect(useQueryStore.getState().connectionLost).toBe(false);

    // 2. 用户已切走的旧连接导出断线，不把当前连接误标为已断开
    invokeMock.mockRejectedValueOnce({ code: 'CONNECTION_LOST', message: 'DATAOMNI_CONNECTION_LOST: EOF' });
    const otherConnFail = useTaskStore.getState().start({
      kind: 'export',
      title: 'e2',
      payload: { connectionId: 'other-conn', sql: 'SELECT 1', path: '/x.csv', options: {} as never }
    });
    await settled(otherConnFail);
    expect(useQueryStore.getState().connectionLost).toBe(false);

    // 3. 当前活动连接的导出断线：立刻标为已断开，让重连按钮出现
    invokeMock.mockRejectedValueOnce({ code: 'CONNECTION_LOST', message: 'DATAOMNI_CONNECTION_LOST: EOF' });
    const activeConnFail = useTaskStore.getState().start({
      kind: 'export',
      title: 'e3',
      payload: { connectionId: 'active-conn', sql: 'SELECT 1', path: '/x.csv', options: {} as never }
    });
    await settled(activeConnFail);
    expect(useQueryStore.getState().connectionLost).toBe(true);
  });
});
