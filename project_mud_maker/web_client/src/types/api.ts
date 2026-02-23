export interface ApiOk {
  ok: true;
}

export interface ApiError {
  error: string;
}

export interface ScriptFile {
  filename: string;
  size: number;
}

export interface ScriptContent {
  filename: string;
  content: string;
}

export interface ServerStatus {
  running: boolean;
  pid?: number;
}
