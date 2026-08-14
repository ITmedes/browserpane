const REQUIRED_DIRECTIVES = [
  "Content-Security-Policy",
  "frame-ancestors 'none'",
  "object-src 'none'",
  "Permissions-Policy",
  "Referrer-Policy \"no-referrer\" always",
  "X-Content-Type-Options \"nosniff\" always",
  "X-Frame-Options \"DENY\" always",
];

export class AdminSecurityHeaderContract {
  validate(headerConfig) {
    for (const directive of REQUIRED_DIRECTIVES) {
      if (!headerConfig.includes(directive)) {
        throw new Error(`admin security headers must contain ${directive}`);
      }
    }
  }
}
