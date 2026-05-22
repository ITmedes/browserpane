FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates squid \
    && rm -rf /var/lib/apt/lists/*

COPY squid.conf /etc/squid/squid.conf

RUN mkdir -p /var/log/squid /var/spool/squid /run/squid \
    && chown -R proxy:proxy /var/log/squid /var/spool/squid /run/squid

USER proxy
EXPOSE 3128

CMD ["squid", "-N", "-f", "/etc/squid/squid.conf"]
