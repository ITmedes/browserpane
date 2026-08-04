export const adminCsp = {
  mode: 'hash',
  directives: {
    'default-src': ['self'],
    'base-uri': ['self'],
    'object-src': ['none'],
    'script-src': ['self'],
    'style-src': ['self', 'unsafe-inline'],
    'img-src': ['self', 'data:', 'blob:'],
    'font-src': ['self', 'data:'],
    'connect-src': ['self', 'https:', 'http://localhost:*', 'ws:', 'wss:'],
    'media-src': ['self', 'blob:'],
    'worker-src': ['self', 'blob:'],
    'form-action': ['self', 'https:', 'http://localhost:*'],
  },
};
