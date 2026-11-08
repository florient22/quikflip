// Small client for the QuikFlip API. Every failure becomes an ApiError with a stable `code` the pages translate.

export class ApiError extends Error {
  constructor(code, message, detail = null, status = 0) {
    super(message);
    this.code = code;
    this.detail = detail;
    this.status = status;
  }
}

async function request(path, { method = 'GET', body } = {}) {
  let response;
  try {
    response = await fetch(path, {
      method,
      headers: body ? { 'Content-Type': 'application/json' } : undefined,
      body: body ? JSON.stringify(body) : undefined,
    });
  } catch {
    throw new ApiError('network', 'Could not reach the QuikFlip server.');
  }
  const data = await response.json().catch(() => null);
  if (!response.ok) {
    throw new ApiError(data?.error ?? 'internal', data?.message ?? `The server answered ${response.status}.`, data?.detail ?? null, response.status);
  }
  return data;
}

export const api = {
  home: () => request('/api/home'),
  settings: () => request('/api/settings'),
  saveAppSettings: settings => request('/api/settings/app', { method: 'PUT', body: settings }),
  saveLetterlockSettings: settings => request('/api/settings/letterlock', { method: 'PUT', body: settings }),
  letterlock: () => request('/api/letterlock'),
  guess: word => request('/api/letterlock/guess', { method: 'POST', body: { word } }),
  saveRarefindSettings: settings => request('/api/settings/rarefind', { method: 'PUT', body: settings }),
  rarefind: () => request('/api/rarefind'),
  rarefindStart: position => request('/api/rarefind/start', { method: 'POST', body: { position } }),
  rarefindAnswer: (position, text) => request('/api/rarefind/answer', { method: 'POST', body: { position, text } }),
  rarefindExpire: position => request('/api/rarefind/expire', { method: 'POST', body: { position } }),
  rarefindContest: (position, text) => request('/api/rarefind/contest', { method: 'POST', body: { position, text } }),
};
