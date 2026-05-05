import { test, expect } from '@playwright/test';

const API_BASE = 'http://127.0.0.1:17371';

test.describe('Agent1 Desktop UI', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('http://127.0.0.1:5173');
  });

  test('loads the app', async ({ page }) => {
    await expect(page.locator('text=Talk to Agent1')).toBeVisible();
    await expect(page.locator('.agent1-core-button')).toBeVisible();
  });

  test('shows control surface settings', async ({ page }) => {
    await expect(page.locator('text=API Base')).toBeVisible();
  });
});

test.describe('Agent1 API', () => {
  test('health endpoint works', async ({ request }) => {
    const response = await request.get(`${API_BASE}/api/health`);
    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.ok).toBe(true);
  });

  test('agents endpoint returns array', async ({ request }) => {
    const response = await request.get(`${API_BASE}/api/agents`);
    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.agents).toBeDefined();
    expect(Array.isArray(data.agents)).toBe(true);
  });

  test('sessions endpoint returns array', async ({ request }) => {
    const response = await request.get(`${API_BASE}/api/sessions`);
    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.sessions).toBeDefined();
  });

  test('events endpoint returns array', async ({ request }) => {
    const response = await request.get(`${API_BASE}/api/events`);
    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.events).toBeDefined();
  });

  test('approvals endpoint returns array', async ({ request }) => {
    const response = await request.get(`${API_BASE}/api/approvals`);
    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.approvals).toBeDefined();
  });

  test('models endpoint returns providers', async ({ request }) => {
    const response = await request.get(`${API_BASE}/api/models`);
    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.providers).toBeDefined();
  });

  test('mcp servers endpoint returns array', async ({ request }) => {
    const response = await request.get(`${API_BASE}/api/mcp/servers`);
    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.servers).toBeDefined();
  });

  test('memory endpoint returns array', async ({ request }) => {
    const response = await request.get(`${API_BASE}/api/memory`);
    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.memories).toBeDefined();
  });

  test('can create agent', async ({ request }) => {
    const agent = {
      id: 'e2e_test_agent',
      name: 'E2E Test Agent',
      system_prompt: 'You are a test agent.',
      tools: ['file_read'],
      model: {
        provider: 'mock',
        model: 'final',
        context_window: 8192,
        temperature: 0.2,
      },
      permissions: { file_read: 'allow' },
      max_iterations: 3,
    };

    const response = await request.post(`${API_BASE}/api/agents`, {
      data: agent,
    });

    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.agent).toBeDefined();
    expect(data.agent.id).toBe('e2e_test_agent');
  });

  test('can create session', async ({ request }) => {
    await request.post(`${API_BASE}/api/agents`, {
      data: {
        id: 'session_test_agent',
        name: 'Session Test',
        system_prompt: 'Test',
        tools: [],
        model: { provider: 'mock', model: 'final', context_window: 8192, temperature: 0.2 },
        max_iterations: 1,
      },
    });

    const response = await request.post(`${API_BASE}/api/sessions`, {
      data: { root_agent_id: 'session_test_agent', title: 'E2E Test Session' },
    });

    expect(response.ok()).toBeTruthy();
    const data = await response.json();
    expect(data.session_id).toBeDefined();
  });

  test('session trace returns 404 for nonexistent', async ({ request }) => {
    const response = await request.get(`${API_BASE}/api/sessions/nonexistent/trace`);
    expect(response.status()).toBe(404);
  });

  test('session cancel returns 404 for nonexistent', async ({ request }) => {
    const response = await request.post(`${API_BASE}/api/sessions/nonexistent/cancel`);
    expect(response.status()).toBe(404);
  });

  test('well known agent endpoint works', async ({ request }) => {
    const response = await request.get(`${API_BASE}/.well-known/agent.json`);
    expect(response.ok()).toBeTruthy();
  });
});
