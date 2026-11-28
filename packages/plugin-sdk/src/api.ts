import type {
  WebInput,
  WebCurrentApp,
  JsonValue,
  CallResults,
} from './contract.js';
/** Structural cancellation port; background packages require no DOM typings. */
export interface CallOptions {
  signal?: {
    readonly aborted: boolean;
    addEventListener(
      type: 'abort',
      listener: () => void,
      options?: {once?: boolean},
    ): void;
    removeEventListener(type: 'abort', listener: () => void): void;
  };
}
export type Unsubscribe = () => void;
export type ResourceStat = CallResults['resources.stat'];
export type ReadTextOptions = CallOptions & {maxBytes?: number};

/** Business APIs available in a WebView and a finite background action. */
export interface BackgroundAnywhere {
  url: {
    open(url: string, options?: CallOptions): Promise<void>;
  };
  resources: {
    stat(handle: string, options?: CallOptions): Promise<ResourceStat>;
    readText(handle: string, options?: ReadTextOptions): Promise<string>;
  };
  context: {
    get(): Promise<WebInput>;
    onChange(callback: (context: WebInput) => void): Unsubscribe;
  };
  input: {
    get(): Promise<string>;
    set(text: string, options?: CallOptions): Promise<void>;
    onChange(callback: (input: string) => void): Unsubscribe;
  };
  clipboard: {
    readText(options?: CallOptions): Promise<string | null>;
    writeText(text: string, options?: CallOptions): Promise<void>;
  };
  storage: {
    get<T extends JsonValue = JsonValue>(
      key: string,
      options?: CallOptions,
    ): Promise<T | null>;
    set(key: string, value: JsonValue, options?: CallOptions): Promise<void>;
    remove(key: string, options?: CallOptions): Promise<void>;
  };
  app: {
    getSource(options?: CallOptions): Promise<WebCurrentApp | null>;
    getFrontmost(options?: CallOptions): Promise<WebCurrentApp | null>;
  };
}

/** Native window operations are offered only to an active WebView. */
export interface Anywhere extends BackgroundAnywhere {
  window: {
    back(options?: CallOptions): Promise<void>;
    close(options?: CallOptions): Promise<void>;
  };
  preview: {
    open(handle: string, options?: CallOptions): Promise<void>;
  };
}
