FROM alpine:latest

RUN apk add tor

COPY torrc /etc/tor/torrc

CMD ["/usr/bin/tor", "-f", "/etc/tor/torrc"]