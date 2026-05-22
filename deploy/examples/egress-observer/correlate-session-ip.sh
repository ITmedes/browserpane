#!/usr/bin/env bash
set -euo pipefail

containers="$(docker ps -q --filter label=browserpane.session_id)"
if [ -z "$containers" ]; then
  echo "No BrowserPane runtime containers found."
  exit 0
fi

docker inspect \
  --format '{{.Name}} session_id={{index .Config.Labels "browserpane.session_id"}} egress_profile_id={{index .Config.Labels "browserpane.egress_profile_id"}} proxy_configured={{index .Config.Labels "browserpane.egress_proxy_configured"}} {{range $name, $network := .NetworkSettings.Networks}}{{$name}}={{$network.IPAddress}} {{end}}' \
  $containers
