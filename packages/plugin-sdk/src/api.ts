import type {WebInput, WebCurrentApp} from './contract.js';
export type Unsubscribe = () => void;
/** The same business API in a WebView or a finite background job. */
export interface Anywhere {
  context: {
    get(): Promise<WebInput>;
    onChange(callback: (context: WebInput) => void): Unsubscribe;
  };
  input: {
    get(): Promise<string>;
    set(text: string): Promise<void>;
    onChange(callback: (input: string) => void): Unsubscribe;
  };
  clipboard: {
    readText(): Promise<string | null>;
    writeText(text: string): Promise<void>;
  };
  app: {
    getSource(): Promise<WebCurrentApp | null>;
    getFrontmost(): Promise<WebCurrentApp | null>;
  };
}
