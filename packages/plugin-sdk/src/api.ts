import type {WebInput, WebCurrentApp, JsonValue} from './contract.js';
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
/** The same business API in a WebView or a finite background job. */
export interface Anywhere {
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
