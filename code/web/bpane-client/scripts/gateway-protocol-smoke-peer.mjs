export class GatewayProtocolSmokePeer {
  #page;
  #timeoutMs;

  constructor(page, timeoutMs) {
    this.#page = page;
    this.#timeoutMs = timeoutMs;
  }

  async exchange({ gatewayUrl, ticket, initialBytes, afterSelectionBytes = [], targetTags }) {
    if (!gatewayUrl || !ticket || targetTags.length === 0) {
      throw new Error('gateway protocol peer requires a URL, ticket, and target tag');
    }
    return await this.#page.evaluate(async (input) => {
      const timeout = async (promise, label) => {
        let timer;
        try {
          return await Promise.race([
            promise,
            new Promise((_, reject) => {
              timer = setTimeout(() => reject(new Error(`timed out waiting for ${label}`)), input.timeoutMs);
            }),
          ]);
        } finally {
          clearTimeout(timer);
        }
      };
      const append = (left, right) => {
        const combined = new Uint8Array(left.length + right.length);
        combined.set(left);
        combined.set(right, left.length);
        return combined;
      };
      const parseFrame = (pending) => {
        if (pending.length < 5) return null;
        const view = new DataView(pending.buffer, pending.byteOffset, pending.byteLength);
        const payloadLength = view.getUint32(1, true);
        if (payloadLength > 16 * 1024 * 1024) throw new Error('gateway returned oversized frame');
        if (pending.length < payloadLength + 5) return null;
        return {
          frame: {
            channel: pending[0],
            payload: Array.from(pending.subarray(5, payloadLength + 5)),
          },
          remaining: pending.slice(payloadLength + 5),
        };
      };

      const url = `${input.gatewayUrl}?session_ticket=${encodeURIComponent(input.ticket)}&_=${Date.now()}`;
      const transport = new WebTransport(url);
      const frames = [];
      let pending = new Uint8Array();
      try {
        await timeout(transport.ready, 'WebTransport readiness');
        const incoming = transport.incomingBidirectionalStreams.getReader();
        const streamResult = await timeout(incoming.read(), 'gateway bidirectional stream');
        if (streamResult.done || !streamResult.value) throw new Error('gateway stream ended before negotiation');
        const reader = streamResult.value.readable.getReader();
        const writer = streamResult.value.writable.getWriter();
        if (input.initialBytes.length > 0) {
          await timeout(writer.write(new Uint8Array(input.initialBytes)), 'initial protocol write');
        }
        let sentAfterSelection = input.afterSelectionBytes.length === 0;

        while (frames.length < 128) {
          let decoded = parseFrame(pending);
          if (!decoded) {
            const read = await timeout(reader.read(), 'gateway protocol frame');
            if (read.done || !read.value) break;
            pending = append(pending, new Uint8Array(read.value));
            if (pending.length > 256 * 1024) throw new Error('gateway smoke peer buffer limit exceeded');
            decoded = parseFrame(pending);
            if (!decoded) continue;
          }
          frames.push(decoded.frame);
          pending = decoded.remaining;
          if (!sentAfterSelection && decoded.frame.channel === 0x0A && decoded.frame.payload[0] === 0x0B) {
            await timeout(
              writer.write(new Uint8Array(input.afterSelectionBytes)),
              'post-selection protocol write',
            );
            sentAfterSelection = true;
          }
          const observed = new Set(
            frames.filter((frame) => frame.channel === 0x0A).map((frame) => frame.payload[0]),
          );
          if (input.targetTags.every((tag) => observed.has(tag))) break;
        }
        return frames;
      } finally {
        transport.close();
      }
    }, {
      gatewayUrl,
      ticket,
      initialBytes,
      afterSelectionBytes,
      targetTags,
      timeoutMs: this.#timeoutMs,
    });
  }
}
