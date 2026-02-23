import type { ContentItem } from '../types/content';
import type { ApiOk, ScriptFile, ScriptContent, ServerStatus } from '../types/api';

const BASE = '/api';

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
  const body = await res.json();
  if (!res.ok) {
    throw new Error(body.error || `HTTP ${res.status}`);
  }
  return body as T;
}

// --- Content API ---

export const contentApi = {
  listCollections: () => request<string[]>('/content'),

  createCollection: (id: string) =>
    request<ApiOk>('/content', {
      method: 'POST',
      body: JSON.stringify({ id }),
    }),

  deleteCollection: (collection: string) =>
    request<ApiOk>(`/content/${collection}`, { method: 'DELETE' }),

  listItems: (collection: string) =>
    request<ContentItem[]>(`/content/${collection}`),

  getItem: (collection: string, id: string) =>
    request<ContentItem>(`/content/${collection}/${id}`),

  updateItem: (collection: string, id: string, data: ContentItem) =>
    request<ApiOk>(`/content/${collection}/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  deleteItem: (collection: string, id: string) =>
    request<ApiOk>(`/content/${collection}/${id}`, { method: 'DELETE' }),
};

// --- Scripts API ---

export const scriptsApi = {
  list: () => request<ScriptFile[]>('/scripts'),

  get: (filename: string) => request<ScriptContent>(`/scripts/${filename}`),

  create: (filename: string, content: string) =>
    request<ApiOk>('/scripts', {
      method: 'POST',
      body: JSON.stringify({ filename, content }),
    }),

  update: (filename: string, content: string) =>
    request<ApiOk>(`/scripts/${filename}`, {
      method: 'PUT',
      body: JSON.stringify({ content }),
    }),

  delete: (filename: string) =>
    request<ApiOk>(`/scripts/${filename}`, { method: 'DELETE' }),
};

// --- World API ---

import type { WorldData, Room, PlacedEntity, GenerateResult } from '../types/world';

export const worldApi = {
  get: () => request<WorldData>('/world'),

  save: (world: WorldData) =>
    request<ApiOk>('/world', {
      method: 'PUT',
      body: JSON.stringify(world),
    }),

  getRoom: (id: string) => request<Room>(`/world/rooms/${id}`),

  updateRoom: (id: string, room: Room) =>
    request<ApiOk>(`/world/rooms/${id}`, {
      method: 'PUT',
      body: JSON.stringify(room),
    }),

  deleteRoom: (id: string) =>
    request<ApiOk>(`/world/rooms/${id}`, { method: 'DELETE' }),

  updateRoomEntities: (roomId: string, entities: PlacedEntity[]) =>
    request<ApiOk>(`/world/rooms/${roomId}/entities`, {
      method: 'PUT',
      body: JSON.stringify(entities),
    }),

  generate: () =>
    request<GenerateResult>('/world/generate', { method: 'POST' }),
};

// --- Server API ---

export const serverApi = {
  status: () => request<ServerStatus>('/server/status'),
  start: () => request<ApiOk & { pid: number }>('/server/start', { method: 'POST' }),
  stop: () => request<ApiOk>('/server/stop', { method: 'POST' }),
  restart: () => request<ApiOk>('/server/restart', { method: 'POST' }),
};
