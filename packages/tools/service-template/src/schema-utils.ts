import { z, type ZodTypeAny } from "zod";

interface JsonSchemaProperty {
  type?: string;
  description?: string;
  minimum?: number;
  maximum?: number;
}

interface JsonSchema {
  type?: string;
  properties?: Record<string, JsonSchemaProperty>;
  required?: string[];
  additionalProperties?: boolean;
}

/**
 * Convert a JSON Schema object to a Zod schema.
 * Handles the property types used by tool inputSchema definitions:
 * string, integer, number, boolean. Respects required array for optionality.
 */
export function jsonSchemaToZod(schema: Record<string, unknown>): z.ZodObject<Record<string, ZodTypeAny>> {
  const jsonSchema = schema as JsonSchema;
  const properties = jsonSchema.properties ?? {};
  const required = new Set(jsonSchema.required ?? []);

  const shape: Record<string, ZodTypeAny> = {};

  for (const [key, prop] of Object.entries(properties)) {
    let field: ZodTypeAny;

    switch (prop.type) {
      case "string":
        field = z.string();
        break;
      case "integer":
      case "number":
        field = z.number();
        break;
      case "boolean":
        field = z.boolean();
        break;
      default:
        field = z.unknown();
        break;
    }

    if (prop.description) {
      field = field.describe(prop.description);
    }

    if (!required.has(key)) {
      field = field.optional();
    }

    shape[key] = field;
  }

  return z.object(shape);
}
