export function parseOtlpJsonLines(content) {
  const lines = content.split("\n");
  const hasPartialTail = lines.at(-1)?.trim() !== "";
  if (hasPartialTail) lines.pop();
  return lines
    .filter((line) => line.trim())
    .map((line, index) => {
      try {
        return JSON.parse(line);
      } catch (error) {
        throw new Error(`invalid OTLP JSON object on completed line ${index + 1}: ${error.message}`);
      }
    });
}

export function flattenTraceSpans(batches) {
  const spans = [];
  for (const batch of batches) {
    for (const resourceSpans of batch.resourceSpans ?? []) {
      const resourceAttributes = attributes(resourceSpans.resource?.attributes);
      for (const scopeSpans of resourceSpans.scopeSpans ?? []) {
        for (const span of scopeSpans.spans ?? []) {
          spans.push({
            ...span,
            attributes: attributes(span.attributes),
            resourceAttributes,
            serviceName: resourceAttributes["service.name"],
          });
        }
      }
    }
  }
  return spans;
}

export function traceSpans(content, traceId) {
  return flattenTraceSpans(parseOtlpJsonLines(content))
    .filter((span) => span.traceId?.toLowerCase() === traceId.toLowerCase());
}

function attributes(entries = []) {
  return Object.fromEntries(entries.map((entry) => [entry.key, attributeValue(entry.value)]));
}

function attributeValue(value = {}) {
  for (const key of ["stringValue", "intValue", "doubleValue", "boolValue", "bytesValue"]) {
    if (Object.hasOwn(value, key)) return value[key];
  }
  if (value.arrayValue) return (value.arrayValue.values ?? []).map(attributeValue);
  if (value.kvlistValue) return attributes(value.kvlistValue.values);
  return null;
}
