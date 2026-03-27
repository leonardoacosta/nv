/**
 * camelCase-to-snake_case mapping for API responses.
 *
 * Drizzle returns camelCase field names, but the frontend expects snake_case
 * (matching the original Rust daemon API). These helpers convert Drizzle
 * query results to the expected API response shape.
 */

/** Convert a single camelCase string to snake_case. */
function camelToSnake(str: string): string {
  return str.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

/**
 * Shallow-convert an object's keys from camelCase to snake_case.
 * Handles Date objects by converting them to ISO strings.
 * Returns null/undefined as-is.
 */
export function toSnakeCase<T extends Record<string, unknown>>(
  obj: T,
): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj)) {
    const snakeKey = camelToSnake(key);
    if (value instanceof Date) {
      result[snakeKey] = value.toISOString();
    } else {
      result[snakeKey] = value;
    }
  }
  return result;
}

/**
 * Convert an array of Drizzle rows to snake_case objects.
 */
export function rowsToSnakeCase<T extends Record<string, unknown>>(
  rows: T[],
): Record<string, unknown>[] {
  return rows.map(toSnakeCase);
}
