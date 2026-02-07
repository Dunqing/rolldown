export interface Config {
  debug: boolean;
  version: string;
}

export function getConfig(): Config {
  return { debug: false, version: '1.0.0' };
}
