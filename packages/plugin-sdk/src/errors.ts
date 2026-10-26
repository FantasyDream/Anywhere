export class HostError extends Error {
  readonly code: string;
  constructor(
    code: string,
    message = 'The Host request could not be completed.',
  ) {
    super(message);
    this.name = 'HostError';
    this.code = code;
  }
}
