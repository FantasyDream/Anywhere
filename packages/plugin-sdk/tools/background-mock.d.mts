import type {WebInput, WebInvocation} from '../dist/contract.js';

export interface BackgroundMockOptions {
  source: string;
  entrypoint: string;
  role: 'background' | 'resolver';
  context: WebInput;
  invocation?: WebInvocation | null;
  permissions: string[];
  methods: Record<string, {roles: string[]; permission: string | null}>;
  urlOpen?: 'allow' | 'cancel';
}

export function runBackgroundMock(
  options: BackgroundMockOptions,
): Promise<{value: unknown; denied: string[]}>;
