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

// --- Trigger API ---

import type { Trigger } from '../types/trigger';

export const triggerApi = {
  list: () => request<Trigger[]>('/triggers'),

  save: (triggers: Trigger[]) =>
    request<ApiOk>('/triggers', {
      method: 'PUT',
      body: JSON.stringify(triggers),
    }),

  get: (id: string) => request<Trigger>(`/triggers/${id}`),

  update: (id: string, trigger: Trigger) =>
    request<ApiOk>(`/triggers/${id}`, {
      method: 'PUT',
      body: JSON.stringify(trigger),
    }),

  delete: (id: string) =>
    request<ApiOk>(`/triggers/${id}`, { method: 'DELETE' }),

  generate: () =>
    request<GenerateResult>('/triggers/generate', { method: 'POST' }),
};

// --- Item Effects API ---

export const itemEffectsApi = {
  generate: () =>
    request<GenerateResult>('/items/generate', { method: 'POST' }),
};

// --- Shop API ---

export const shopApi = {
  generate: () =>
    request<GenerateResult>('/shops/generate', { method: 'POST' }),
};

// --- Dialogue API ---

export const dialogueApi = {
  generate: () =>
    request<GenerateResult>('/dialogues/generate', { method: 'POST' }),
};

// --- Quest API ---

export const questApi = {
  generate: () =>
    request<GenerateResult>('/quests/generate', { method: 'POST' }),
};

// --- Attribute Schema API ---

import type { AttributeSchema } from '../types/attribute_schema';

export const attributeSchemaApi = {
  list: () => request<AttributeSchema[]>('/attribute-schemas'),

  save: (schemas: AttributeSchema[]) =>
    request<ApiOk>('/attribute-schemas', {
      method: 'PUT',
      body: JSON.stringify(schemas),
    }),
};

// --- Level Table API ---

import type { LevelEntry } from '../types/level_table';

export const levelTableApi = {
  list: () => request<LevelEntry[]>('/level-table'),

  save: (table: LevelEntry[]) =>
    request<ApiOk>('/level-table', {
      method: 'PUT',
      body: JSON.stringify(table),
    }),
};

// --- Generate All API ---

export const generateAllApi = {
  generateAll: () =>
    request<ApiOk & { results: string[] }>('/generate-all', { method: 'POST' }),
};

// --- Server API ---

export const serverApi = {
  status: () => request<ServerStatus>('/server/status'),
  start: () => request<ApiOk & { pid: number }>('/server/start', { method: 'POST' }),
  stop: () => request<ApiOk>('/server/stop', { method: 'POST' }),
  restart: () => request<ApiOk>('/server/restart', { method: 'POST' }),
};
