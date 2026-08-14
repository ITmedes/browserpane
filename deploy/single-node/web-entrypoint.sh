#!/bin/sh
set -eu

/usr/local/bin/web-auth-config
exec nginx -g 'daemon off;'
