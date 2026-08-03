const REDACTED = '<redacted>';

export class DiagnosticRedactor {
  redact(value) {
    let result = String(value);
    result = result.replace(
      /-----BEGIN ([A-Z0-9 ]+)-----[\s\S]*?-----END \1-----/g,
      '-----BEGIN REDACTED-----\n<redacted>\n-----END REDACTED-----'
    );
    result = result.replace(
      /\b(authorization|proxy-authorization)\s*[:=]\s*(?:bearer|basic)\s+[^\s]+/gi,
      `$1: ${REDACTED}`
    );
    result = result.replace(/\b(set-cookie|cookie):\s*[^\r\n]+/gi, `$1: ${REDACTED}`);
    result = result.replace(
      /\b(https?:\/\/[^\s/:@]+:)[^\s/@]+@/gi,
      `$1${REDACTED}@`
    );
    result = result.replace(
      /([?&](?:access_token|code|connect_ticket|id_token|refresh_token|session_ticket|token)=)[^&\s]+/gi,
      `$1${REDACTED}`
    );
    result = result.replace(
      /\b((?:[a-z0-9_]{0,64}(?:api_key|client_secret|password|ticket|token))["']?\s*[:=]\s*["']?)[^"',\s}\]]+/gi,
      `$1${REDACTED}`
    );
    result = result.replace(
      /(--[a-z0-9-]{0,64}(?:api-key|password|secret|ticket|token)(?:=|\s+))[^\s]+/gi,
      `$1${REDACTED}`
    );
    result = result.replace(
      /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]*\b/g,
      REDACTED
    );
    result = result.replace(
      /\b[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b/gi,
      '<uuid>'
    );
    result = result.replace(
      /(bpane-(?:runtime|workflow)-)[0-9a-f]{32}\b/gi,
      '$1<id>'
    );
    return result;
  }
}
