/**
 * Validates a 5-field standard cron expression.
 * Fields: minute hour day-of-month month day-of-week
 *
 * Each field allows: numbers, *, ranges (1-5), steps (asterisk/15), and lists (1,3,5).
 */

const FIELD_RANGES: [number, number][] = [
  [0, 59],  // minute
  [0, 23],  // hour
  [1, 31],  // day of month
  [1, 12],  // month
  [0, 7],   // day of week (0 and 7 are both Sunday)
];

function validateField(field: string, min: number, max: number): boolean {
  // Split by comma for lists (e.g. "1,3,5")
  const parts = field.split(",");

  for (const part of parts) {
    // Check for step values (e.g. "*/15" or "1-5/2")
    const stepParts = part.split("/");
    if (stepParts.length > 2) return false;

    const base = stepParts[0]!;
    const step = stepParts[1];

    // Validate step if present
    if (step !== undefined) {
      const stepNum = parseInt(step, 10);
      if (isNaN(stepNum) || stepNum < 1) return false;
    }

    // Wildcard
    if (base === "*") continue;

    // Range (e.g. "1-5")
    if (base.includes("-")) {
      const rangeParts = base.split("-");
      if (rangeParts.length !== 2) return false;
      const start = parseInt(rangeParts[0]!, 10);
      const end = parseInt(rangeParts[1]!, 10);
      if (isNaN(start) || isNaN(end)) return false;
      if (start < min || end > max || start > end) return false;
      continue;
    }

    // Single number
    const num = parseInt(base, 10);
    if (isNaN(num) || num < min || num > max) return false;
  }

  return true;
}

export function validateCron(expr: string): boolean {
  const fields = expr.trim().split(/\s+/);
  if (fields.length !== 5) return false;

  for (let i = 0; i < 5; i++) {
    const [min, max] = FIELD_RANGES[i]!;
    if (!validateField(fields[i]!, min, max)) return false;
  }

  return true;
}
