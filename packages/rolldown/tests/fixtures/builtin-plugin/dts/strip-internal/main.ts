/**
 * Public configuration interface.
 */
export interface Config {
  name: string;
  version: string;
}

/**
 * @internal
 * Internal implementation details - should be stripped.
 */
export interface InternalConfig {
  debugMode: boolean;
  secretKey: string;
}

/**
 * Public function to get config.
 */
export function getConfig(): Config {
  return { name: 'app', version: '1.0.0' };
}

/**
 * @internal
 * Internal helper - should be stripped.
 */
export function internalHelper(): void {
  console.log('internal');
}
