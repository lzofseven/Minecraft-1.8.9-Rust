#!/usr/bin/env python3
"""Proxy TCP com atraso fixo em cada direção: lagproxy.py PORTA_LOCAL HOST:PORTA ATRASO_MS."""
import asyncio, sys, time

LOCAL, DEST, DELAY = int(sys.argv[1]), sys.argv[2], int(sys.argv[3]) / 1000.0

async def pipe(reader, writer):
    q = asyncio.Queue()
    async def ler():
        while True:
            data = await reader.read(65536)
            await q.put((time.monotonic() + DELAY, data))
            if not data:
                return
    async def escrever():
        while True:
            due, data = await q.get()
            wait = due - time.monotonic()
            if wait > 0:
                await asyncio.sleep(wait)
            if not data:
                writer.close()
                return
            writer.write(data)
            await writer.drain()
    await asyncio.gather(ler(), escrever(), return_exceptions=True)

async def handle(cr, cw):
    host, port = DEST.rsplit(":", 1)
    sr, sw = await asyncio.open_connection(host, int(port))
    await asyncio.gather(pipe(cr, sw), pipe(sr, cw), return_exceptions=True)

async def main():
    srv = await asyncio.start_server(handle, "127.0.0.1", LOCAL)
    async with srv:
        await srv.serve_forever()

asyncio.run(main())
