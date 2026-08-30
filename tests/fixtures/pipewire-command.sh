#!/bin/sh

case "$1" in
  --no-colors)
    printf '%s\n' '[{"id":0,"type":"PipeWire:Interface:Core","info":{"version":"1.6.2"}},{"id":10,"type":"PipeWire:Interface:Node","info":{"state":"running","props":{"media.class":"Stream/Output/Video","node.name":"private-desktop"}}},{"id":30,"type":"PipeWire:Interface:Client","info":{"props":{"pipewire.access.portal.is_portal":true}}}]'
    ;;
  status)
    printf '%s\n' "PipeWire 'pipewire-0' [1.6.2, private-host]"
    printf '%s\n' '  33. WirePlumber [1.6.2, private-host]'
    ;;
  *)
    exit 2
    ;;
esac
