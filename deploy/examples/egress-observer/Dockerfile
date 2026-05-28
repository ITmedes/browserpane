FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends apache2-utils ca-certificates squid \
    && rm -rf /var/lib/apt/lists/*

COPY squid.conf /etc/squid/squid.conf
COPY squid-auth.conf /etc/squid/squid-auth.conf

RUN mkdir -p /var/log/squid /var/spool/squid /run/squid \
    && htpasswd -bc /etc/squid/proxy-auth.htpasswd proxy-user proxy-pass \
    && chown proxy:proxy /etc/squid/proxy-auth.htpasswd \
    && chmod 640 /etc/squid/proxy-auth.htpasswd \
    && chown -R proxy:proxy /var/log/squid /var/spool/squid /run/squid

USER proxy
EXPOSE 3128 3130

CMD ["squid", "-N", "-f", "/etc/squid/squid.conf"]
